using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using BenchmarkDotNet.Attributes;
using LAsOsuBeatmapParser.Analysis;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Beatmaps.Formats;
using Xunit;
using Xunit.Abstractions;

namespace LAsOsuBeatmapParser.Tests
{
    [Collection("Sequential")]
    public class VS_SR_Tests
    {
        // 测试文件路径定义
        private static readonly string TestResourceDir = Path.Combine(
            AppDomain.CurrentDomain.BaseDirectory,
            "..", "..", "..", "Resource"
        );

        private static readonly string SingleTestFile = Path.Combine(
            TestResourceDir,
            "Glen Check - 60's Cardin (SK_la) [Insane].osu"
        );

        private static readonly string[]          TestFiles = Directory.GetFiles(TestResourceDir, "*.osu");
        private readonly        ITestOutputHelper _output;

        public VS_SR_Tests(ITestOutputHelper output)
        {
            _output = output;
        }

        [Fact]
        public void Test_3Version_MultipleFiles()
        {
            // Arrange
            Assert.NotEmpty(TestFiles);

            var decoder = new LegacyBeatmapDecoder();

            var results = new List<(double Cs, double? RustSr, long RustTime, double? CsSr, long CsTime, double? PySr, long PyTime)>();

            // Act
            foreach (string filePath in TestFiles)
            {
                Beatmap beatmap = decoder.Decode(filePath);
                double  cs      = beatmap.BeatmapInfo.Difficulty.CircleSize;

                // Rust
                var     stopwatch = Stopwatch.StartNew();
                double? rustSr    = SRCalculatorRust.CalculateSR_FromFile(filePath);
                stopwatch.Stop();
                long rustTime = stopwatch.ElapsedMilliseconds;

                // C#
                stopwatch = Stopwatch.StartNew();
                double csSr = SRCalculator.Instance.CalculateSR(beatmap, out _);
                stopwatch.Stop();
                long csTime = stopwatch.ElapsedMilliseconds;

                // Python
                stopwatch.Restart();
                double? pySr = SRCalculatorPython.CalculateSR_FromFile(filePath);
                stopwatch.Stop();
                long pyTime = stopwatch.ElapsedMilliseconds;

                results.Add((cs, rustSr, rustTime, csSr, csTime, pySr, pyTime));
            }

            // Output results
            _output.WriteLine("SR Performance Results:");
            _output.WriteLine("CS | Rust SR | C# SR | Py SR | Rust Time | C# Time | Py Time");
            _output.WriteLine("---|---------|-------|-------|-----------|---------|---------");

            foreach ((double cs, double? rustSr, long rustTime, double? csSr, long csTime, double? pySr, long pyTime) in results)
                _output.WriteLine($"{cs} | {rustSr:F4} | {csSr:F4} | {pySr:F4} | {rustTime,8} | {csTime,7} | {pyTime,7}");

            double avgRustTime = results.Average(r => r.RustTime);
            double avgCsTime   = results.Average(r => r.CsTime);
            double avgPyTime   = results.Average(r => r.PyTime);

            _output.WriteLine($"Average Rust Time: {avgRustTime:F2}ms");
            _output.WriteLine($"Average C# Time: {avgCsTime:F2}ms");
            _output.WriteLine($"Average Py Time: {avgPyTime:F2}ms");
            _output.WriteLine($"Total Files: {results.Count}");

            // Assert after printing the table
            foreach ((double cs, double? rustSr, long rustTime, double? csSr, long csTime, double? pySr, long pyTime) in results)
            {
                if (rustSr.HasValue && pySr.HasValue && !double.IsNaN(rustSr.Value) && !double.IsNaN(pySr.Value))
                    Assert.True(Math.Abs(rustSr.Value - pySr.Value) < 0.001, $"Rust SR {rustSr.Value} vs Python {pySr.Value}");

                if (csSr.HasValue && pySr.HasValue && !double.IsNaN(csSr.Value) && !double.IsNaN(pySr.Value))
                    Assert.True(Math.Abs(csSr.Value - pySr.Value) < 0.001, $"C# SR {csSr.Value} vs Python {pySr.Value}");
            }
        }
    }

    [MemoryDiagnoser]
    public class RustSRCalculatorBenchmarks
    {
        private string? _testFilePath;

        [GlobalSetup]
        public void Setup()
        {
            _testFilePath = Path.Combine(
                AppDomain.CurrentDomain.BaseDirectory,
                "..", "..", "..", "..", "tests", "Resource",
                "Glen Check - 60's Cardin (SK_la) [Insane].osu"
            );

            if (!File.Exists(_testFilePath))
                throw new FileNotFoundException($"Benchmark file not found: {_testFilePath}");
        }
    }
}
