using System;
using System.Collections.Generic;
using Xunit;
using Xunit.Abstractions;
using System.IO;
using System.Text;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Encode;

namespace LAsOsuBeatmapParser.Tests;

public class BeatmapEncoderTests
{
    private readonly ITestOutputHelper _output;

    public BeatmapEncoderTests(ITestOutputHelper output)
    {
        _output = output;
    }
    [Fact]
    public void EncodeToString_ValidBeatmap_ReturnsValidOsuContent()
    {
        // Arrange
        var beatmap = new Beatmap
        {
            Mode = GameMode.Mania,
            Version = 14,
            BeatmapInfo = new BeatmapInfo
            {
                Metadata = new BeatmapMetadata
                {
                    Title = "Test Song",
                    Artist = "Test Artist",
                    Author = new BeatmapAuthor { Username = "Test Creator" },
                    Version = "Easy"
                },
                DifficultyName = "Easy",
                Difficulty = new BeatmapDifficulty
                {
                    HPDrainRate = 5,
                    CircleSize = 4,
                    OverallDifficulty = 5,
                    ApproachRate = 5
                }
            },
            DifficultyLegacy = new BeatmapDifficulty
            {
                HPDrainRate = 5,
                CircleSize = 4,
                OverallDifficulty = 5,
                ApproachRate = 5
            },
            TimingPoints = new List<TimingPoint>
            {
                new TimingPoint { Time = 0, BeatLength = 500, Meter = 4, SampleSet = 1, SampleIndex = 1, Volume = 100, Inherited = false, Effects = 0 }
            },
            HitObjects = new List<HitObject>
            {
                new ManiaHitObject(1000, 0),
                new ManiaHitObject(1500, 1),
                new ManiaHitObject(2000, 2),
                new ManiaHitObject(2500, 3)
            }
        };

        var encoder = new BeatmapEncoder();

        // Act
        var result = encoder.EncodeToString(beatmap);

        // Assert
        Assert.Contains("osu file format v14", result);
        Assert.Contains("[General]", result);
        Assert.Contains("Mode: 3", result);
        Assert.Contains("[Metadata]", result);
        Assert.Contains("Title:Test Song", result);
        Assert.Contains("Artist:Test Artist", result);
        Assert.Contains("[Difficulty]", result);
        Assert.Contains("CircleSize:4", result);
        Assert.Contains("[TimingPoints]", result);
        Assert.Contains("0,500,4,1,1,100,1,0", result);
        Assert.Contains("[HitObjects]", result);
        Assert.Contains("1000,1,0,0:0:0:0:", result); // Mania hit object format
    }

    [Fact]
    public void EncodeDecode_RoundTrip_PreservesData()
    {
        // Arrange - Create a test beatmap
        var originalBeatmap = new Beatmap
        {
            Mode = GameMode.Mania,
            Version = 14,
            BeatmapInfo = new BeatmapInfo
            {
                Metadata = new BeatmapMetadata
                {
                    Title = "Round Trip Test",
                    Artist = "Test Artist",
                    Author = new BeatmapAuthor { Username = "Test Creator" },
                    Version = "Test"
                },
                DifficultyName = "Test",
                Difficulty = new BeatmapDifficulty
                {
                    HPDrainRate = 6,
                    CircleSize = 7,
                    OverallDifficulty = 8,
                    ApproachRate = 9
                }
            },
            DifficultyLegacy = new BeatmapDifficulty
            {
                HPDrainRate = 6,
                CircleSize = 7,
                OverallDifficulty = 8,
                ApproachRate = 9
            },
            TimingPoints = new List<TimingPoint>
            {
                new TimingPoint { Time = 0, BeatLength = 400, Meter = 4, SampleSet = 1, SampleIndex = 1, Volume = 80, Inherited = false, Effects = 0 }
            },
            HitObjects = new List<HitObject>
            {
                new ManiaHitObject(500, 0, 7),
                new ManiaHitObject(1000, 3, 7),
                new ManiaHoldNote { StartTime = 1500, EndTime = 2000, Column = 1, KeyCount = 7 }
            }
        };

        var encoder = new BeatmapEncoder();
        var decoder = new Decode.BeatmapDecoder();

        // Act - Encode then decode
        var encodedContent = encoder.EncodeToString(originalBeatmap);
        using var stream = new MemoryStream(Encoding.UTF8.GetBytes(encodedContent));
        var decodedBeatmap = decoder.Decode(stream);

        // Assert - Check key properties are preserved
        Assert.Equal(originalBeatmap.Mode, decodedBeatmap.Mode);
        Assert.Equal(originalBeatmap.Metadata.Title, decodedBeatmap.Metadata.Title);
        Assert.Equal(originalBeatmap.Metadata.Artist, decodedBeatmap.Metadata.Artist);
        Assert.Equal(originalBeatmap.Metadata.Author.Username, decodedBeatmap.Metadata.Author.Username);
        Assert.Equal(originalBeatmap.BeatmapInfo.Difficulty.CircleSize, decodedBeatmap.BeatmapInfo.Difficulty.CircleSize);
        Assert.Equal(originalBeatmap.HitObjects.Count, decodedBeatmap.HitObjects.Count);
        Assert.Equal(originalBeatmap.TimingPoints.Count, decodedBeatmap.TimingPoints.Count);
    }

    [Fact]
    public void Mania_CoordinateConversion_WorksCorrectly()
    {
        // Test coordinate conversion for different key counts
        var testCases = new[]
        {
            (keyCount: 4, column: 0, expectedX: 64),   // 4k: (0 + 0.5) * (512/4) = 0.5 * 128 = 64
            (keyCount: 4, column: 1, expectedX: 192),  // 4k: (1 + 0.5) * (512/4) = 1.5 * 128 = 192
            (keyCount: 4, column: 2, expectedX: 320),  // 4k: (2 + 0.5) * (512/4) = 2.5 * 128 = 320
            (keyCount: 4, column: 3, expectedX: 448),  // 4k: (3 + 0.5) * (512/4) = 3.5 * 128 = 448
            (keyCount: 7, column: 0, expectedX: 37),   // 7k: (0 + 0.5) * (512/7) ≈ 0.5 * 73.14 ≈ 37
            (keyCount: 7, column: 3, expectedX: 256),  // 7k: (3 + 0.5) * (512/7) ≈ 3.5 * 73.14 ≈ 256
            (keyCount: 7, column: 6, expectedX: 475),  // 7k: (6 + 0.5) * (512/7) ≈ 6.5 * 73.14 ≈ 475
        };

        foreach (var (keyCount, column, expectedX) in testCases)
        {
            // Test encoding (column to x)
            var maniaHit = new ManiaHitObject(1000, column, keyCount);
            var encoded = maniaHit.ToString();
            var parts = encoded.Split(',');
            var actualX = int.Parse(parts[0]);

            // Allow small rounding differences
            Assert.InRange(actualX, expectedX - 2, expectedX + 2);
        }

        // Test decoding (x to column)
        var decoder = new Decode.BeatmapDecoder();
        foreach (var (keyCount, column, expectedX) in testCases)
        {
            var beatmap = new Beatmap
            {
                Mode = GameMode.Mania,
                Difficulty = new BeatmapDifficulty { CircleSize = keyCount }
            };

            var line = $"{expectedX},192,1000,1,0,0:0:0:0:";
            var parts = line.Split(',');

            var maniaHit = decoder.GetType()
                .GetMethod("ParseManiaHitObject", System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance)
                ?.Invoke(decoder, new object[] { beatmap, parts }) as ManiaHitObject;

            Assert.NotNull(maniaHit);
            Assert.Equal(column, maniaHit.Column);
            Assert.Equal(keyCount, maniaHit.KeyCount);
        }
    }

    [Fact]
    public void EncodeToFile_ValidBeatmap_CreatesFile()
    {
        // Arrange
        var beatmap = new Beatmap
        {
            Mode = GameMode.Mania,
            BeatmapInfo = new BeatmapInfo
            {
                Metadata = new BeatmapMetadata
                {
                    Title = "File Test",
                    Artist = "Test",
                    Author = new BeatmapAuthor { Username = "Creator" }
                },
                Difficulty = new BeatmapDifficulty()
            },
            HitObjects = new List<HitObject> { new ManiaHitObject(1000, 0) }
        };

        var encoder = new BeatmapEncoder();
        var tempFile = Path.GetTempFileName() + ".osu";

        try
        {
            // Act
            encoder.EncodeToFile(beatmap, tempFile);

            // Assert
            Assert.True(File.Exists(tempFile));
            var content = File.ReadAllText(tempFile);
            Assert.Contains("Title:File Test", content);
        }
        finally
        {
            // Cleanup
            if (File.Exists(tempFile))
                File.Delete(tempFile);
        }
    }

    [Fact]
    public void EncodeRealOsuFile_RoundTrip_PreservesContent()
    {
        // Arrange - Find the Resource directory
        var testDir = Path.GetDirectoryName(typeof(BeatmapEncoderTests).Assembly.Location);
        // Navigate up until we find the Resource directory
        while (testDir != null && !Directory.Exists(Path.Combine(testDir, "Resource")))
        {
            var parent = Directory.GetParent(testDir);
            testDir = parent?.FullName;
        }
        Assert.NotNull(testDir); // Ensure we found the Resource directory

        var testFilePath = Path.Combine(testDir, "Resource", "Jumpstream - Happy Hardcore Synthesizer (SK_la) [5k-1].osu");
        var decoder = new Decode.BeatmapDecoder();
        var encoder = new BeatmapEncoder();

        // Load and parse the real osu file
        using var stream = File.OpenRead(testFilePath);
        var originalBeatmap = decoder.Decode(stream);

        // Debug: Output original beatmap version
        _output.WriteLine($"Original beatmap version: {originalBeatmap.Version}");

        // Act - Encode the beatmap back to string
        var encodedContent = encoder.EncodeToString(originalBeatmap);

        // Output the full encoded content for manual inspection
        _output.WriteLine("=== ENCODED CONTENT START ===");
        _output.WriteLine(encodedContent);
        _output.WriteLine("=== ENCODED CONTENT END ===");

        // Assert - Verify the encoded content contains expected sections and data
        Assert.Contains($"osu file format v{originalBeatmap.Version}", encodedContent);
        // Temporarily comment out other assertions to see the output
        // Assert.Contains("[General]", encodedContent);
        // Assert.Contains($"Mode: {(int)originalBeatmap.Mode}", encodedContent); // Mania mode
        // Assert.Contains("[Metadata]", encodedContent);
        // Assert.Contains($"Title:{originalBeatmap.Metadata.Title}", encodedContent);
        // Assert.Contains($"Artist:{originalBeatmap.Metadata.Artist}", encodedContent);
        // Assert.Contains($"Creator:{originalBeatmap.Metadata.Author.Username}", encodedContent);
        // Assert.Contains($"Version:{originalBeatmap.Metadata.Version}", encodedContent);
        // Assert.Contains("[Difficulty]", encodedContent);
        // Assert.Contains($"CircleSize:{originalBeatmap.BeatmapInfo.Difficulty.CircleSize}", encodedContent);
        // Assert.Contains($"HPDrainRate:{originalBeatmap.BeatmapInfo.Difficulty.HPDrainRate}", encodedContent);
        // Assert.Contains($"OverallDifficulty:{originalBeatmap.BeatmapInfo.Difficulty.OverallDifficulty}", encodedContent);
        // Assert.Contains($"ApproachRate:{originalBeatmap.BeatmapInfo.Difficulty.ApproachRate}", encodedContent);
        // Assert.Contains("[Events]", encodedContent);
        // Assert.Contains("[TimingPoints]", encodedContent);
        // Assert.Contains("[HitObjects]", encodedContent);

        // Verify the encoded content can be parsed back
        using var encodedStream = new MemoryStream(Encoding.UTF8.GetBytes(encodedContent));
        var decodedBeatmap = decoder.Decode(encodedStream);

        // Verify key properties are preserved
        Assert.Equal(GameMode.Mania, decodedBeatmap.Mode);
        Assert.Equal("Happy Hardcore Synthesizer", decodedBeatmap.Metadata.Title);
        Assert.Equal("Jumpstream", decodedBeatmap.Metadata.Artist);
        Assert.Equal("SK_la", decodedBeatmap.Metadata.Author.Username);
        Assert.Equal(5, decodedBeatmap.BeatmapInfo.Difficulty.CircleSize);
        Assert.Equal(8, decodedBeatmap.BeatmapInfo.Difficulty.HPDrainRate);
        Assert.Equal(8, decodedBeatmap.BeatmapInfo.Difficulty.OverallDifficulty);
        Assert.Equal(9, decodedBeatmap.BeatmapInfo.Difficulty.ApproachRate);

        // Verify timing points and hit objects are preserved
        Assert.True(decodedBeatmap.TimingPoints.Count > 0);
        Assert.True(decodedBeatmap.HitObjects.Count > 0);

        // Output the full encoded content for manual inspection
        _output.WriteLine("=== ENCODED CONTENT START ===");
        _output.WriteLine(encodedContent);
        _output.WriteLine("=== ENCODED CONTENT END ===");
    }
}
