using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
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

        public RustSRCalculatorTests(ITestOutputHelper output)
        {
            _output = output;
        }

        [Fact]
        public void TestRust_AllMethods_Consistency_SingleFile()
        {
            // Arrange
            Assert.True(File.Exists(SingleTestFile), $"Test file not found: {SingleTestFile}");

            var                           decoder    = new LegacyBeatmapDecoder();
            Beatmap                       beatmap    = decoder.Decode(SingleTestFile);
            string                        content    = File.ReadAllText(SingleTestFile);
            string                        json       = SRCalculatorRust.ConvertBeatmapToJson(beatmap);
            SRCalculatorRust.CBeatmapData structData = SRCalculatorRust.ConvertBeatmapToStruct(beatmap);

            // Act & Collect results
            var results = new List<(string Method, double Sr, long TimeMs)>();

            // FromFile
            var     stopwatch = Stopwatch.StartNew();
            double? srFile    = SRCalculatorRust.CalculateSR_FromFile(SingleTestFile);
            stopwatch.Stop();
            Assert.NotNull(srFile);
            Assert.True(srFile > 0);
            results.Add(("FromFile", srFile.Value, stopwatch.ElapsedMilliseconds));

            // FromContent
            stopwatch.Restart();
            double? srContent = SRCalculatorRust.CalculateSR_FromContent(content);
            stopwatch.Stop();
            Assert.NotNull(srContent);
            Assert.True(srContent > 0);
            results.Add(("FromContent", srContent.Value, stopwatch.ElapsedMilliseconds));

            // FromJson
            stopwatch.Restart();
            double? srJson = SRCalculatorRust.CalculateSR_FromJson(json);
            stopwatch.Stop();
            Assert.NotNull(srJson);
            Assert.True(srJson > 0);
            results.Add(("FromJson", srJson.Value, stopwatch.ElapsedMilliseconds));

            // FromStruct
            stopwatch.Restart();
            double srStruct = SRCalculatorRust.CalculateSR_FromStruct(structData);
            stopwatch.Stop();
            Assert.True(srStruct > 0);
            results.Add(("FromStruct", srStruct, stopwatch.ElapsedMilliseconds));

            // Assert
            foreach ((string method, double sr, long _) in results) Assert.True(sr > 0, $"SR from {method} must be positive: {sr}");

            // Output table
            _output.WriteLine("=== Rust SR Calculator Methods Comparison ===");
            var table = new StringBuilder();
            table.AppendLine("| Method     | SR       | Time (ms) |");
            table.AppendLine("|------------|----------|-----------|");
            foreach ((string method, double sr, long timeMs) in results) table.AppendLine($"| {method,-10} | {sr,8:F4} | {timeMs,9} |");
            _output.WriteLine(table.ToString());
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
                double  cs      = beatmap.BeatmapInfo.Difficulty.CircleSize;

                var     stopwatch = Stopwatch.StartNew();
                double? sr        = SRCalculatorRust.CalculateSR_FromFile(filePath);
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

            double avgTime = results.Average(r => r.TimeMs);

            _output.WriteLine($"Average Time: {avgTime:F2}ms");
            _output.WriteLine($"Total Files: {results.Count}");
        }
    }

    [MemoryDiagnoser]
    public class RustSRCalculatorBenchmarks
    {
        private string                        _testContent;
        private string                        _testFilePath;
        private string                        _testJson;
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

            var     decoder = new LegacyBeatmapDecoder();
            Beatmap beatmap = decoder.Decode(_testFilePath);

            _testContent = File.ReadAllText(_testFilePath);
            _testJson    = SRCalculatorRust.ConvertBeatmapToJson(beatmap);
            _testStruct  = SRCalculatorRust.ConvertBeatmapToStruct(beatmap);
        }
    }
}
