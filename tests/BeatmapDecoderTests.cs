using System.Collections.Generic;
using System.Linq;
using Xunit;
using System.IO;
using System.Text;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Beatmaps.Formats;
using LAsOsuBeatmapParser.Exceptions;
using Xunit.Abstractions;

namespace LAsOsuBeatmapParser.Tests
{
    public class BeatmapDecoderTests
    {
        private readonly ITestOutputHelper _testOutputHelper;

        public BeatmapDecoderTests(ITestOutputHelper testOutputHelper)
        {
            _testOutputHelper = testOutputHelper;
        }

        [Fact]
        public void Decode_ValidOsuFile_ReturnsBeatmap()
        {
            // Arrange
            string osuContent = @"osu file format v14

[General]
Mode: 3

[Metadata]
Title:Test Song
Artist:Test Artist
Creator:Test Creator
Version:Easy

[Difficulty]
HPDrainRate:5
CircleSize:4
OverallDifficulty:5
ApproachRate:5

[TimingPoints]
0,500,4,1,1,100,1,0

[HitObjects]
64,192,1000,1,0,0:0:0:0:
192,192,1500,1,0,0:0:0:0:
320,192,2000,1,0,0:0:0:0:
448,192,2500,1,0,0:0:0:0:
";

            using var stream = new MemoryStream(Encoding.UTF8.GetBytes(osuContent));
            var decoder = new LegacyBeatmapDecoder();

            // Act
            Beatmap beatmap = decoder.Decode(stream);

            // Assert
            Assert.Equal(GameMode.Mania, beatmap.Mode);
            Assert.Equal("Test Song", beatmap.Metadata.Title);
            Assert.Equal("Test Artist", beatmap.Metadata.Artist);
            Assert.Equal(4, beatmap.HitObjects.Count);
            Assert.Equal(4, beatmap.Difficulty.CircleSize);
            Assert.True(beatmap.BPM > 0); // BPM should be calculated
            Assert.True(beatmap.Matrix.Count > 0); // Matrix should be built
        }

        public static IEnumerable<object[]> GetAllOsuFiles()
        {
            // 获取 tests 目录的绝对路径
            string? testDir = Path.GetDirectoryName(typeof(BeatmapDecoderTests).Assembly.Location);

            // 向上查找直到找到 Resource 目录
            while (testDir != null && !Directory.Exists(Path.Combine(testDir, "Resource")))
            {
                DirectoryInfo? parent = Directory.GetParent(testDir);
                testDir = parent?.FullName;
            }

            if (testDir == null)
                yield break;
            string resourceDir = Path.Combine(testDir, "Resource");
            string[] files = Directory.GetFiles(resourceDir, "*.osu", SearchOption.AllDirectories);
            foreach (string file in files) yield return new object[] { file };
        }

        [Theory]
        [MemberData(nameof(GetAllOsuFiles))]
        public void DecodeEncode_RoundTrip_Consistency(string osuFilePath)
        {
            // Arrange - Decode original file
            using FileStream originalStream = File.OpenRead(osuFilePath);
            var decoder = new LegacyBeatmapDecoder();
            var encoder = new LegacyBeatmapEncoder();
            Beatmap originalBeatmap = decoder.Decode(originalStream);

            // Act - Encode then decode again
            string encodedContent = encoder.EncodeToString(originalBeatmap);
            using var encodedStream = new MemoryStream(Encoding.UTF8.GetBytes(encodedContent));
            Beatmap decodedBeatmap = decoder.Decode(encodedStream);

            // Assert - Verify round-trip consistency
            Assert.Equal(originalBeatmap.Version, decodedBeatmap.Version);
            Assert.Equal(originalBeatmap.Mode, decodedBeatmap.Mode);

            // Metadata consistency
            Assert.Equal(originalBeatmap.Metadata.Title, decodedBeatmap.Metadata.Title);
            Assert.Equal(originalBeatmap.Metadata.Artist, decodedBeatmap.Metadata.Artist);
            Assert.Equal(originalBeatmap.Metadata.Author.Username, decodedBeatmap.Metadata.Author.Username);
            Assert.Equal(originalBeatmap.Metadata.Version, decodedBeatmap.Metadata.Version);
            Assert.Equal(originalBeatmap.Metadata.BeatmapID, decodedBeatmap.Metadata.BeatmapID);
            Assert.Equal(originalBeatmap.Metadata.BeatmapSetID, decodedBeatmap.Metadata.BeatmapSetID);

            // Difficulty consistency
            Assert.Equal(originalBeatmap.BeatmapInfo.Difficulty.CircleSize, decodedBeatmap.BeatmapInfo.Difficulty.CircleSize);
            Assert.Equal(originalBeatmap.BeatmapInfo.Difficulty.HPDrainRate, decodedBeatmap.BeatmapInfo.Difficulty.HPDrainRate);
            Assert.Equal(originalBeatmap.BeatmapInfo.Difficulty.OverallDifficulty, decodedBeatmap.BeatmapInfo.Difficulty.OverallDifficulty);
            Assert.Equal(originalBeatmap.BeatmapInfo.Difficulty.ApproachRate, decodedBeatmap.BeatmapInfo.Difficulty.ApproachRate);

            // Timing points consistency
            Assert.Equal(originalBeatmap.TimingPoints.Count, decodedBeatmap.TimingPoints.Count);

            for (int i = 0; i < originalBeatmap.TimingPoints.Count; i++)
            {
                TimingPoint originalTp = originalBeatmap.TimingPoints[i];
                TimingPoint decodedTp = decodedBeatmap.TimingPoints[i];
                Assert.Equal(originalTp.Time, decodedTp.Time);
                Assert.Equal(originalTp.BeatLength, decodedTp.BeatLength);
                Assert.Equal(originalTp.Meter, decodedTp.Meter);
                Assert.Equal(originalTp.SampleSet, decodedTp.SampleSet);
                Assert.Equal(originalTp.SampleIndex, decodedTp.SampleIndex);
                Assert.Equal(originalTp.Volume, decodedTp.Volume);
                Assert.Equal(originalTp.Inherited, decodedTp.Inherited);
                Assert.Equal(originalTp.Effects, decodedTp.Effects);
            } // Hit objects consistency

            Assert.Equal(originalBeatmap.HitObjects.Count, decodedBeatmap.HitObjects.Count);

            for (int i = 0; i < originalBeatmap.HitObjects.Count; i++)
            {
                HitObject originalHo = originalBeatmap.HitObjects[i];
                HitObject decodedHo = decodedBeatmap.HitObjects[i];

                // Check common properties
                Assert.Equal(originalHo.StartTime, decodedHo.StartTime);

                // Check specific types
                if (originalHo is ManiaHitObject originalMania && decodedHo is ManiaHitObject decodedMania)
                {
                    Assert.Equal(originalMania.Column, decodedMania.Column);
                    Assert.Equal(originalMania.KeyCount, decodedMania.KeyCount);
                }
                else if (originalHo is ManiaHoldNote originalHold && decodedHo is ManiaHoldNote decodedHold)
                {
                    Assert.Equal(originalHold.Column, decodedHold.Column);
                    Assert.Equal(originalHold.EndTime, decodedHold.EndTime);
                    Assert.Equal(originalHold.KeyCount, decodedHold.KeyCount);
                }
            }

            // BPM and Matrix should be recalculated consistently
            Assert.Equal(originalBeatmap.BPM, decodedBeatmap.BPM);
            Assert.Equal(originalBeatmap.Matrix.Count, decodedBeatmap.Matrix.Count);
        }

        [Fact]
        public void EncodeRealOsuFile_RoundTrip_PreservesContent()
        {
            // Arrange - Use specific test file
            string testDir = Path.GetDirectoryName(typeof(BeatmapDecoderTests).Assembly.Location) ?? "";
            string resourceDir = Path.Combine(testDir, "..", "..", "..", "Resource");
            string testFilePath = Path.Combine(resourceDir, "Jumpstream - Happy Hardcore Synthesizer (SK_la) [5k-1].osu");
            string originalContent = File.ReadAllText(testFilePath);

            _testOutputHelper.WriteLine("=== ORIGINAL FILE SUMMARY ===");
            _testOutputHelper.WriteLine($"File has {originalContent.Split('\n').Length} lines");
            _testOutputHelper.WriteLine($"Contains [General]: {originalContent.Contains("[General]")}");
            _testOutputHelper.WriteLine($"Contains [Metadata]: {originalContent.Contains("[Metadata]")}");
            _testOutputHelper.WriteLine($"Contains [Difficulty]: {originalContent.Contains("[Difficulty]")}");
            _testOutputHelper.WriteLine($"Contains [TimingPoints]: {originalContent.Contains("[TimingPoints]")}");
            _testOutputHelper.WriteLine($"Contains [HitObjects]: {originalContent.Contains("[HitObjects]")}");
            _testOutputHelper.WriteLine("=== END ORIGINAL SUMMARY ===");

            // Act - Decode and re-encode
            using FileStream originalStream = File.OpenRead(testFilePath);
            var decoder = new LegacyBeatmapDecoder();
            var encoder = new LegacyBeatmapEncoder();
            Beatmap beatmap = decoder.Decode(originalStream);

            _testOutputHelper.WriteLine("=== PARSED BEATMAP PROPERTIES ===");
            _testOutputHelper.WriteLine($"Version: {beatmap.Version}");
            _testOutputHelper.WriteLine($"Mode: {beatmap.Mode}");
            _testOutputHelper.WriteLine($"AudioFile: {beatmap.Metadata.AudioFile}");
            _testOutputHelper.WriteLine($"AudioLeadIn: {beatmap.AudioLeadIn}");
            _testOutputHelper.WriteLine($"PreviewTime: {beatmap.Metadata.PreviewTime}");
            _testOutputHelper.WriteLine($"Countdown: {beatmap.Countdown}");
            _testOutputHelper.WriteLine($"SampleSet: {beatmap.SampleSet}");
            _testOutputHelper.WriteLine($"StackLeniency: {beatmap.StackLeniency}");
            _testOutputHelper.WriteLine($"LetterboxInBreaks: {beatmap.LetterboxInBreaks}");
            _testOutputHelper.WriteLine($"WidescreenStoryboard: {beatmap.WidescreenStoryboard}");
            _testOutputHelper.WriteLine($"EpilepsyWarning: {beatmap.EpilepsyWarning}");
            _testOutputHelper.WriteLine($"CountdownOffset: {beatmap.CountdownOffset}");
            _testOutputHelper.WriteLine($"SpecialStyle: {beatmap.SpecialStyle}");
            _testOutputHelper.WriteLine($"SamplesMatchPlaybackRate: {beatmap.SamplesMatchPlaybackRate}");
            _testOutputHelper.WriteLine($"DistanceSpacing: {beatmap.DistanceSpacing}");
            _testOutputHelper.WriteLine($"GridSize: {beatmap.GridSize}");
            _testOutputHelper.WriteLine($"TimelineZoom: {beatmap.TimelineZoom}");
            _testOutputHelper.WriteLine($"Bookmarks: [{string.Join(", ", beatmap.Bookmarks)}]");
            _testOutputHelper.WriteLine($"Title: {beatmap.Metadata.Title}");
            _testOutputHelper.WriteLine($"Artist: {beatmap.Metadata.Artist}");
            _testOutputHelper.WriteLine($"Creator: {beatmap.Metadata.Author.Username}");
            _testOutputHelper.WriteLine($"Version (Metadata): {beatmap.Metadata.Version}");
            _testOutputHelper.WriteLine($"DifficultyName: {beatmap.BeatmapInfo.DifficultyName}");
            _testOutputHelper.WriteLine($"CircleSize: {beatmap.BeatmapInfo.Difficulty.CircleSize}");
            _testOutputHelper.WriteLine($"HPDrainRate: {beatmap.BeatmapInfo.Difficulty.HPDrainRate}");
            _testOutputHelper.WriteLine($"OverallDifficulty: {beatmap.BeatmapInfo.Difficulty.OverallDifficulty}");
            _testOutputHelper.WriteLine($"ApproachRate: {beatmap.BeatmapInfo.Difficulty.ApproachRate}");
            _testOutputHelper.WriteLine($"TimingPoints Count: {beatmap.TimingPoints.Count}");
            _testOutputHelper.WriteLine($"HitObjects Count: {beatmap.HitObjects.Count}");
            _testOutputHelper.WriteLine($"BPM: {beatmap.BPM}");
            _testOutputHelper.WriteLine("=== END PARSED PROPERTIES ===");

            string encodedContent = encoder.EncodeToString(beatmap);

            _testOutputHelper.WriteLine("=== ENCODED FILE SUMMARY ===");
            _testOutputHelper.WriteLine($"Encoded content has {encodedContent.Split('\n').Length} lines");
            _testOutputHelper.WriteLine($"Contains [General]: {encodedContent.Contains("[General]")}");
            _testOutputHelper.WriteLine($"Contains [Metadata]: {encodedContent.Contains("[Metadata]")}");
            _testOutputHelper.WriteLine($"Contains [Difficulty]: {encodedContent.Contains("[Difficulty]")}");
            _testOutputHelper.WriteLine($"Contains [TimingPoints]: {encodedContent.Contains("[TimingPoints]")}");
            _testOutputHelper.WriteLine($"Contains [HitObjects]: {encodedContent.Contains("[HitObjects]")}");
            _testOutputHelper.WriteLine("=== END ENCODED SUMMARY ===");

            // Save encoded content to file for inspection
            string outputFilePath = Path.Combine(resourceDir, "encoded_output.osu");
            File.WriteAllText(outputFilePath, encodedContent);
            _testOutputHelper.WriteLine($"Encoded content saved to: {outputFilePath}");

            // Assert - Check that key sections are present
            Assert.Contains("[General]", encodedContent);
            Assert.Contains("[Metadata]", encodedContent);
            Assert.Contains("[Difficulty]", encodedContent);
            Assert.Contains("[TimingPoints]", encodedContent);
            Assert.Contains("[HitObjects]", encodedContent);

            // Check that metadata is preserved
            Assert.Contains($"Title:{beatmap.Metadata.Title}", encodedContent);
            Assert.Contains($"Artist:{beatmap.Metadata.Artist}", encodedContent);
            Assert.Contains($"Creator:{beatmap.Metadata.Author.Username}", encodedContent);
            Assert.Contains($"Version:{beatmap.BeatmapInfo.DifficultyName}", encodedContent);
        }

        [Fact]
        public void EncodeRealOsuFile_FieldByFieldComparison()
        {
            // Arrange - Use specific test file
            string testDir = Path.GetDirectoryName(typeof(BeatmapDecoderTests).Assembly.Location) ?? "";
            string resourceDir = Path.Combine(testDir, "..", "..", "..", "Resource");
            string testFilePath = Path.Combine(resourceDir, "Various Artists - la's 10K -SUPERMUG-", "Various Artists - la's 10K -SUPERMUG- (SK_la) [Caramell - Caramelldansen (Ryu Remix)  EX].osu");

            // Act - Decode and re-encode
            using FileStream originalStream = File.OpenRead(testFilePath);
            var decoder = new LegacyBeatmapDecoder();
            var encoder = new LegacyBeatmapEncoder();
            Beatmap beatmap = decoder.Decode(originalStream);
            string encodedContent = encoder.EncodeToString(beatmap);

            // Parse both original and encoded content into dictionaries for comparison
            Dictionary<string, Dictionary<string, string>> originalFields = ParseOsuFileToFields(File.ReadAllText(testFilePath));
            Dictionary<string, Dictionary<string, string>> encodedFields = ParseOsuFileToFields(encodedContent);

            _testOutputHelper.WriteLine("=== FIELD-BY-FIELD COMPARISON ===");

            bool allFieldsMatch = true;

            // Compare all sections that exist in original
            foreach (string sectionName in originalFields.Keys)
            {
                _testOutputHelper.WriteLine($"{sectionName} Section:");
                bool sectionMatches = CompareSectionFields(originalFields, encodedFields, sectionName, _testOutputHelper);
                if (!sectionMatches) allFieldsMatch = false;
            }

            // Check for extra sections in encoded that weren't in original
            foreach (string sectionName in encodedFields.Keys)
            {
                if (!originalFields.ContainsKey(sectionName))
                {
                    _testOutputHelper.WriteLine($"{sectionName} Section: (extra section in encoded)");
                    allFieldsMatch = false;
                }
            }

            Assert.True(allFieldsMatch, "Some fields did not match between original and encoded content");

            _testOutputHelper.WriteLine("=== COMPARISON COMPLETE ===");
        }

        private Dictionary<string, Dictionary<string, string>> ParseOsuFileToFields(string content)
        {
            var result = new Dictionary<string, Dictionary<string, string>>();
            string[] lines = content.Split('\n');
            string currentSection = "";

            foreach (string line in lines)
            {
                string trimmedLine = line.Trim();
                if (string.IsNullOrEmpty(trimmedLine) || trimmedLine.StartsWith("//"))
                    continue;

                if (trimmedLine.StartsWith("[") && trimmedLine.EndsWith("]"))
                {
                    currentSection = trimmedLine[1..^1];
                    result[currentSection] = new Dictionary<string, string>();
                    continue;
                }

                if (!string.IsNullOrEmpty(currentSection) && trimmedLine.Contains(":"))
                {
                    string[] parts = trimmedLine.Split(':', 2);

                    if (parts.Length == 2)
                    {
                        string key = parts[0].Trim();
                        string value = parts[1].Trim();
                        result[currentSection][key] = value;
                    }
                }
            }

            return result;
        }

        private bool CompareSectionFields(
            Dictionary<string, Dictionary<string, string>> original,
            Dictionary<string, Dictionary<string, string>> encoded,
            string sectionName,
            ITestOutputHelper output)
        {
            bool sectionMatches = true;

            if (!original.ContainsKey(sectionName))
            {
                output.WriteLine($"  Section '{sectionName}' not found in original");
                return false;
            }

            if (!encoded.ContainsKey(sectionName))
            {
                output.WriteLine($"  Section '{sectionName}' not found in encoded");
                return false;
            }

            Dictionary<string, string> originalFields = original[sectionName];
            Dictionary<string, string> encodedFields = encoded[sectionName];

            if (sectionName == "HitObjects")
            {
                // Special handling for HitObjects: only check note count and first 20 timelines
                int originalCount = originalFields.Count;
                int encodedCount = encodedFields.Count;
                bool countMatch = originalCount == encodedCount;
                output.WriteLine($"  Note count: {originalCount} -> {encodedCount} {(countMatch ? "✓" : "✗")}");
                if (!countMatch) sectionMatches = false;

                // Check first 20 timelines (timestamps)
                int minCount = System.Math.Min(originalCount, encodedCount);
                int checkCount = System.Math.Min(20, minCount);
                var originalKeys = new List<string>(originalFields.Keys);
                var encodedKeys = new List<string>(encodedFields.Keys);

                for (int i = 0; i < checkCount; i++)
                {
                    string originalKey = originalKeys[i];
                    string encodedKey = encodedKeys[i];
                    // Extract timestamp (third comma-separated value)
                    string originalTimestamp = originalKey.Split(',')[2];
                    string encodedTimestamp = encodedKey.Split(',')[2];
                    bool match = originalTimestamp == encodedTimestamp;
                    output.WriteLine($"  Timeline {i + 1}: '{originalTimestamp}' -> '{encodedTimestamp}' {(match ? "✓" : "✗")}");
                    if (!match) sectionMatches = false;
                }

                return sectionMatches;
            }

            foreach (KeyValuePair<string, string> field in originalFields)
            {
                if (encodedFields.ContainsKey(field.Key))
                {
                    string originalValue = field.Value;
                    string encodedValue = encodedFields[field.Key];
                    bool match = originalValue == encodedValue;
                    output.WriteLine($"  {field.Key}: '{originalValue}' -> '{encodedValue}' {(match ? "✓" : "✗")}");
                    if (!match) sectionMatches = false;
                }
                else
                {
                    output.WriteLine($"  {field.Key}: '{field.Value}' -> (missing) ✗");
                    sectionMatches = false;
                }
            }

            // Check for extra fields in encoded that weren't in original
            foreach (KeyValuePair<string, string> field in encodedFields)
            {
                if (!originalFields.ContainsKey(field.Key))
                {
                    output.WriteLine($"  {field.Key}: (not in original) -> '{field.Value}' ⚠");
                    sectionMatches = false;
                }
            }

            return sectionMatches;
        }

        [Fact]
        public void GetStatistics_ReturnsValidStatistics()
        {
            // 获取 tests 目录的绝对路径
            string? testDir = Path.GetDirectoryName(typeof(BeatmapDecoderTests).Assembly.Location);

            // 向上查找直到找到 Resource 目录
            while (testDir != null && !Directory.Exists(Path.Combine(testDir, "Resource")))
            {
                DirectoryInfo? parent = Directory.GetParent(testDir);
                testDir = parent?.FullName;
            }

            string testFile = Path.Combine(testDir ?? "", "Resource", "Jumpstream - Happy Hardcore Synthesizer (SK_la) [10k-1].osu");
            var decoder = new LegacyBeatmapDecoder();

            // Act
            Beatmap beatmap = decoder.Decode(testFile);
            var statistics = beatmap.GetStatistics().ToList();

            // Assert
            Assert.NotNull(statistics);
            Assert.Single(statistics);
            var stat = statistics[0];
            Assert.Equal("Hit Objects", stat.Name);
            Assert.Equal(beatmap.HitObjects.Count.ToString(), stat.Content);
            Assert.True(stat.TotalNotes > 0);
            Assert.True(stat.TotalDuration > 0);
            Assert.True(stat.SR >= 0);
        }
    }
}
