using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text.Json;
using BenchmarkDotNet.Attributes;
using LAsOsuBeatmapParser.Analysis;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Beatmaps.Formats;
using Xunit;
using Xunit.Abstractions;

namespace LAsOsuBeatmapParser.Tests
{
    [Collection("Sequential")]
    public class RustSRCalculatorTests
    {
        private readonly ITestOutputHelper _output;

        // 测试文件路径定义
        private static readonly string TestResourceDir = Path.Combine(
            AppDomain.CurrentDomain.BaseDirectory,
            "..", "..", "..", "Resource"
        );

        private static readonly string SingleTestFile = Path.Combine(
            TestResourceDir,
            "Glen Check - 60's Cardin (SK_la) [Insane].osu"
        );

        private static readonly string[] TestFiles = Directory.GetFiles(TestResourceDir, "*.osu");

        public RustSRCalculatorTests(ITestOutputHelper output)
        {
            _output = output;
        }

        [Fact]
        public void TestRust_CalculateSR_FromFile_SingleFile()
        {
            // Arrange
            Assert.True(File.Exists(SingleTestFile), $"Test file not found: {SingleTestFile}");

            // Act
            var stopwatch = Stopwatch.StartNew();
            double? sr = SRCalculatorRust.CalculateSR_FromFile(SingleTestFile);
            stopwatch.Stop();

            // Assert
            Assert.NotNull(sr);
            Assert.True(sr > 0);

            _output.WriteLine($"Rust SR (File): {sr:F4}");
            _output.WriteLine($"Time: {stopwatch.ElapsedMilliseconds}ms");
        }

        [Fact]
        public void TestRust_CalculateSR_FromContent_SingleFile()
        {
            // Arrange
            Assert.True(File.Exists(SingleTestFile), $"Test file not found: {SingleTestFile}");
            string content = File.ReadAllText(SingleTestFile);

            // Act
            var stopwatch = Stopwatch.StartNew();
            double? sr = SRCalculatorRust.CalculateSR_FromContent(content);
            stopwatch.Stop();

            // Assert
            Assert.NotNull(sr);
            Assert.True(sr > 0);

            _output.WriteLine($"Rust SR (Content): {sr:F4}");
            _output.WriteLine($"Time: {stopwatch.ElapsedMilliseconds}ms");
        }

        [Fact]
        public void TestRust_CalculateSR_FromJson_SingleFile()
        {
            // Arrange
            Assert.True(File.Exists(SingleTestFile), $"Test file not found: {SingleTestFile}");

            var decoder = new LegacyBeatmapDecoder();
            Beatmap beatmap = decoder.Decode(SingleTestFile);
            string json = SRCalculatorRust.ConvertBeatmapToJson(beatmap);

            // Act
            var stopwatch = Stopwatch.StartNew();
            double? sr = SRCalculatorRust.CalculateSR_FromJson(json);
            stopwatch.Stop();

            // Assert
            Assert.NotNull(sr);
            Assert.True(sr > 0);

            _output.WriteLine($"Rust SR (JSON): {sr:F4}");
            _output.WriteLine($"Time: {stopwatch.ElapsedMilliseconds}ms");
        }

        [Fact]
        public void TestRust_CalculateSR_FromStruct_SingleFile()
        {
            // Arrange
            Assert.True(File.Exists(SingleTestFile), $"Test file not found: {SingleTestFile}");

            var decoder = new LegacyBeatmapDecoder();
            Beatmap beatmap = decoder.Decode(SingleTestFile);
            SRCalculatorRust.CBeatmapData structData = SRCalculatorRust.ConvertBeatmapToStruct(beatmap);

            // Act
            var stopwatch = Stopwatch.StartNew();
            double sr = SRCalculatorRust.CalculateSR_FromStruct(structData);
            stopwatch.Stop();

            // Assert
            Assert.True(sr > 0);

            _output.WriteLine($"Rust SR (Struct): {sr:F4}");
            _output.WriteLine($"Time: {stopwatch.ElapsedMilliseconds}ms");
        }

        [Fact]
        public void TestRust_AllMethods_Consistency_SingleFile()
        {
            // Arrange
            Assert.True(File.Exists(SingleTestFile), $"Test file not found: {SingleTestFile}");

            var decoder = new LegacyBeatmapDecoder();
            Beatmap beatmap = decoder.Decode(SingleTestFile);
            string content = File.ReadAllText(SingleTestFile);
            string json = SRCalculatorRust.ConvertBeatmapToJson(beatmap);
            SRCalculatorRust.CBeatmapData structData = SRCalculatorRust.ConvertBeatmapToStruct(beatmap);

            // Act
            double? sr = SRCalculatorRust.CalculateSR_FromFile(SingleTestFile);
            double? srContent = SRCalculatorRust.CalculateSR_FromContent(content);
            SRCalculatorRust.CalculateSR(SingleTestFile);
            double? srJson = SRCalculatorRust.CalculateSR_FromJson(json);
            double srStruct = SRCalculatorRust.CalculateSR_FromStruct(structData);

            // Assert
            Assert.NotNull(sr);
            Assert.NotNull(srContent);
            Assert.NotNull(srJson);
            Assert.True(sr > 0);
            Assert.True(srContent > 0);
            Assert.True(srJson > 0);
            Assert.True(srStruct > 0);

            // Check consistency (allow small differences)
            double baseSr = sr.Value;

            _output.WriteLine($"File SR: {sr:F4}");
            _output.WriteLine($"Content SR: {srContent:F4}");
            _output.WriteLine($"JSON SR: {srJson:F4}");
            _output.WriteLine($"Struct SR: {srStruct:F4}");

            Assert.True(Math.Abs(srContent.Value - baseSr) < 0.01, $"Content SR differs: {srContent} vs {baseSr}");
            Assert.True(Math.Abs(srJson.Value - baseSr) < 0.01, $"JSON SR differs: {srJson} vs {baseSr}");
            Assert.True(Math.Abs(srStruct - baseSr) < 0.01, $"Struct SR differs: {srStruct} vs {baseSr}");
        }

        [Fact]
        public void TestRust_Performance_MultipleFiles()
        {
            // Arrange
            Assert.NotEmpty(TestFiles);

            var decoder = new LegacyBeatmapDecoder();

            var results = new List<(double Cs, double? Sr, long TimeMs)>();

            // Act
            foreach (string filePath in TestFiles)
            {
                Beatmap beatmap = decoder.Decode(filePath);
                double cs = beatmap.BeatmapInfo.Difficulty.CircleSize;

                var stopwatch = Stopwatch.StartNew();
                double? sr = SRCalculatorRust.CalculateSR_FromFile(filePath);
                stopwatch.Stop();

                results.Add((cs, sr, stopwatch.ElapsedMilliseconds));

                Assert.NotNull(sr);
                Assert.True(sr > 0);
            }

            // Output results
            _output.WriteLine("Rust SR Performance Results:");
            _output.WriteLine("CS | SR | Time (ms)");
            _output.WriteLine("---|-----|----------");

            foreach ((double cs, double? sr, long timeMs) in results) _output.WriteLine($"{cs,-3:F1} | {sr:F4} | {timeMs,8}");

            double avgSr = results.Average(r => r.Sr ?? 0);
            double avgTime = results.Average(r => r.TimeMs);

            _output.WriteLine($"Average SR: {avgSr:F4}");
            _output.WriteLine($"Average Time: {avgTime:F2}ms");
            _output.WriteLine($"Total Files: {results.Count}");
        }
    }

    [MemoryDiagnoser]
    public class RustSRCalculatorBenchmarks
    {
        private string _testFilePath;
        private string _testContent;
        private string _testJson;
        private SRCalculatorRust.CBeatmapData _testStruct;

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

            var decoder = new LegacyBeatmapDecoder();
            Beatmap beatmap = decoder.Decode(_testFilePath);

            _testContent = File.ReadAllText(_testFilePath);
            _testJson = SRCalculatorRust.ConvertBeatmapToJson(beatmap);
            _testStruct = SRCalculatorRust.ConvertBeatmapToStruct(beatmap);
        }

        [Benchmark]
        public double? CalculateSR_FromFile()
        {
            return SRCalculatorRust.CalculateSR_FromFile(_testFilePath);
        }

        [Benchmark]
        public double? CalculateSR_FromContent()
        {
            return SRCalculatorRust.CalculateSR_FromContent(_testContent);
        }

        [Benchmark]
        public double? CalculateSR_FromJson()
        {
            return SRCalculatorRust.CalculateSR_FromJson(_testJson);
        }

        [Benchmark]
        public double CalculateSR_FromStruct()
        {
            return SRCalculatorRust.CalculateSR_FromStruct(_testStruct);
        }
    }
}
