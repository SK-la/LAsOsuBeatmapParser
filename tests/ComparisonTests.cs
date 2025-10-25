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
    /// <summary>
    /// C#+Rust所有SR算法对比测试
    /// 测试单一文件计算一次，多个文件计算一次
    /// </summary>
    public class ComparisonTests
    {
        private readonly ITestOutputHelper _output;

        // SR显示精度控制常量
        private const int SR_DECIMAL_PLACES = 4;

        // 测试文件路径
        private static readonly string TestResourceDir = Path.Combine(
            AppDomain.CurrentDomain.BaseDirectory,
            "..", "..", "..", "Resource"
        );

        private static readonly string SingleTestFile = Path.Combine(
            TestResourceDir,
            "Jumpstream - Happy Hardcore Synthesizer (SK_la) [10k-1].osu"
        );

        private static readonly string ComparisonTestFile = Path.Combine(
            TestResourceDir,
            "encoded_output.osu"
        );

        private static readonly string[] MultipleTestFiles = Directory.GetFiles(TestResourceDir, "*.osu")
                                                                      .Where(f => !f.Contains("SUPERMUG"))
                                                                      .ToArray();

        public ComparisonTests(ITestOutputHelper output)
        {
            _output = output;
        }

        [Fact]
        public void TestComparisonSingleFile_Once()
        {
            // Arrange
            Assert.True(File.Exists(SingleTestFile), $"Test file not found: {SingleTestFile}");
            var decoder = new LegacyBeatmapDecoder();
            Beatmap beatmap = decoder.Decode(SingleTestFile);

            _output.WriteLine($"=== C#+Rust SR算法对比 - 单一文件测试 ===");
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
                ("C# Current", bm => SRCalculator.Instance.CalculateSR(bm, out _)),
                ("C# Rust", bm => SRCalculator.Instance.CalculateSRRust(bm)),
                ("C# FromFile", bm => CalculateSRFromFile(SingleTestFile)),
                ("C# FromContent", bm => CalculateSRFromContent(File.ReadAllText(SingleTestFile))),
                ("Rust FromFile", bm => SRCalculatorRust.CalculateSR_FromFile(SingleTestFile) ?? -1),
                ("Rust FromContent", bm => SRCalculatorRust.CalculateSR_FromContent(File.ReadAllText(SingleTestFile)) ?? -1),
                ("Rust FromJson", bm => SRCalculatorRust.CalculateSR_FromJson(SRCalculatorRust.ConvertBeatmapToJson(bm)) ?? -1)
            };

            // Act - 每个算法计算一次
            foreach ((string name, Func<Beatmap, double> calculator) in algorithms)
            {
                long initialMemory = GC.GetAllocatedBytesForCurrentThread();

                var stopwatch = Stopwatch.StartNew();
                double sr = calculator(beatmap);
                stopwatch.Stop();

                long finalMemory = GC.GetAllocatedBytesForCurrentThread();
                long memoryUsed = finalMemory - initialMemory;

                results.Add((name, sr, stopwatch.ElapsedMilliseconds, memoryUsed));

                _output.WriteLine($"{name}:");
                _output.WriteLine($"  SR: {sr:F4}");
                _output.WriteLine($"  时间: {stopwatch.ElapsedMilliseconds}ms");
                _output.WriteLine($"  内存使用: {memoryUsed} bytes ({memoryUsed / (1024.0 * 1024.0):F2} MB)");
                _output.WriteLine("");

                // 计算间隔延迟
                System.Threading.Thread.Sleep(50);
            }

            // Assert - 验证结果合理性
            foreach ((string algorithm, double sr, long timeMs, long memoryBytes) in results)
            {
                Assert.True(sr >= 0, $"SR值不能为负 ({algorithm}): {sr}");
                Assert.True(sr <= 100, $"SR值过高 ({algorithm}): {sr}");
            }

            _output.WriteLine("✅ 单一文件对比测试完成");
        }

        [Fact]
        public void TestComparisonMultipleFiles_Once()
        {
            // Arrange
            var decoder = new LegacyBeatmapDecoder();

            _output.WriteLine($"=== C#+Rust SR算法对比 - 多个文件测试 ===");
            _output.WriteLine($"测试文件数量: {MultipleTestFiles.Length}");
            _output.WriteLine("");

            // 预热JIT
            Beatmap warmupBeatmap = decoder.Decode(MultipleTestFiles.First());
            SRCalculator.Instance.CalculateSR(warmupBeatmap, out _);
            GC.Collect();
            GC.WaitForPendingFinalizers();

            var results = new List<(string fileName, string algorithm, double sr, long timeMs, long memoryBytes)>();

            // 定义所有算法
            var algorithms = new (string name, Func<Beatmap, string, double> calculator)[]
            {
                ("C# Current", (bm, fp) => SRCalculator.Instance.CalculateSR(bm, out _)),
                ("C# Rust", (bm, fp) => SRCalculator.Instance.CalculateSRRust(bm)),
                ("C# FromFile", (bm, fp) => CalculateSRFromFile(fp)),
                ("C# FromContent", (bm, fp) => CalculateSRFromContent(File.ReadAllText(fp))),
                ("Rust FromFile", (bm, fp) => SRCalculatorRust.CalculateSR_FromFile(fp) ?? -1),
                ("Rust FromContent", (bm, fp) => SRCalculatorRust.CalculateSR_FromContent(File.ReadAllText(fp)) ?? -1),
                ("Rust FromJson", (bm, fp) => SRCalculatorRust.CalculateSR_FromJson(SRCalculatorRust.ConvertBeatmapToJson(bm)) ?? -1)
            };

            // Act - 每个文件每个算法计算一次
            foreach (string filePath in MultipleTestFiles)
            {
                Beatmap beatmap = decoder.Decode(filePath);
                string fileName = Path.GetFileName(filePath);

                foreach ((string algorithmName, Func<Beatmap, string, double> calculator) in algorithms)
                {
                    long initialMemory = GC.GetAllocatedBytesForCurrentThread();

                    var stopwatch = Stopwatch.StartNew();
                    double sr = calculator(beatmap, filePath);
                    stopwatch.Stop();

                    long finalMemory = GC.GetAllocatedBytesForCurrentThread();
                    long memoryUsed = finalMemory - initialMemory;

                    results.Add((fileName, algorithmName, sr, stopwatch.ElapsedMilliseconds, memoryUsed));

                    // 计算间隔延迟
                    System.Threading.Thread.Sleep(10);
                }
            }

            // Assert - 验证结果合理性
            foreach ((string fileName, string algorithm, double sr, long timeMs, long memoryBytes) in results)
            {
                Assert.True(sr >= 0, $"SR值不能为负 ({fileName} - {algorithm}): {sr}");
                Assert.True(sr <= 100, $"SR值过高 ({fileName} - {algorithm}): {sr}");
            }

            // 计算统计信息
            IEnumerable<IGrouping<string, (string fileName, string algorithm, double sr, long timeMs, long memoryBytes)>> algorithmGroups = results.GroupBy(r => r.algorithm);

            foreach (IGrouping<string, (string fileName, string algorithm, double sr, long timeMs, long memoryBytes)> group in algorithmGroups)
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

            _output.WriteLine("✅ 多个文件对比测试完成");
        }

        [Fact]
        public void TestComparisonEncodedOutputFile_Once()
        {
            // Arrange
            Assert.True(File.Exists(ComparisonTestFile), $"Comparison test file not found: {ComparisonTestFile}");
            var decoder = new LegacyBeatmapDecoder();
            Beatmap beatmap = decoder.Decode(ComparisonTestFile);

            _output.WriteLine($"=== C#+Rust SR算法对比 - encoded_output.osu测试 ===");
            _output.WriteLine($"测试文件: {Path.GetFileName(ComparisonTestFile)}");
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
                ("C# Current", bm => SRCalculator.Instance.CalculateSR(bm, out _)),
                ("C# Rust", bm => SRCalculator.Instance.CalculateSRRust(bm)),
                ("C# FromFile", bm => CalculateSRFromFile(ComparisonTestFile)),
                ("C# FromContent", bm => CalculateSRFromContent(File.ReadAllText(ComparisonTestFile))),
                ("Rust FromFile", bm => SRCalculatorRust.CalculateSR_FromFile(ComparisonTestFile) ?? -1),
                ("Rust FromContent", bm => SRCalculatorRust.CalculateSR_FromContent(File.ReadAllText(ComparisonTestFile)) ?? -1),
                ("Rust FromJson", bm => SRCalculatorRust.CalculateSR_FromJson(SRCalculatorRust.ConvertBeatmapToJson(bm)) ?? -1)
            };

            // Act - 每个算法计算一次
            foreach ((string name, Func<Beatmap, double> calculator) in algorithms)
            {
                long initialMemory = GC.GetAllocatedBytesForCurrentThread();

                var stopwatch = Stopwatch.StartNew();
                double sr = calculator(beatmap);
                stopwatch.Stop();

                long finalMemory = GC.GetAllocatedBytesForCurrentThread();
                long memoryUsed = finalMemory - initialMemory;

                results.Add((name, sr, stopwatch.ElapsedMilliseconds, memoryUsed));

                _output.WriteLine($"{name}:");
                _output.WriteLine($"  SR: {sr:F4}");
                _output.WriteLine($"  时间: {stopwatch.ElapsedMilliseconds}ms");
                _output.WriteLine($"  内存使用: {memoryUsed} bytes ({memoryUsed / (1024.0 * 1024.0):F2} MB)");
                _output.WriteLine("");

                // 计算间隔延迟
                System.Threading.Thread.Sleep(50);
            }

            // Assert - 验证结果合理性
            foreach ((string algorithm, double sr, long timeMs, long memoryBytes) in results)
            {
                Assert.True(sr >= 0, $"SR值不能为负 ({algorithm}): {sr}");
                Assert.True(sr <= 100, $"SR值过高 ({algorithm}): {sr}");
            }

            _output.WriteLine("✅ encoded_output.osu对比测试完成");
        }

        // 辅助方法：从文件计算SR
        private double CalculateSRFromFile(string filePath)
        {
            return SRCalculator.Instance.CalculateSRFromFile(filePath);
        }

        // 辅助方法：从内容计算SR
        private double CalculateSRFromContent(string content)
        {
            return SRCalculator.Instance.CalculateSRFromContent(content);
        }
    }
}
