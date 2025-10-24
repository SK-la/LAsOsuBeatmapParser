using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Numerics;
using System.Threading.Tasks;
using BenchmarkDotNet.Attributes;
using LAsOsuBeatmapParser.Analysis;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Beatmaps.Formats;
using Xunit;
using Xunit.Abstractions;

namespace LAsOsuBeatmapParser.Tests;

public class SRCalculatorTests
{
    private readonly ITestOutputHelper _output;

    // SR显示精度控制常量
    private const int SR_DECIMAL_PLACES = 4;

    public SRCalculatorTests(ITestOutputHelper output)
    {
        _output = output;
    }

    [MemoryDiagnoser]
    public class SRCalculatorBenchmarks
    {
        private Beatmap<ManiaHitObject> _testBeatmap;
        private SRCalculator _calculator;

        [GlobalSetup]
        public void Setup()
        {
            // 创建测试数据：一个中等复杂度的谱面
            var random = new Random(42); // 固定种子确保一致性
            var totalNotes = 1000;
            var maxTime = 180000; // 3分钟
            var keyCount = 4;

            var hitObjects = new List<ManiaHitObject>();
            for (var i = 0; i < totalNotes; i++)
            {
                var time = (int)(i * (maxTime / (double)totalNotes));
                var column = random.Next(0, keyCount);
                var isLn = random.Next(0, 10) < 2; // 20% LN
                var tail = isLn ? time + random.Next(500, 2000) : time;

                var hitObject = new ManiaHitObject(time, column, keyCount);
                if (isLn)
                {
                    hitObject.EndTime = tail;
                }
                hitObjects.Add(hitObject);
            }

            _testBeatmap = new Beatmap<ManiaHitObject>();
            _testBeatmap.Difficulty.CircleSize = keyCount;
            _testBeatmap.Difficulty.OverallDifficulty = 8.0f;
            _testBeatmap.HitObjects = hitObjects;

            _calculator = SRCalculator.Instance;
        }

        [Benchmark]
        public double CalculateSR()
        {
            return _calculator.CalculateSR(_testBeatmap, out _);
        }

        [Benchmark]
        public async Task<double> CalculateSRAsync()
        {
            var (sr, _) = await _calculator.CalculateSRAsync(_testBeatmap);
            return sr;
        }
    }

    [Fact]
    public void RunDetailedPerformanceAnalysis()
    {
        // 加载真实的谱面文件 (4k-10k)
        var resourcePath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", "..", "Resource");
        var osuFiles = Directory.GetFiles(resourcePath, "*.osu")
            .Where(f => !f.Contains("encoded_output")) // 排除编码输出文件
            .OrderBy(f => Path.GetFileName(f))
            .ToArray();

        var beatmaps = osuFiles.Select(f => new LegacyBeatmapDecoder().Decode(f)).ToArray();
        var keyCounts = beatmaps.Select(bm => (int)bm.BeatmapInfo.Difficulty.CircleSize).ToArray();

        _output.WriteLine($"=== SR计算详细性能分析 (4k-10k) === 队列{beatmaps.Length}，运行3次以获取平均时间...");
        _output.WriteLine("");

        // 计算SR和时间
        var srs = new List<double>();
        var timesList = new List<Dictionary<string, long>>();

        foreach (var bm in beatmaps)
        {
            double srSum = 0;
            var timesSum = new Dictionary<string, long>();
            for (var i = 0; i < 3; i++)
            {
                var sr = SRCalculator.Instance.CalculateSR(bm, out var times);
                srSum += sr;
                foreach (var kv in times)
                {
                    if (!timesSum.ContainsKey(kv.Key)) timesSum[kv.Key] = 0;
                    timesSum[kv.Key] += kv.Value;
                }
            }
            srs.Add(srSum / 3);
            timesList.Add(timesSum.ToDictionary(kv => kv.Key, kv => kv.Value / 3));
        }

        // 使用10k谱面的数据作为代表
        var benchmarkIndex = Array.FindIndex(keyCounts, k => k == 10);
        if (benchmarkIndex == -1) benchmarkIndex = keyCounts.Length - 1; // 如果没有10k，用最后一个
        var avgTimes = timesList[benchmarkIndex];
        var avgSR = srs[benchmarkIndex];

        // 计算一致性 (标准差)
        double CalculateStdDev(List<double> values)
        {
            var avg = values.Average();
            return Math.Sqrt(values.Sum(v => Math.Pow(v - avg, 2)) / values.Count);
        }

        var consistency = CalculateStdDev(srs);

        // 生成 ASCII 表格
        var sections = new[] { "Section232425", "Section2627", "Section3", "Total" };
        var displaySections = new[] { "Section23/24/25", "Section26/27", "Section3", "Total" };
        var colWidths = new[] { 8, 15, 11, 7, 5, 6, 10 }; // 版本, Section23/24/25, Section26/27, Section3, Total, SR, 一致性

        // 表头
        var header =
            $"| {"版本".PadRight(colWidths[0])} | {displaySections[0].PadRight(colWidths[1])} | {displaySections[1].PadRight(colWidths[2])} | {displaySections[2].PadRight(colWidths[3])} | {displaySections[3].PadRight(colWidths[4])} | {"SR".PadRight(colWidths[5])} | {"一致性".PadRight(colWidths[6])} |";
        var separator = $"+{string.Join("+", colWidths.Select(w => new string('-', w + 2)))}+";

        _output.WriteLine(separator);
        _output.WriteLine(header);
        _output.WriteLine(separator);

        // 新版本行
        var newTimes = sections.Select(s =>
                avgTimes.GetValueOrDefault(s, 0).ToString("F1").PadLeft(colWidths[Array.IndexOf(sections, s) + 1]))
            .ToArray();
        var srStr = avgSR.ToString($"F{SR_DECIMAL_PLACES}").PadLeft(colWidths[5]);
        var consistencyStr = consistency.ToString("F4").PadLeft(colWidths[6]);
        _output.WriteLine(
            $"| {"新版本".PadRight(colWidths[0])} | {newTimes[0]} | {newTimes[1]} | {newTimes[2]} | {newTimes[3]} | {srStr} | {consistencyStr} |");

        _output.WriteLine(separator);

        // 新表：4-10k 详细数据
        _output.WriteLine("=== 4-10k 详细数据 ===");
        var kLabels = keyCounts.Select(k => $"{k}k").ToArray();
        var detailColWidths = new[] { 10 }.Concat(Enumerable.Repeat(8, kLabels.Length)).ToArray(); // 项目, 4k, 5k, ...
        var detailHeader = $"| {"项目".PadRight(detailColWidths[0])} | {string.Join(" | ", kLabels.Select((k, i) => k.PadRight(detailColWidths[i + 1])))} |";
        var detailSeparator = $"+{string.Join("+", detailColWidths.Select(w => new string('-', w + 2)))}+";

        _output.WriteLine(detailSeparator);
        _output.WriteLine(detailHeader);
        _output.WriteLine(detailSeparator);

        // SR 行
        var srRow = $"| {"SR".PadRight(detailColWidths[0])} | {string.Join(" | ", srs.Select(sr => sr.ToString($"F{SR_DECIMAL_PLACES}").PadLeft(detailColWidths[1])))} |";
        _output.WriteLine(srRow);

        // 总用时 行
        var timeRow = $"| {"总用时".PadRight(detailColWidths[0])} | {string.Join(" | ", timesList.Select(t => t.GetValueOrDefault("Total", 0).ToString("F1").PadLeft(detailColWidths[1])))} |";
        _output.WriteLine(timeRow);

        _output.WriteLine(detailSeparator);
        _output.WriteLine("");

        // 验证SR结果合理性
        foreach (var sr in srs)
        {
            Assert.True(sr >= 0, $"SR值不能为负: {sr}");
            Assert.True(sr <= 100, $"SR值过高: {sr}"); // 合理的上限
        }

        _output.WriteLine($"✅ SR结果合理性验证通过");

        // 验证times包含预期键 (只对有效SR验证)
        var expectedKeys = new[] { "Section232425", "Section2627", "Section3", "Total" };
        for (int i = 0; i < srs.Count; i++)
        {
            var sr = srs[i];
            var times = timesList[i];
            if (sr > 0) // 只验证有效SR的times字典 (SR=-1表示不支持的键数)
            {
                foreach (var key in expectedKeys)
                {
                    Assert.True(times.ContainsKey(key), $"times字典缺少键: {key} (SR={sr})");
                    Assert.True(times[key] >= 0, $"时间值不能为负: {key} = {times[key]}");
                }
            }
        }

        _output.WriteLine($"✅ times字典验证通过");
    }
}
