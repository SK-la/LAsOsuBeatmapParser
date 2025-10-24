using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Numerics;
using System.Threading.Tasks;
using BenchmarkDotNet.Attributes;
using LAsOsuBeatmapParser.Analysis;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Beatmaps.Formats;
using LAsOsuBeatmapParser.Extensions;
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
            var totalNotes = 2000;
            var maxTime = 180000; // 3分钟
            var keyCount = 10;

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

        _output.WriteLine($"=== SR计算详细性能分析 (4k-10k) === 队列{beatmaps.Length}，运行3次计算...");
        _output.WriteLine("");

        // 预热JIT编译和内存分配
        _output.WriteLine("🔥 预热阶段：进行JIT编译和内存预分配...");
        var warmupBeatmap = beatmaps.First(); // 使用第一个谱面进行预热
        for (int i = 0; i < 3; i++)
        {
            SRCalculator.Instance.CalculateSR(warmupBeatmap, out _);
        }
        _output.WriteLine("✅ 预热完成");
        _output.WriteLine("");

        // 计算SR和时间 - 存储3次的结果
        var srsList = new List<double[]>(); // 每个谱面3次SR结果
        var timesList = new List<Dictionary<string, long>[]>(); // 每个谱面3次时间结果

        foreach (var bm in beatmaps)
        {
            var srs = new double[3];
            var times = new Dictionary<string, long>[3];
            for (var i = 0; i < 3; i++)
            {
                srs[i] = SRCalculator.Instance.CalculateSR(bm, out var timeDict);
                times[i] = timeDict;

                // 在计算之间添加小延迟，确保系统稳定
                if (i < 2) // 最后一次不需要延迟
                {
                    System.Threading.Thread.Sleep(50); // 50ms延迟
                }
            }
            srsList.Add(srs);
            timesList.Add(times);
        }

        // 使用10k谱面的数据作为代表
        var benchmarkIndex = Array.FindIndex(keyCounts, k => k == 10);
        if (benchmarkIndex == -1) benchmarkIndex = keyCounts.Length - 1; // 如果没有10k，用最后一个
        var benchmarkTimes = timesList[benchmarkIndex];
        var benchmarkSRs = srsList[benchmarkIndex];

        // 生成 ASCII 表格 - 显示3次计算的详细结果
        var sections = new[] { "Section232425", "Section2627", "Section3", "Total" };
        var displaySections = new[] { "Section23/24/25", "Section26/27", "Section3", "Total" };
        var colWidths = new[] { 8, 15, 11, 7, 5, 8 }; // 计算次数, Section23/24/25, Section26/27, Section3, Total, SR

        // 表头
        var header =
            $"| {"计算次数".PadRight(colWidths[0])} | {displaySections[0].PadRight(colWidths[1])} | {displaySections[1].PadRight(colWidths[2])} | {displaySections[2].PadRight(colWidths[3])} | {displaySections[3].PadRight(colWidths[4])} | {"SR".PadRight(colWidths[5])} |";
        var separator = $"+{string.Join("+", colWidths.Select(w => new string('-', w + 2)))}+";

        _output.WriteLine(separator);
        _output.WriteLine(header);
        _output.WriteLine(separator);

        // 3行数据 - 每行显示一次计算的结果
        for (int run = 0; run < 3; run++)
        {
            var runTimes = sections.Select(s =>
                    benchmarkTimes[run].GetValueOrDefault(s, 0).ToString("F1").PadLeft(colWidths[Array.IndexOf(sections, s) + 1]))
                .ToArray();
            var srStr = benchmarkSRs[run].ToString($"F{SR_DECIMAL_PLACES}").PadLeft(colWidths[5]);
            _output.WriteLine(
                $"| {$"第{run + 1}次".PadRight(colWidths[0])} | {runTimes[0]} | {runTimes[1]} | {runTimes[2]} | {runTimes[3]} | {srStr} |");
        }

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

        // SR 行 - 显示3次计算的结果
        for (int run = 0; run < 3; run++)
        {
            var srValues = srsList.Select(srs => srs[run]).ToArray();
            var srRow = $"| {$"SR{run + 1}".PadRight(detailColWidths[0])} | {string.Join(" | ", srValues.Select(sr => sr.ToString($"F{SR_DECIMAL_PLACES}").PadLeft(detailColWidths[1])))} |";
            _output.WriteLine(srRow);
        }

        // 总用时 行 - 显示3次计算的时间
        for (int run = 0; run < 3; run++)
        {
            var timeValues = timesList.Select(times => times[run].GetValueOrDefault("Total", 0)).ToArray();
            var timeRow = $"| {$"总用时{run + 1}".PadRight(detailColWidths[0])} | {string.Join(" | ", timeValues.Select(t => t.ToString("F1").PadLeft(detailColWidths[1])))} |";
            _output.WriteLine(timeRow);
        }

        _output.WriteLine(detailSeparator);
        _output.WriteLine("");

        // 验证SR结果合理性
        foreach (var srsArray in srsList)
        {
            foreach (var sr in srsArray)
            {
                Assert.True(sr >= 0, $"SR值不能为负: {sr}");
                Assert.True(sr <= 100, $"SR值过高: {sr}"); // 合理的上限
            }
        }

        _output.WriteLine($"✅ SR结果合理性验证通过");

        // 验证times包含预期键 (只对有效SR验证)
        var expectedKeys = new[] { "Section232425", "Section2627", "Section3", "Total" };
        for (int i = 0; i < srsList.Count; i++)
        {
            var srsArray = srsList[i];
            var timesArray = timesList[i];
            // 检查是否有有效的SR值
            if (srsArray.Any(sr => sr > 0)) // 只验证有效SR的times字典 (SR=-1表示不支持的键数)
            {
                foreach (var key in expectedKeys)
                {
                    Assert.True(timesArray[0].ContainsKey(key), $"times字典缺少键: {key} (谱面{i})");
                    Assert.True(timesArray[0][key] >= 0, $"时间值不能为负: {key} = {timesArray[0][key]}");
                }
            }
        }

                _output.WriteLine($"✅ times字典验证通过");
    }

    [Fact]
    public void CompareParsingStrategies()
    {
        // 加载测试谱面
        var resourcePath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", "..", "Resource");
        var osuFiles = Directory.GetFiles(resourcePath, "*.osu")
            .Where(f => !f.Contains("encoded_output"))
            .OrderBy(f => Path.GetFileName(f))
            .ToArray();

        var beatmap = new LegacyBeatmapDecoder().Decode(osuFiles.First()); // 使用第一个谱面测试
        var cs = (int)beatmap.BeatmapInfo.Difficulty.CircleSize;
        var od = beatmap.BeatmapInfo.Difficulty.OverallDifficulty;

        const int iterations = 1000; // 运行多次取平均值

        // 策略1: 当前方式 - 使用List收集然后ToArray
        double strategy1Total = 0;
        for (int i = 0; i < iterations; i++)
        {
            var stopwatch = Stopwatch.StartNew();

            var noteSequence = new List<SRsNote>();
            foreach (var hitObject in beatmap.HitObjects)
            {
                int col = hitObject is ManiaHitObject maniaHit ? maniaHit.Column : (int)Math.Floor(hitObject.Position.X * cs / 512.0);
                var time = (int)hitObject.StartTime;
                var tail = hitObject.EndTime > hitObject.StartTime ? (int)hitObject.EndTime : -1;
                noteSequence.Add(new SRsNote(col, time, tail));
            }
            var noteSeq = noteSequence.ToArray();

            stopwatch.Stop();
            strategy1Total += stopwatch.Elapsed.TotalMilliseconds;
        }

        // 策略2: 优化方式 - 预分配Array直接填充
        double strategy2Total = 0;
        for (int i = 0; i < iterations; i++)
        {
            var stopwatch = Stopwatch.StartNew();

            var noteSeq = new SRsNote[beatmap.HitObjects.Count];
            int index = 0;
            foreach (var hitObject in beatmap.HitObjects)
            {
                int col = hitObject is ManiaHitObject maniaHit ? maniaHit.Column : (int)Math.Floor(hitObject.Position.X * cs / 512.0);
                var time = (int)hitObject.StartTime;
                var tail = hitObject.EndTime > hitObject.StartTime ? (int)hitObject.EndTime : -1;
                noteSeq[index++] = new SRsNote(col, time, tail);
            }

            stopwatch.Stop();
            strategy2Total += stopwatch.Elapsed.TotalMilliseconds;
        }

        var strategy1Avg = strategy1Total / iterations;
        var strategy2Avg = strategy2Total / iterations;
        var improvement = ((strategy1Avg - strategy2Avg) / strategy1Avg) * 100;

        _output.WriteLine("=== 解析策略性能对比 ===");
        _output.WriteLine($"谱面: {Path.GetFileName(osuFiles.First())}");
        _output.WriteLine($"Note数量: {beatmap.HitObjects.Count}");
        _output.WriteLine($"测试迭代次数: {iterations}");
        _output.WriteLine("");
        _output.WriteLine($"策略1 (List收集+ToArray): {strategy1Avg:F4}ms 平均");
        _output.WriteLine($"策略2 (预分配Array):     {strategy2Avg:F4}ms 平均");
        _output.WriteLine($"性能提升: {improvement:F2}%");
        _output.WriteLine("");

        // 验证两种策略产生相同结果
        var noteSequence1 = new List<SRsNote>();
        foreach (var hitObject in beatmap.HitObjects)
        {
            int col = hitObject is ManiaHitObject maniaHit ? maniaHit.Column : (int)Math.Floor(hitObject.Position.X * cs / 512.0);
            var time = (int)hitObject.StartTime;
            var tail = hitObject.EndTime > hitObject.StartTime ? (int)hitObject.EndTime : -1;
            noteSequence1.Add(new SRsNote(col, time, tail));
        }
        var result1 = noteSequence1.ToArray();

        var result2 = new SRsNote[beatmap.HitObjects.Count];
        int idx = 0;
        foreach (var hitObject in beatmap.HitObjects)
        {
            int col = hitObject is ManiaHitObject maniaHit ? maniaHit.Column : (int)Math.Floor(hitObject.Position.X * cs / 512.0);
            var time = (int)hitObject.StartTime;
            var tail = hitObject.EndTime > hitObject.StartTime ? (int)hitObject.EndTime : -1;
            result2[idx++] = new SRsNote(col, time, tail);
        }

        Assert.Equal(result1.Length, result2.Length);
        for (int i = 0; i < result1.Length; i++)
        {
            Assert.Equal(result1[i].Index, result2[i].Index);
            Assert.Equal(result1[i].StartTime, result2[i].StartTime);
            Assert.Equal(result1[i].EndTime, result2[i].EndTime);
        }

        _output.WriteLine("✅ 两种策略结果一致性验证通过");
    }

    [Fact]
    public void CompareDirectSRCalculationVsExtensionMethodPerformance()
    {
        // 加载测试谱面
        var resourcePath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", "..", "Resource");
        var osuFiles = Directory.GetFiles(resourcePath, "*.osu")
            .Where(f => !f.Contains("encoded_output"))
            .OrderBy(f => Path.GetFileName(f))
            .ToArray();

        if (osuFiles.Length < 1)
        {
            _output.WriteLine("⚠️  需要至少1个测试谱面文件，跳过性能对比测试");
            return;
        }

        // 选择所有可用谱面进行测试
        var testFiles = osuFiles.ToArray();

        _output.WriteLine($"=== SR计算性能对比测试 ===");
        _output.WriteLine($"测试谱面数量: {testFiles.Length} 个");
        _output.WriteLine("");

        const int iterations = 5; // 每个谱面测试多次取平均值

        // 预热JIT编译
        _output.WriteLine("🔥 预热阶段...");
        using (var warmupStream = File.OpenRead(testFiles[0]))
        {
            new LegacyBeatmapDecoder(false).Decode(warmupStream);
        }
        using (var warmupStream = File.OpenRead(testFiles[0]))
        {
            new LegacyBeatmapDecoder(true).Decode(warmupStream);
        }
        _output.WriteLine("✅ 预热完成");
        _output.WriteLine("");

        // ===== 单谱面测试 =====
        _output.WriteLine("📊 第一阶段：单谱面完整流程对比");
        var singleDirectTimes = new List<long>();
        var singleExtensionTimes = new List<long>();

        var testFile = testFiles[0]; // 使用第一个谱面
        var fileName = Path.GetFileName(testFile);
        _output.WriteLine($"测试谱面: {fileName}");

        // 测试直接SR计算：解析谱面 + 直接计算SR
        for (int i = 0; i < iterations; i++)
        {
            var stopwatch = Stopwatch.StartNew();

            // 从路径开始：解析谱面 + 直接计算SR
            using var stream = File.OpenRead(testFile);
            var beatmap = new LegacyBeatmapDecoder(false).Decode(stream);
            var sr = SRCalculator.Instance.CalculateSR(beatmap, out _);

            stopwatch.Stop();
            singleDirectTimes.Add(stopwatch.ElapsedTicks);
        }

        // 测试扩展方法获取：解析谱面 + 调用扩展方法 + 获取SR值
        for (int i = 0; i < iterations; i++)
        {
            var stopwatch = Stopwatch.StartNew();

            // 从路径开始：解析谱面 + 调用扩展方法 + 获取SR值
            using var stream = File.OpenRead(testFile);
            var beatmap = new LegacyBeatmapDecoder(false).Decode(stream);
            beatmap.CalculateAnalysisData(true); // 调用扩展方法计算分析数据
            var sr = beatmap.AnalysisData.StarRating ?? 0.0; // 获取SR值

            stopwatch.Stop();
            singleExtensionTimes.Add(stopwatch.ElapsedTicks);
        }

        var singleDirectAvg = singleDirectTimes.Average() * 1000.0 / Stopwatch.Frequency;
        var singleExtensionAvg = singleExtensionTimes.Average() * 1000.0 / Stopwatch.Frequency;
        var singleSpeedup = singleDirectAvg / singleExtensionAvg;

        _output.WriteLine($"直接SR计算（解析+计算SR）: {singleDirectAvg:F2} ms");
        _output.WriteLine($"扩展方法获取（解析+扩展+获取SR）: {singleExtensionAvg:F2} ms");
        _output.WriteLine($"🚀 单谱面性能对比: {singleSpeedup:F1}x 倍速（{(singleSpeedup > 1.0 ? "直接计算更快" : "扩展方法更快")}）");
        _output.WriteLine("");

        // ===== 多谱面测试 =====
        _output.WriteLine($"📊 第二阶段：{testFiles.Length}个谱面完整流程对比");
        var multiDirectTimes = new List<long>();
        var multiExtensionTimes = new List<long>();

        // 测试直接SR计算：对所有谱面进行解析+直接计算SR
        for (int i = 0; i < iterations; i++)
        {
            var stopwatch = Stopwatch.StartNew();

            foreach (var file in testFiles)
            {
                using var stream = File.OpenRead(file);
                var beatmap = new LegacyBeatmapDecoder(false).Decode(stream);
                var sr = SRCalculator.Instance.CalculateSR(beatmap, out _);
            }

            stopwatch.Stop();
            multiDirectTimes.Add(stopwatch.ElapsedTicks);
        }

        // 测试扩展方法获取：对所有谱面进行解析+调用扩展方法+获取SR值
        for (int i = 0; i < iterations; i++)
        {
            var stopwatch = Stopwatch.StartNew();

            foreach (var file in testFiles)
            {
                using var stream = File.OpenRead(file);
                var beatmap = new LegacyBeatmapDecoder(false).Decode(stream);
                beatmap.CalculateAnalysisData(true); // 调用扩展方法
                var sr = beatmap.AnalysisData.StarRating ?? 0.0; // 获取SR值
            }

            stopwatch.Stop();
            multiExtensionTimes.Add(stopwatch.ElapsedTicks);
        }

        var multiDirectAvg = multiDirectTimes.Average() * 1000.0 / Stopwatch.Frequency;
        var multiExtensionAvg = multiExtensionTimes.Average() * 1000.0 / Stopwatch.Frequency;
        var multiSpeedup = multiDirectAvg / multiExtensionAvg;

        _output.WriteLine($"直接SR计算（{testFiles.Length}谱面 解析+计算SR）: {multiDirectAvg:F2} ms");
        _output.WriteLine($"扩展方法获取（{testFiles.Length}谱面 解析+扩展+获取SR）: {multiExtensionAvg:F2} ms");
        _output.WriteLine($"🚀 多谱面性能对比: {multiSpeedup:F1}x 倍速（{(multiSpeedup > 1.0 ? "直接计算更快" : "扩展方法更快")}）");
        _output.WriteLine("");

        // ===== 详细时间分析 =====
        _output.WriteLine("📊 第三阶段：详细时间分解分析");

        // 分析直接SR计算的时间分布
        var directParseTimes = new List<long>();
        var directSrTimes = new List<long>();

        for (int i = 0; i < iterations; i++)
        {
            using var stream = File.OpenRead(testFile);
            var parseStopwatch = Stopwatch.StartNew();
            var beatmap = new LegacyBeatmapDecoder(false).Decode(stream);
            parseStopwatch.Stop();
            directParseTimes.Add(parseStopwatch.ElapsedTicks);

            var srStopwatch = Stopwatch.StartNew();
            var sr = SRCalculator.Instance.CalculateSR(beatmap, out _);
            srStopwatch.Stop();
            directSrTimes.Add(srStopwatch.ElapsedTicks);
        }

        // 分析扩展方法获取的时间分布
        var extensionParseTimes = new List<long>();
        var extensionSetupTimes = new List<long>();
        var extensionSrAccessTimes = new List<long>();

        for (int i = 0; i < iterations; i++)
        {
            using var stream = File.OpenRead(testFile);
            var parseStopwatch = Stopwatch.StartNew();
            var beatmap = new LegacyBeatmapDecoder(false).Decode(stream);
            parseStopwatch.Stop();
            extensionParseTimes.Add(parseStopwatch.ElapsedTicks);

            var setupStopwatch = Stopwatch.StartNew();
            beatmap.CalculateAnalysisData(true); // 调用扩展方法
            setupStopwatch.Stop();
            extensionSetupTimes.Add(setupStopwatch.ElapsedTicks);

            var srAccessStopwatch = Stopwatch.StartNew();
            var sr = beatmap.AnalysisData.StarRating ?? 0.0; // 获取SR值
            srAccessStopwatch.Stop();
            extensionSrAccessTimes.Add(srAccessStopwatch.ElapsedTicks);
        }

        var directParseAvg = directParseTimes.Average() * 1000.0 / Stopwatch.Frequency;
        var directSrAvg = directSrTimes.Average() * 1000.0 / Stopwatch.Frequency;
        var extensionParseAvg = extensionParseTimes.Average() * 1000.0 / Stopwatch.Frequency;
        var extensionSetupAvg = extensionSetupTimes.Average() * 1000.0 / Stopwatch.Frequency;
        var extensionSrAccessAvg = extensionSrAccessTimes.Average() * 1000.0 / Stopwatch.Frequency;

        _output.WriteLine($"直接SR计算 - 解析时间: {directParseAvg:F2} ms");
        _output.WriteLine($"直接SR计算 - SR计算时间: {directSrAvg:F2} ms");
        _output.WriteLine($"直接SR计算 - 总时间: {directParseAvg + directSrAvg:F2} ms");
        _output.WriteLine("");
        _output.WriteLine($"扩展方法获取 - 解析时间: {extensionParseAvg:F2} ms");
        _output.WriteLine($"扩展方法获取 - 扩展设置时间: {extensionSetupAvg:F2} ms");
        _output.WriteLine($"扩展方法获取 - SR访问时间: {extensionSrAccessAvg:F2} ms");
        _output.WriteLine($"扩展方法获取 - 总时间: {extensionParseAvg + extensionSetupAvg + extensionSrAccessAvg:F2} ms");
        _output.WriteLine("");
        _output.WriteLine($"解析时间差距: {extensionParseAvg - directParseAvg:F2} ms");
        _output.WriteLine($"SR处理时间差距: {extensionSetupAvg + extensionSrAccessAvg - directSrAvg:F2} ms");
        _output.WriteLine("");

        // ===== 最终总结 =====
        _output.WriteLine("📊 最终总结");
        _output.WriteLine("扩展方法的特点：");
        _output.WriteLine($"• 解析时间开销: +{extensionParseAvg - directParseAvg:F1} ms");
        _output.WriteLine($"• 扩展设置开销: {extensionSetupAvg:F1} ms（准备数据结构）");
        _output.WriteLine($"• SR访问性能: {directSrAvg / extensionSrAccessAvg:F1}x 倍速提升（延迟计算）");
        _output.WriteLine($"• 单谱面处理: {singleSpeedup:F1}x 倍速（{(singleSpeedup > 1.0 ? "直接计算更快" : "扩展方法更快")}）");
        _output.WriteLine($"• 多谱面处理: {multiSpeedup:F1}x 倍速（{(multiSpeedup > 1.0 ? "直接计算更快" : "扩展方法更快")}）");
        _output.WriteLine("");
        _output.WriteLine("💡 适用场景建议：");
        _output.WriteLine("• 单次SR计算：建议使用直接计算（开销最小）");
        _output.WriteLine("• 需要缓存SR值：建议使用扩展方法（延迟计算，避免重复计算）");
        _output.WriteLine("• 批量处理谱面：扩展方法更适合（数据结构复用）");

        // 验证SR值一致性（使用第一个谱面）
        double directSR;
        using (var stream = File.OpenRead(testFile))
        {
            var beatmap = new LegacyBeatmapDecoder(false).Decode(stream);
            directSR = SRCalculator.Instance.CalculateSR(beatmap, out _);
            _output.WriteLine($"直接SR计算值: {directSR:F4}");
        }

        double extensionSR;
        using (var stream = File.OpenRead(testFile))
        {
            var beatmap = new LegacyBeatmapDecoder(false).Decode(stream);
            beatmap.CalculateAnalysisData(true);
            extensionSR = beatmap.AnalysisData.StarRating ?? 0.0000;
            _output.WriteLine($"扩展方法获取值: {extensionSR:F4}");
            _output.WriteLine($"扩展方法IsPrecomputed: {beatmap.AnalysisData.IsPrecomputed}");
            _output.WriteLine($"扩展方法SRsNotes: {(beatmap.AnalysisData.SRsNotes != null ? beatmap.AnalysisData.SRsNotes.Length.ToString() : "null")}");
        }

        Assert.Equal(directSR, extensionSR, SR_DECIMAL_PLACES);

        _output.WriteLine("✅ SR值一致性验证通过");
        _output.WriteLine("✅ 性能对比测试完成");
    }
}
