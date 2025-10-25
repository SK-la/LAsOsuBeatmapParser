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
    /// 生产环境C# SR算法测试
    /// 测试单一文件循环3次，多个文件计算
    /// </summary>
    public class ProductionCSRTests
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

        private static readonly string[] MultipleTestFiles = Directory.GetFiles(TestResourceDir, "*.osu")
                                                                      .Where(f => !f.Contains("SUPERMUG") && !f.Contains("encoded_output"))
                                                                      .ToArray();

        public ProductionCSRTests(ITestOutputHelper output)
        {
            _output = output;
        }

        [Fact]
        public void TestProductionCSRSingleFile_Loop3Times()
        {
            // Arrange
            Assert.True(File.Exists(SingleTestFile), $"Test file not found: {SingleTestFile}");
            var decoder = new LegacyBeatmapDecoder();
            Beatmap beatmap = decoder.Decode(SingleTestFile);

            _output.WriteLine($"=== 生产环境C# SR算法 - 单一文件循环3次测试 ===");
            _output.WriteLine($"CS值: {beatmap.BeatmapInfo.Difficulty.CircleSize}");
            _output.WriteLine($"谱面信息: {beatmap.BeatmapInfo.Metadata.Artist} - {beatmap.BeatmapInfo.Metadata.Title} [{beatmap.BeatmapInfo.Metadata.Version}]");
            _output.WriteLine($"键数: {(int)beatmap.BeatmapInfo.Difficulty.CircleSize}k");
            _output.WriteLine("");

            // 预热JIT
            SRCalculator.Instance.CalculateSR(beatmap, out _);
            GC.Collect(); // 清理GC以获得准确的内存测量
            GC.WaitForPendingFinalizers();

            var results = new List<(double sr, long timeMs, long memoryBytes)>();

            // Act - 循环3次
            for (int i = 0; i < 3; i++)
            {
                long initialMemory = GC.GetAllocatedBytesForCurrentThread();

                var stopwatch = Stopwatch.StartNew();
                double sr = SRCalculator.Instance.CalculateSR(beatmap, out Dictionary<string, long> times);
                stopwatch.Stop();

                long finalMemory = GC.GetAllocatedBytesForCurrentThread();
                long memoryUsed = finalMemory - initialMemory;

                results.Add((sr, stopwatch.ElapsedMilliseconds, memoryUsed));

                _output.WriteLine($"第{i + 1}次计算:");
                _output.WriteLine($"  SR: {sr:F4}");
                _output.WriteLine($"  时间: {stopwatch.ElapsedMilliseconds}ms");
                _output.WriteLine($"  内存使用: {memoryUsed} bytes ({memoryUsed / (1024.0 * 1024.0):F2} MB)");
                _output.WriteLine(
                    $"  详细时间: Section23/24/25={times.GetValueOrDefault("Section232425", 0)}ms, Section26/27={times.GetValueOrDefault("Section2627", 0)}ms, Section3={times.GetValueOrDefault("Section3", 0)}ms, Total={times.GetValueOrDefault("Total", 0)}ms");
                _output.WriteLine("");

                // 计算间隔延迟
                if (i < 2) System.Threading.Thread.Sleep(50);
            }

            // Assert - 验证结果合理性
            foreach ((double sr, long timeMs, long memoryBytes) in results)
            {
                Assert.True(sr >= 0, $"SR值不能为负: {sr}");
                Assert.True(sr <= 10, $"SR值过高: {sr}");
            }

            // 计算平均值
            double avgSR = results.Average(r => r.sr);
            double avgTime = results.Average(r => r.timeMs);
            double avgMemory = results.Average(r => r.memoryBytes);

            _output.WriteLine("=== 平均结果 ===");
            _output.WriteLine($"平均SR: {avgSR:F4}");
            _output.WriteLine($"平均时间: {avgTime:F2}ms");
            _output.WriteLine($"平均内存使用: {avgMemory:F0} bytes ({avgMemory / (1024.0 * 1024.0):F2} MB)");
            _output.WriteLine("");

            _output.WriteLine("✅ 单一文件循环3次测试完成");
        }

        [Fact]
        public void TestProductionCSRSingleFile_MultipleFiles()
        {
            // Arrange
            var decoder = new LegacyBeatmapDecoder();
            Beatmap[] beatmaps = MultipleTestFiles.Select(f => decoder.Decode(f)).ToArray();

            _output.WriteLine($"=== 生产环境C# SR算法 - 多个文件测试 ===");
            _output.WriteLine($"测试文件数量: {beatmaps.Length}");
            _output.WriteLine("");

            // 预热JIT
            SRCalculator.Instance.CalculateSR(beatmaps.First(), out _);
            GC.Collect();
            GC.WaitForPendingFinalizers();

            var results = new List<(string fileName, double sr, long timeMs, long memoryBytes)>();

            // Act - 计算多个文件
            foreach (string filePath in MultipleTestFiles)
            {
                Beatmap beatmap = decoder.Decode(filePath);
                string fileName = Path.GetFileName(filePath);

                long initialMemory = GC.GetAllocatedBytesForCurrentThread();

                var stopwatch = Stopwatch.StartNew();
                double sr = SRCalculator.Instance.CalculateSR(beatmap, out Dictionary<string, long> times);
                stopwatch.Stop();

                long finalMemory = GC.GetAllocatedBytesForCurrentThread();
                long memoryUsed = finalMemory - initialMemory;

                results.Add((fileName, sr, stopwatch.ElapsedMilliseconds, memoryUsed));

                _output.WriteLine($"CS {beatmap.BeatmapInfo.Difficulty.CircleSize}:");
                _output.WriteLine($"  SR: {sr:F4}");
                _output.WriteLine($"  时间: {stopwatch.ElapsedMilliseconds}ms");
                _output.WriteLine($"  内存使用: {memoryUsed} bytes ({memoryUsed / (1024.0 * 1024.0):F2} MB)");
                _output.WriteLine("");

                // 计算间隔延迟
                System.Threading.Thread.Sleep(20);
            }

            // Assert - 验证结果合理性
            foreach ((string fileName, double sr, long timeMs, long memoryBytes) in results)
            {
                Assert.True(sr >= 0, $"SR值不能为负 ({fileName}): {sr}");
                Assert.True(sr <= 10, $"SR值过高 ({fileName}): {sr}");
            }

            // 计算统计信息
            double avgSR = results.Average(r => r.sr);
            double avgTime = results.Average(r => r.timeMs);
            double avgMemory = results.Average(r => r.memoryBytes);
            double totalTime = results.Sum(r => r.timeMs);
            double totalMemory = results.Sum(r => r.memoryBytes);

            _output.WriteLine("=== 统计结果 ===");
            _output.WriteLine($"平均SR: {avgSR:F4}");
            _output.WriteLine($"平均时间: {avgTime:F2}ms");
            _output.WriteLine($"平均内存使用: {avgMemory:F0} bytes ({avgMemory / (1024.0 * 1024.0):F2} MB)");
            _output.WriteLine($"总时间: {totalTime:F0}ms");
            _output.WriteLine($"总内存使用: {totalMemory:F0} bytes ({totalMemory / (1024.0 * 1024.0):F2} MB)");
            _output.WriteLine("");

            _output.WriteLine("✅ 多个文件测试完成");
        }
    }
}
