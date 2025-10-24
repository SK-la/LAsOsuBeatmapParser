using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using LAsOsuBeatmapParser.Analysis;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Beatmaps.Formats;
using Xunit;
using Xunit.Abstractions;

namespace LAsOsuBeatmapParser.Tests
{
    public class SRCalculatorPerformanceTests
    {
        private readonly ITestOutputHelper _output;

        // SR显示精度控制常量
        private const int SR_DECIMAL_PLACES = 4;

        public SRCalculatorPerformanceTests(ITestOutputHelper output)
        {
            _output = output;
        }

        [Fact]
        public void RunDetailedPerformanceAnalysis()
        {
            // 加载真实的谱面文件 (4k-10k)
            string resourcePath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", "..", "Resource");
            string[] osuFiles = Directory.GetFiles(resourcePath, "*.osu")
                                         .Where(f => !f.Contains("encoded_output")) // 排除编码输出文件
                                         .OrderBy(f => Path.GetFileName(f))
                                         .ToArray();

            Beatmap[] beatmaps = osuFiles.Select(f => new LegacyBeatmapDecoder().Decode(f)).ToArray();
            int[] keyCounts = beatmaps.Select(bm => (int)bm.BeatmapInfo.Difficulty.CircleSize).ToArray();

            _output.WriteLine($"=== SR计算详细性能分析 (4k-10k) 队列{beatmaps.Length}，运行3次计算...");
            _output.WriteLine("");

            // 预热JIT编译和内存分配
            _output.WriteLine("🔥 预热阶段：进行JIT编译和内存预分配...");
            Beatmap warmupBeatmap = beatmaps.First(); // 使用第一个谱面进行预热
            for (int i = 0; i < 3; i++) SRCalculator.Instance.CalculateSR(warmupBeatmap, out _);
            _output.WriteLine("✅ 预热完成");
            _output.WriteLine("");

            // 计算SR和时间 - 存储3次的结果
            var srsList = new List<double[]>(); // 每个谱面3次SR结果
            var timesList = new Dictionary<string, long>[beatmaps.Length][]; // 每个谱面3次时间结果

            foreach (Beatmap bm in beatmaps)
            {
                double[] srs = new double[3];
                var times = new Dictionary<string, long>[3];

                for (int i = 0; i < 3; i++)
                {
                    srs[i] = SRCalculator.Instance.CalculateSR(bm, out Dictionary<string, long> timeDict);
                    times[i] = timeDict;

                    // 在计算之间添加小延迟，确保系统稳定
                    if (i < 2) // 最后一次不需要延迟
                        System.Threading.Thread.Sleep(50); // 50ms延迟
                }

                srsList.Add(srs);
                timesList[Array.IndexOf(beatmaps, bm)] = times;
            }

            // 使用10k谱面的数据作为代表
            int benchmarkIndex = Array.FindIndex(keyCounts, k => k == 10);
            if (benchmarkIndex == -1) benchmarkIndex = keyCounts.Length - 1; // 如果没有10k，用最后一个
            Dictionary<string, long>[] benchmarkTimes = timesList[benchmarkIndex];
            double[] benchmarkSRs = srsList[benchmarkIndex];

            // 生成 ASCII 表格 - 显示3次计算的详细结果
            string[] sections = new[] { "Section232425", "Section2627", "Section3", "Total" };
            string[] displaySections = new[] { "Section23/24/25", "Section26/27", "Section3", "Total" };
            int[] colWidths = new[] { 8, 15, 11, 7, 5, 8 }; // 计算次数, Section23/24/25, Section26/27, Section3, Total, SR

            // 表头
            string header =
                $"| {"计算次数".PadRight(colWidths[0])} | {displaySections[0].PadRight(colWidths[1])} | {displaySections[1].PadRight(colWidths[2])} | {displaySections[2].PadRight(colWidths[3])} | {displaySections[3].PadRight(colWidths[4])} | {"SR".PadRight(colWidths[5])} |";
            string separator = $"+{string.Join("+", colWidths.Select(w => new string('-', w + 2)))}+";

            _output.WriteLine(separator);
            _output.WriteLine(header);
            _output.WriteLine(separator);

            // 3行数据 - 每行显示一次计算的结果
            for (int run = 0; run < 3; run++)
            {
                string[] runTimes = sections.Select(s =>
                {
                    long totalMs = benchmarkTimes[run].GetValueOrDefault(s, 0);
                    return totalMs > 0 ? $"{totalMs}ms" : "-";
                }).ToArray();

                string srValue = $"{benchmarkSRs[run]:F4}";
                string row = $"| {"第" + (run + 1) + "次".PadRight(colWidths[0])} | {runTimes[0].PadRight(colWidths[1])} | {runTimes[1].PadRight(colWidths[2])} | {runTimes[2].PadRight(colWidths[3])} | {runTimes[3].PadRight(colWidths[4])} | {srValue.PadRight(colWidths[5])} |";
                _output.WriteLine(row);
            }

            _output.WriteLine(separator);

            // 计算平均值和标准差
            double avgSR = benchmarkSRs.Average();
            double stdDevSR = Math.Sqrt(benchmarkSRs.Select(sr => Math.Pow(sr - avgSR, 2)).Average());

            var avgTimes = new Dictionary<string, double>();
            var stdDevTimes = new Dictionary<string, double>();

            foreach (string section in sections)
            {
                double[] sectionTimes = benchmarkTimes.Select(t => (double)t.GetValueOrDefault(section, 0)).ToArray();
                avgTimes[section] = sectionTimes.Average();
                stdDevTimes[section] = Math.Sqrt(sectionTimes.Select(t => Math.Pow(t - avgTimes[section], 2)).Average());
            }

            _output.WriteLine("");
            _output.WriteLine("=== 统计汇总 ===");
            _output.WriteLine($"SR平均值: {avgSR:F4} ± {stdDevSR:F4}");

            foreach (string section in sections)
            {
                _output.WriteLine($"{displaySections[Array.IndexOf(sections, section)]}平均时间: {avgTimes[section]:F2}ms ± {stdDevTimes[section]:F2}ms");
            }

            // 验证SR值的一致性
            double srVariance = benchmarkSRs.Select(sr => Math.Pow(sr - avgSR, 2)).Sum() / benchmarkSRs.Length;
            _output.WriteLine($"SR方差: {srVariance:F8}");

            if (srVariance < 1e-10) // 非常小的方差表示SR计算稳定
            {
                _output.WriteLine("✅ SR计算结果稳定");
            }
            else
            {
                _output.WriteLine("⚠️  SR计算结果存在波动，可能存在内存累积或其他问题");
            }

            // 验证时间字典的完整性
            bool timesValid = benchmarkTimes.All(t => sections.All(s => t.ContainsKey(s)));
            if (timesValid)
            {
                _output.WriteLine("✅ times字典验证通过");
            }
            else
            {
                _output.WriteLine("❌ times字典验证失败");
            }
        }
    }
}
