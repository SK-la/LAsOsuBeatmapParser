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
    ///     C#+Rust所有SR算法对比测试
    ///     测试单一文件计算一次，多个文件计算一次
    /// </summary>
    public class ComparisonTests
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

        private static readonly string[]          MultipleTestFiles = Directory.GetFiles(TestResourceDir, "*.osu");
        private readonly        ITestOutputHelper _output;

        public ComparisonTests(ITestOutputHelper output)
        {
            _output = output;
        }

        [Fact]
        public void TestComparisonSingleFile_Once()
        {
            // Arrange
            Assert.True(File.Exists(SingleTestFile), $"Test file not found: {SingleTestFile}");
            var     decoder = new LegacyBeatmapDecoder();
            Beatmap beatmap = decoder.Decode(SingleTestFile);

            _output.WriteLine("=== C#+Rust SR算法对比 - 单一文件测试 ===");
            _output.WriteLine($"CS值: {beatmap.BeatmapInfo.Difficulty.CircleSize}");
            _output.WriteLine($"谱面信息: {beatmap.BeatmapInfo.Metadata.Artist} - {beatmap.BeatmapInfo.Metadata.Title} [{beatmap.BeatmapInfo.Metadata.Version}]");
            _output.WriteLine($"键数: {(int)beatmap.BeatmapInfo.Difficulty.CircleSize}k");
            _output.WriteLine($"C# parsed hit objects: {beatmap.HitObjects.Count}");
            _output.WriteLine("");

            // 预热JIT
            SRCalculator.Instance.CalculateSR(beatmap, out _);
            GC.Collect();
            GC.WaitForPendingFinalizers();

            var results = new List<(string algorithm, double sr, long timeMs, long memoryBytes)>();

            // 定义所有算法
            var algorithms = new (string name, Func<Beatmap, double> calculator)[]
            {
                ("C# Current", bm => SRCalculator.Instance.CalculateSR(bm, out _)),
                ("C# FromFile", bm => CalculateSRFromFile(SingleTestFile)),
                ("C# FromContent", bm => CalculateSRFromContent(File.ReadAllText(SingleTestFile))),
                ("C# V3.0", bm => SRCalculatorV30.Instance.CalculateSR(bm, out _)),
                ("C# V2.3", CalculateWithV23),
                ("Rust FromFile", bm => SRCalculatorRust.CalculateSR_FromFile(SingleTestFile) ?? -1)
            };

            // Act - 每个算法计算一次
            foreach ((string name, Func<Beatmap, double> calculator) in algorithms)
            {
                long initialMemory = GC.GetAllocatedBytesForCurrentThread();

                var    stopwatch = Stopwatch.StartNew();
                double sr        = calculator(beatmap);
                stopwatch.Stop();

                long finalMemory = GC.GetAllocatedBytesForCurrentThread();
                long memoryUsed  = finalMemory - initialMemory;

                results.Add((name, sr, stopwatch.ElapsedMilliseconds, memoryUsed));

                // 计算间隔延迟
                Thread.Sleep(50);
            }

            // 输出表格
            _output.WriteLine("=== 单一文件算法对比表格 ===");
            var table = new StringBuilder();
            table.AppendLine("| 算法       | SR     | 时间(ms) | 内存(MB) |");
            table.AppendLine("|------------|--------|----------|----------|");

            foreach ((string algorithm, double sr, long timeMs, long memoryBytes) in results)
            {
                double memoryMB = memoryBytes / (1024.0 * 1024.0);
                table.AppendLine($"| {algorithm,-10} | {sr,6:F4} | {timeMs,8} | {memoryMB,8:F2} |");
                Console.WriteLine($"{algorithm}: SR={sr:F4}, Time={timeMs}ms, Memory={memoryMB:F2}MB");
            }

            _output.WriteLine(table.ToString());
            _output.WriteLine("");

            // Assert - 验证结果合理性
            foreach ((string algorithm, double sr, long _, long _) in results)
            {
                Assert.True(sr >= 0, $"SR值不能为负 ({algorithm}): {sr}");
                Assert.True(sr <= 10, $"SR值过高 ({algorithm}): {sr}");
            }

            _output.WriteLine("✅ 单一文件对比测试完成");
        }

        [Fact]
        public void TestComparisonMultipleFiles_Once()
        {
            // Arrange
            var decoder = new LegacyBeatmapDecoder();

            _output.WriteLine("=== C#+Rust SR算法对比 - 多个文件测试 ===");
            _output.WriteLine($"测试文件数量: {MultipleTestFiles.Length}");
            _output.WriteLine("");

            // 预热JIT
            Beatmap warmupBeatmap = decoder.Decode(MultipleTestFiles.First());
            SRCalculator.Instance.CalculateSR(warmupBeatmap, out _);
            GC.Collect();
            GC.WaitForPendingFinalizers();

            var results = new List<(double cs, string algorithm, double sr, long timeMs, long memoryBytes)>();

            // 定义所有算法
            var algorithms = new (string name, Func<Beatmap, string, double> calculator)[]
            {
                ("C# Current", (bm,       fp) => SRCalculator.Instance.CalculateSR(bm, out _)),
                ("C# V3.0", (bm,          fp) => SRCalculatorV30.Instance.CalculateSR(bm, out _)),
                ("C# V2.3", (bm,          fp) => CalculateWithV23(bm)),
                ("C# FromFile", (bm,      fp) => CalculateSRFromFile(fp)),
                ("C# FromContent", (bm,   fp) => CalculateSRFromContent(File.ReadAllText(fp))),
                ("Rust FromFile", (bm,    fp) => SRCalculatorRust.CalculateSR_FromFile(fp) ?? -1)
            };

            // Act - 每个文件每个算法计算一次
            foreach (string filePath in MultipleTestFiles)
            {
                Beatmap beatmap = decoder.Decode(filePath);

                foreach ((string algorithmName, Func<Beatmap, string, double> calculator) in algorithms)
                {
                    long initialMemory = GC.GetAllocatedBytesForCurrentThread();

                    var    stopwatch = Stopwatch.StartNew();
                    double sr        = calculator(beatmap, filePath);
                    stopwatch.Stop();

                    long finalMemory = GC.GetAllocatedBytesForCurrentThread();
                    long memoryUsed  = finalMemory - initialMemory;

                    results.Add((beatmap.BeatmapInfo.Difficulty.CircleSize, algorithmName, sr, stopwatch.ElapsedMilliseconds, memoryUsed));

                    // 计算间隔延迟
                    Thread.Sleep(10);
                }
            }

            // Assert - 验证结果合理性
            foreach ((double cs, string algorithm, double sr, long _, long _) in results)
            {
                Assert.True(sr >= 0, $"SR值不能为负 (CS{cs} - {algorithm}): {sr}");
                Assert.True(sr <= 10, $"SR值过高 (CS{cs} - {algorithm}): {sr}");
            }

            // 计算统计信息
            IEnumerable<IGrouping<string, (double cs, string algorithm, double sr, long timeMs, long memoryBytes)>> algorithmGroups = results.GroupBy(r => r.algorithm);

            // 输出统计表格
            _output.WriteLine("=== 多个文件算法统计表格 ===");
            var table = new StringBuilder();
            table.AppendLine("| 算法       | 平均SR | 平均时间(ms) | 平均内存(MB) |");
            table.AppendLine("|------------|--------|--------------|--------------|");

            foreach (IGrouping<string, (double cs, string algorithm, double sr, long timeMs, long memoryBytes)> group in algorithmGroups)
            {
                double avgSR     = group.Average(r => r.sr);
                double avgTime   = group.Average(r => r.timeMs);
                double avgMemory = group.Average(r => r.memoryBytes);
                double avgMemoryMB = avgMemory / (1024.0 * 1024.0);

                table.AppendLine($"| {group.Key,-10} | {avgSR,6:F4} | {avgTime,12:F2} | {avgMemoryMB,12:F2} |");
            }

            _output.WriteLine(table.ToString());
            _output.WriteLine("");

            _output.WriteLine("✅ 多个文件对比测试完成");
        }

        [Fact]
        public void TestComparisonEncodedOutputFile_Once()
        {
            // Arrange
            var     decoder = new LegacyBeatmapDecoder();
            Beatmap beatmap = decoder.Decode(SingleTestFile);

            _output.WriteLine($"=== C#+Rust SR算法对比 -.osu测试 ===");
            _output.WriteLine($"CS值: {beatmap.BeatmapInfo.Difficulty.CircleSize}");
            _output.WriteLine($"谱面信息: {beatmap.BeatmapInfo.Metadata.Artist} - {beatmap.BeatmapInfo.Metadata.Title} [{beatmap.BeatmapInfo.Metadata.Version}]");
            _output.WriteLine($"键数: {(int)beatmap.BeatmapInfo.Difficulty.CircleSize}k");
            _output.WriteLine("");

            // 预热JIT
            SRCalculator.Instance.CalculateSR(beatmap, out _);
            GC.Collect();
            GC.WaitForPendingFinalizers();

            var results = new List<(string algorithm, double sr, long timeMs, long memoryBytes)>();

            // 定义所有算法
            var algorithms = new (string name, Func<Beatmap, double> calculator)[]
            {
                ("C# F-File ", bm => CalculateSRFromFile(SingleTestFile)),
                ("C# Instance", bm => SRCalculator.Instance.CalculateSR(bm, out _)),
                ("C# F-Content", bm => CalculateSRFromContent(File.ReadAllText(SingleTestFile))),

                ("C# V3.0", bm => SRCalculatorV30.Instance.CalculateSR(bm, out _)),
                ("C# V2.3", CalculateWithV23),

                ("Rust F-File", bm => SRCalculatorRust.CalculateSR_FromFile(SingleTestFile) ?? -1)
            };

            // Act - 每个算法计算一次
            foreach ((string name, Func<Beatmap, double> calculator) in algorithms)
            {
                long initialMemory = GC.GetAllocatedBytesForCurrentThread();

                var    stopwatch = Stopwatch.StartNew();
                double sr        = calculator(beatmap);
                stopwatch.Stop();

                long finalMemory = GC.GetAllocatedBytesForCurrentThread();
                long memoryUsed  = finalMemory - initialMemory;

                results.Add((name, sr, stopwatch.ElapsedMilliseconds, memoryUsed));

                // 计算间隔延迟
                Thread.Sleep(50);
            }

            // 输出表格
            _output.WriteLine("=== test10K--6.1SR.osu算法对比表格 ===");
            var table = new StringBuilder();
            table.AppendLine("| 算法       | SR     | 时间(ms) | 内存(MB) |");
            table.AppendLine("|------------|--------|----------|----------|");

            foreach ((string algorithm, double sr, long timeMs, long memoryBytes) in results)
            {
                double memoryMB = memoryBytes / (1024.0 * 1024.0);
                table.AppendLine($"| {algorithm,-10} | {sr,6:F4} | {timeMs,8} | {memoryMB,8:F2} |");
            }

            _output.WriteLine(table.ToString());
            _output.WriteLine("");

            // Assert - 验证结果合理性
            foreach ((string algorithm, double sr, long _, long _) in results)
            {
                Assert.True(sr >= 0, $"SR值不能为负 ({algorithm}): {sr}");
                Assert.True(sr <= 10, $"SR值过高 ({algorithm}): {sr}");
            }

            _output.WriteLine("✅ test10K--6.1SR.osu对比测试完成");
        }

        // 辅助方法：从文件计算SR
        private double CalculateSRFromFile(string filePath)
        {
            return SRCalculator.Instance.CalculateSRFromFileCS(filePath);
        }

        // 辅助方法：从内容计算SR
        private double CalculateSRFromContent(string content)
        {
            return SRCalculator.Instance.CalculateSRFromContentCS(content);
        }

        // 辅助方法：使用V2.3版本计算
        private double CalculateWithV23(Beatmap beatmap)
        {
            var    calculator   = new SRCalculatorV23();
            var    noteSequence = new List<SRsNote>();
            int    keyCount     = (int)beatmap.BeatmapInfo.Difficulty.CircleSize;
            double od           = beatmap.BeatmapInfo.Difficulty.OverallDifficulty;

            foreach (HitObject hitObject in beatmap.HitObjects)
            {
                int col  = hitObject is ManiaHitObject maniaHit ? maniaHit.Column : ManiaExtensions.GetColumnFromX(keyCount, hitObject.Position.X);
                int time = (int)hitObject.StartTime;
                int tail = hitObject.EndTime > hitObject.StartTime ? (int)hitObject.EndTime : -1;
                noteSequence.Add(new SRsNote(col, time, tail));
            }

            return calculator.Calculate(noteSequence, keyCount, od, out _);
        }
    }
}
