using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading;
using LAsOsuBeatmapParser.Analysis;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Beatmaps.Formats;
using LAsOsuBeatmapParser.Extensions;
using LAsOsuBeatmapParser.Tests.AnalysisSR;
using Xunit;
using Xunit.Abstractions;

namespace LAsOsuBeatmapParser.Tests
{
    /// <summary>
    ///     不同版本C# SR算法测试
    ///     测试单一文件计算一次，多个文件计算一次
    /// </summary>
    public class VersionCSRTests
    {
        // SR显示精度控制常量
        private const int SR_DECIMAL_PLACES = 4;

        // 测试文件路径
        private static readonly string TestResourceDir = Path.Combine(
            AppDomain.CurrentDomain.BaseDirectory,
            "..", "..", "..", "Resource"
        );

        private static readonly string SingleTestFile = Path.Combine(
            TestResourceDir,
            "Glen Check - 60's Cardin (SK_la) [Insane].osu"
        );

        private static readonly string[] MultipleTestFiles = Directory.GetFiles(TestResourceDir, "*.osu")
                                                                      .Where(f => !f.Contains("encoded_output"))
                                                                      .ToArray();
        private readonly ITestOutputHelper _output;

        public VersionCSRTests(ITestOutputHelper output)
        {
            _output = output;
        }

        [Fact]
        public void TestVersionCSRSingleFile_Once()
        {
            // Arrange
            Assert.True(File.Exists(SingleTestFile), $"Test file not found: {SingleTestFile}");
            var decoder = new LegacyBeatmapDecoder();
            Beatmap beatmap = decoder.Decode(SingleTestFile);

            _output.WriteLine("=== 不同版本C# SR算法 - 单一文件测试 ===");
            _output.WriteLine($"CS值: {beatmap.BeatmapInfo.Difficulty.CircleSize}");
            _output.WriteLine($"谱面信息: {beatmap.BeatmapInfo.Metadata.Artist} - {beatmap.BeatmapInfo.Metadata.Title} [{beatmap.BeatmapInfo.Metadata.Version}]");
            _output.WriteLine($"键数: {(int)beatmap.BeatmapInfo.Difficulty.CircleSize}k");
            _output.WriteLine("");

            // 预热JIT
            SRCalculator.Instance.CalculateSR(beatmap, out _);
            GC.Collect();
            GC.WaitForPendingFinalizers();

            var results = new List<(string version, double sr, long timeMs, long memoryBytes)>();

            // 定义不同版本的计算器
            var calculators = new (string name, Func<Beatmap, double> calculator)[]
            {
                ("Current    ", bm => SRCalculator.Instance.CalculateSR(bm, out _)),
                ("V3.0       ", bm => SRCalculatorV30.Instance.CalculateSR(bm, out _)),
                ("V2.3       ", CalculateWithV23)
            };

            // Act - 每个版本计算一次
            foreach ((string name, Func<Beatmap, double> calculator) in calculators)
            {
                long initialMemory = GC.GetAllocatedBytesForCurrentThread();

                var stopwatch = Stopwatch.StartNew();
                double sr = calculator(beatmap);
                stopwatch.Stop();

                long finalMemory = GC.GetAllocatedBytesForCurrentThread();
                long memoryUsed = finalMemory - initialMemory;

                results.Add((name, sr, stopwatch.ElapsedMilliseconds, memoryUsed));

                // 计算间隔延迟
                Thread.Sleep(50);
            }

            // 输出表格对比
            _output.WriteLine("=== SR值表格对比 ===");
            var table = new StringBuilder();
            table.AppendLine("| Version     | SR     | ms  | MB  |");

            foreach ((string version, double sr, long timeMs, long memoryBytes) in results)
            {
                double memoryMB = memoryBytes / (1024.0 * 1024.0);
                table.AppendLine($"| {version} | {sr:F4} | {timeMs} | {memoryMB:F2} |");
            }

            _output.WriteLine(table.ToString());

            // Assert - 验证结果合理性
            foreach ((string version, double sr, long _, long _) in results)
            {
                Assert.True(sr >= 0, $"SR值不能为负 ({version}): {sr}");
                Assert.True(sr <= 10, $"SR值过高 ({version}): {sr}");
            }

            _output.WriteLine("✅ 单一文件不同版本测试完成");
        }

        [Fact]
        public void TestVersionCSRSingleFile_MultipleFiles()
        {
            // Arrange
            var decoder = new LegacyBeatmapDecoder();

            _output.WriteLine("=== 不同版本C# SR算法 - 多个文件测试 ===");
            _output.WriteLine($"测试文件数量: {MultipleTestFiles.Length}");
            _output.WriteLine("");

            // 预热JIT
            Beatmap warmupBeatmap = decoder.Decode(MultipleTestFiles.First());
            SRCalculator.Instance.CalculateSR(warmupBeatmap, out _);
            GC.Collect();
            GC.WaitForPendingFinalizers();

            var results = new List<(double cs, string version, double sr, long timeMs, long memoryBytes)>();

            // 定义不同版本的计算器
            var calculators = new (string name, Func<Beatmap, double> calculator)[]
            {
                ("BP   ", bm => SRCalculator.Instance.CalculateSR(bm, out _)),
                ("V3.0  ", bm => SRCalculatorV30.Instance.CalculateSR(bm, out _)),
                ("V2.3  ", CalculateWithV23)
            };

            // Act - 每个文件每个版本计算一次
            foreach (string filePath in MultipleTestFiles)
            {
                Beatmap beatmap = decoder.Decode(filePath);
                string fileName = Path.GetFileName(filePath);

                foreach ((string versionName, Func<Beatmap, double> calculator) in calculators)
                {
                    long initialMemory = GC.GetAllocatedBytesForCurrentThread();

                    var stopwatch = Stopwatch.StartNew();
                    double sr = calculator(beatmap);
                    stopwatch.Stop();

                    long finalMemory = GC.GetAllocatedBytesForCurrentThread();
                    long memoryUsed = finalMemory - initialMemory;

                    results.Add((beatmap.BeatmapInfo.Difficulty.CircleSize, versionName, sr, stopwatch.ElapsedMilliseconds, memoryUsed));

                    // 计算间隔延迟
                    Thread.Sleep(10);
                }
            }

            // Assert - 验证结果合理性
            foreach ((double cs, string version, double sr, long _, long _) in results)
            {
                Assert.True(sr >= 0, $"SR值不能为负 (CS {cs} - {version}): {sr}");
                Assert.True(sr <= 10, $"SR值过高 (CS {cs} - {version}): {sr}");
            }

            // 输出表格对比
            _output.WriteLine("=== SR值表格对比 ===");
            IOrderedEnumerable<IGrouping<double, (double cs, string version, double sr, long timeMs, long memoryBytes)>> fileGroups = results.GroupBy(r => r.cs).OrderBy(g => g.Key);
            string[] versions = calculators.Select(c => c.name).ToArray();

            // 构建表格
            var table = new StringBuilder();
            table.Append("| CS  ");
            foreach (string version in versions) table.Append($"| {version} ");
            table.AppendLine("|");

            // 表内容
            foreach (IGrouping<double, (double cs, string version, double sr, long timeMs, long memoryBytes)> fileGroup in fileGroups)
            {
                string csStr = fileGroup.Key.ToString("F1");
                table.Append($"| {csStr} ");

                foreach (string version in versions)
                {
                    (double cs, string version, double sr, long timeMs, long memoryBytes) result = fileGroup.FirstOrDefault(r => r.version == version);
                    string srStr = result != default ? $"{result.sr:F4}" : "N/A";
                    table.Append($"| {srStr} ");
                }

                table.AppendLine("|");
            }

            _output.WriteLine(table.ToString());

            // 计算统计信息
            IEnumerable<IGrouping<string, (double cs, string version, double sr, long timeMs, long memoryBytes)>> versionGroups = results.GroupBy(r => r.version);

            foreach (IGrouping<string, (double cs, string version, double sr, long timeMs, long memoryBytes)> group in versionGroups)
            {
                double avgSR = group.Average(r => r.sr);
                double avgTime = group.Average(r => r.timeMs);
                double avgMemory = group.Average(r => r.memoryBytes);

                _output.WriteLine($"{group.Key} 统计:");
                _output.WriteLine($"  平均SR: {avgSR:F4}");
                _output.WriteLine($"  平均时间: {avgTime:F2}ms");
                _output.WriteLine($"  平均内存使用: {avgMemory:F0} bytes ({avgMemory / (1024.0 * 1024.0):F2} MB)");
                _output.WriteLine("");
            }

            _output.WriteLine("✅ 多个文件不同版本测试完成");
        }

        // 辅助方法：使用V2.3版本计算
        private double CalculateWithV23(Beatmap beatmap)
        {
            var calculator = new SRCalculatorV23();
            var noteSequence = new List<SRsNote>();
            int keyCount = (int)beatmap.BeatmapInfo.Difficulty.CircleSize;
            double od = beatmap.BeatmapInfo.Difficulty.OverallDifficulty;

            foreach (HitObject hitObject in beatmap.HitObjects)
            {
                int col = hitObject is ManiaHitObject maniaHit ? maniaHit.Column : ManiaExtensions.GetColumnFromX(keyCount, hitObject.Position.X);
                int time = (int)hitObject.StartTime;
                int tail = hitObject.EndTime > hitObject.StartTime ? (int)hitObject.EndTime : -1;
                noteSequence.Add(new SRsNote(col, time, tail));
            }

            return calculator.Calculate(noteSequence, keyCount, od, out _);
        }
    }
}
