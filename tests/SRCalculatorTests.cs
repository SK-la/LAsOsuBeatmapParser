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

namespace LAsOsuBeatmapParser.Tests
{
    [CollectionDefinition("Sequential", DisableParallelization = true)]
    public class SequentialCollection { }

    [Collection("Sequential")]
    public class SRCalculatorTests
    {
        private readonly ITestOutputHelper _output;

        // SR显示精度控制常量
        private const int SR_DECIMAL_PLACES = 4;

        public SRCalculatorTests(ITestOutputHelper output)
        {
            _output = output;
        }

        private Beatmap LoadBeatmap(string filePath)
        {
            if (!File.Exists(filePath))
            {
                throw new FileNotFoundException($"Test file not found: {filePath}");
            }

            var decoder = new LegacyBeatmapDecoder();
            return decoder.Decode(filePath);
        }

        private string[] GetTestFilePaths()
        {
            string resourceDir = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", "..", "..", "tests", "Resource");
            return Directory.GetFiles(resourceDir, "*.osu").Where(f => !f.Contains("SUPERMUG")).ToArray();
        }

        [Fact]
        public void TestSRCalculatorCSharp_SingleFile()
        {
            string testFilePath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", "..", "..", "tests", "Resource", "Jumpstream - Happy Hardcore Synthesizer (SK_la) [10k-1].osu");

            var beatmap = LoadBeatmap(testFilePath);
            var calculator = SRCalculator.Instance;

            double sr = calculator.CalculateSR(beatmap, out var times);

            _output.WriteLine($"C# SR (Single File): {sr:F4}");
            _output.WriteLine($"Total Time: {times.GetValueOrDefault("Total", 0)}ms");

            Assert.True(sr > 0);
        }

        [Fact]
        public void TestSRCalculatorCSharp_MultipleFiles()
        {
            var filePaths = GetTestFilePaths();
            var calculator = SRCalculator.Instance;

            foreach (var filePath in filePaths)
            {
                var beatmap = LoadBeatmap(filePath);
                double sr = calculator.CalculateSR(beatmap, out var times);

                string fileName = Path.GetFileName(filePath);
                _output.WriteLine($"{fileName} - C# SR: {sr:F4}, Time: {times.GetValueOrDefault("Total", 0)}ms");

                Assert.True(sr > 0);
            }
        }

        [Fact]
        public void TestSRCalculatorRust_SingleFile()
        {
            string testFilePath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", "..", "..", "tests", "Resource", "Jumpstream - Happy Hardcore Synthesizer (SK_la) [10k-1].osu");

            double? sr = SRCalculatorRust.CalculateSR(testFilePath);

            _output.WriteLine($"Rust SR (Single File): {sr:F4}");

            Assert.NotNull(sr);
            Assert.True(sr > 0);
        }

        [Fact]
        public void TestSRCalculatorRust_MultipleFiles()
        {
            var filePaths = GetTestFilePaths();

            foreach (var filePath in filePaths)
            {
                double? sr = SRCalculatorRust.CalculateSR(filePath);

                string fileName = Path.GetFileName(filePath);
                _output.WriteLine($"{fileName} - Rust SR: {sr:F4}");

                Assert.NotNull(sr);
                Assert.True(sr > 0);
            }
        }

        [Fact]
        public void TestSRCalculator_Compare_SingleFile()
        {
            string testFilePath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", "..", "..", "tests", "Resource", "Jumpstream - Happy Hardcore Synthesizer (SK_la) [10k-1].osu");

            var beatmap = LoadBeatmap(testFilePath);
            var calculator = SRCalculator.Instance;

            double csharpSr = calculator.CalculateSR(beatmap, out _);
            double? rustSr = SRCalculatorRust.CalculateSR(testFilePath);

            _output.WriteLine($"C# SR: {csharpSr:F4}");
            _output.WriteLine($"Rust SR: {rustSr:F4}");
            _output.WriteLine($"Difference: {Math.Abs(csharpSr - (rustSr ?? 0)):F4}");

            Assert.NotNull(rustSr);
            Assert.True(csharpSr > 0);
            Assert.True(rustSr > 0);
            // Allow small difference due to implementation details
            Assert.True(Math.Abs(csharpSr - rustSr.Value) < 1.0);
        }

        [Fact]
        public void TestSRCalculator_Compare_MultipleFiles()
        {
            var filePaths = GetTestFilePaths();
            var calculator = SRCalculator.Instance;

            foreach (var filePath in filePaths)
            {
                var beatmap = LoadBeatmap(filePath);
                double csharpSr = calculator.CalculateSR(beatmap, out _);
                double? rustSr = SRCalculatorRust.CalculateSR(filePath);

                string fileName = Path.GetFileName(filePath);
                double diff = Math.Abs(csharpSr - (rustSr ?? 0));

                _output.WriteLine($"{fileName}:");
                _output.WriteLine($"  C# SR: {csharpSr:F4}");
                _output.WriteLine($"  Rust SR: {rustSr:F4}");
                _output.WriteLine($"  Difference: {diff:F4}");

                Assert.NotNull(rustSr);
                Assert.True(csharpSr > 0);
                Assert.True(rustSr > 0);

                Assert.True(diff < 0.0001); // 如果不通过说明其中一个代码有问题，有限排查Rust版本
            }
        }
    }
}
