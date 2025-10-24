using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json.Serialization;
using LAsOsuBeatmapParser.Beatmaps.ControlPoints;
using LAsOsuBeatmapParser.Converters;
using LAsOsuBeatmapParser.FakeFrameWork;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// 表示一个完整的osu!谱面。
/// </summary>
public class Beatmap<T> : IBeatmap<T>
    where T : HitObject
{
    private BeatmapDifficulty difficulty = new BeatmapDifficulty();

    /// <summary>
    /// This beatmap's difficulty settings.
    /// </summary>
    public BeatmapDifficulty Difficulty
    {
        get => difficulty;
        set
        {
            difficulty = value;
            beatmapInfo.Difficulty = difficulty.Clone();
        }
    }

    private BeatmapInfo beatmapInfo;

    /// <summary>
    /// This beatmap's info.
    /// </summary>
    public BeatmapInfo BeatmapInfo
    {
        get => beatmapInfo;
        set
        {
            beatmapInfo = value;
            Difficulty = beatmapInfo.Difficulty.Clone();
        }
    }

    /// <summary>
    /// This beatmap's metadata.
    /// </summary>
    [JsonIgnore]
    public BeatmapMetadata Metadata => BeatmapInfo.Metadata;

    /// <summary>
    /// The control points in this beatmap.
    /// </summary>
    public ControlPointInfo ControlPointInfo { get; set; } = new ControlPointInfo();

    /// <summary>
    /// The breaks in this beatmap.
    /// </summary>
    public SortedList<BreakPeriod> Breaks { get; set; } = new SortedList<BreakPeriod>();

    /// <summary>
    /// 谱面元数据。
    /// </summary>
    [JsonIgnore]
    public BeatmapMetadata MetadataLegacy { get; set; } = new();

    /// <summary>
    /// 难度设置。
    /// </summary>
    [JsonIgnore]
    public BeatmapDifficulty DifficultyLegacy { get; set; } = new();

    /// <summary>
    /// TimingPoint列表。
    /// </summary>
    [JsonIgnore]
    public List<TimingPoint> TimingPoints { get; set; } = new();

    /// <summary>
    /// HitObject列表。
    /// </summary>
    [JsonConverter(typeof(HitObjectListConverter))]
    public List<T> HitObjects { get; set; } = new();

    /// <summary>
    /// 事件（背景、视频、故事板等）。
    /// </summary>
    public List<Event> Events { get; set; } = new();

    /// <summary>
    /// 游戏模式（Standard、Taiko、Catch、Mania）。
    /// </summary>
    [JsonIgnore]
    public IGameMode Mode { get; set; } = GameMode.Standard;

    /// <summary>
    /// 谱面版本。
    /// </summary>
    public int Version { get; set; }

    /// <summary>
    /// 谱面计算得到的BPM。
    /// </summary>
    public double BPM { get; set; }

    /// <summary>
    /// 用于分析工具的时间-音符矩阵。
    /// </summary>
    public Dictionary<double, List<T>> Matrix { get; set; } = new();

    /// <summary>
    /// Letterbox in breaks.
    /// </summary>
    public bool LetterboxInBreaks { get; set; }

    /// <summary>
    /// Letterbox in breaks was explicitly set.
    /// </summary>
    public bool LetterboxInBreaksSet { get; set; }

    /// <summary>
    /// Total amount of break time in the beatmap.
    /// </summary>
    public double TotalBreakTime => Breaks.Sum(b => b.Duration);

    /// <summary>
    /// Finds the most common beat length represented by the control points in this beatmap.
    /// </summary>
    public double GetMostCommonBeatLength()
    {
        if (!ControlPointInfo.TimingPoints.Any())
            return 1000; // Default

        var groups = ControlPointInfo.TimingPoints
            .GroupBy(t => Math.Round(t.BeatLength * 1000) / 1000)
            .Select(g => new { BeatLength = g.Key, Count = g.Count() })
            .OrderByDescending(g => g.Count)
            .First();

        return groups.BeatLength;
    }

    /// <summary>
    /// Returns statistics for the <see cref="HitObjects"/> contained in this beatmap.
    /// </summary>
    public virtual IEnumerable<BeatmapStatistic> GetStatistics() => new[]
    {
        new BeatmapStatistic
        {
            Name = "Hit Objects",
            Content = HitObjects.Count.ToString()
        }
    };

    /// <summary>
    /// Widescreen storyboard.
    /// </summary>
    public bool WidescreenStoryboard { get; set; } = true;

    /// <summary>
    /// Widescreen storyboard was explicitly set.
    /// </summary>
    public bool WidescreenStoryboardSet { get; set; }

    /// <summary>
    /// Epilepsy warning.
    /// </summary>
    public bool EpilepsyWarning { get; set; }

    /// <summary>
    /// Epilepsy warning was explicitly set.
    /// </summary>
    public bool EpilepsyWarningSet { get; set; }

    /// <summary>
    /// Samples match playback rate.
    /// </summary>
    public bool SamplesMatchPlaybackRate { get; set; }

    /// <summary>
    /// Samples match playback rate was explicitly set.
    /// </summary>
    public bool SamplesMatchPlaybackRateSet { get; set; }

    /// <summary>
    /// Distance spacing.
    /// </summary>
    public double DistanceSpacing { get; set; } = 1.0;

    /// <summary>
    /// Distance spacing was explicitly set.
    /// </summary>
    public bool DistanceSpacingSet { get; set; }

    /// <summary>
    /// Grid size.
    /// </summary>
    public int GridSize { get; set; }

    /// <summary>
    /// Grid size was explicitly set.
    /// </summary>
    public bool GridSizeSet { get; set; }

    /// <summary>
    /// Timeline zoom.
    /// </summary>
    public double TimelineZoom { get; set; } = 1.0;

    /// <summary>
    /// Timeline zoom was explicitly set.
    /// </summary>
    public bool TimelineZoomSet { get; set; }

    /// <summary>
    /// Countdown type.
    /// </summary>
    public CountdownType Countdown { get; set; } = CountdownType.None;

    /// <summary>
    /// Countdown was explicitly set.
    /// </summary>
    public bool CountdownSet { get; set; }

    /// <summary>
    /// Countdown offset.
    /// </summary>
    public int CountdownOffset { get; set; }

    /// <summary>
    /// Countdown offset was explicitly set.
    /// </summary>
    public bool CountdownOffsetSet { get; set; }

    /// <summary>
    /// Bookmarks.
    /// </summary>
    public int[] Bookmarks { get; set; } = new int[0];

    /// <summary>
    /// Bookmarks was explicitly set.
    /// </summary>
    public bool BookmarksSet { get; set; }

    /// <summary>
    /// Audio lead-in time.
    /// </summary>
    public int AudioLeadIn { get; set; }

    /// <summary>
    /// Audio lead-in was explicitly set.
    /// </summary>
    public bool AudioLeadInSet { get; set; }

    /// <summary>
    /// Sample set.
    /// </summary>
    public string SampleSet { get; set; } = "Normal";

    /// <summary>
    /// Sample set was explicitly set.
    /// </summary>
    public bool SampleSetSet { get; set; }

    /// <summary>
    /// Stack leniency.
    /// </summary>
    public double StackLeniency { get; set; } = 0.7;

    /// <summary>
    /// Stack leniency was explicitly set.
    /// </summary>
    public bool StackLeniencySet { get; set; }

    /// <summary>
    /// Special style.
    /// </summary>
    public bool SpecialStyle { get; set; }

    /// <summary>
    /// Special style was explicitly set.
    /// </summary>
    public bool SpecialStyleSet { get; set; }

    /// <summary>
    /// Story fire in front.
    /// </summary>
    public bool StoryFireInFront { get; set; } = true;

    /// <summary>
    /// Story fire in front was explicitly set.
    /// </summary>
    public bool StoryFireInFrontSet { get; set; }

    /// <summary>
    /// Use skin sprites.
    /// </summary>
    public bool UseSkinSprites { get; set; }

    /// <summary>
    /// Use skin sprites was explicitly set.
    /// </summary>
    public bool UseSkinSpritesSet { get; set; }

    /// <summary>
    /// Always show playfield.
    /// </summary>
    public bool AlwaysShowPlayfield { get; set; }

    /// <summary>
    /// Always show playfield was explicitly set.
    /// </summary>
    public bool AlwaysShowPlayfieldSet { get; set; }

    /// <summary>
    /// Overlay position.
    /// </summary>
    public string OverlayPosition { get; set; } = "NoChange";

    /// <summary>
    /// Overlay position was explicitly set.
    /// </summary>
    public bool OverlayPositionSet { get; set; }

    /// <summary>
    /// Skin preference.
    /// </summary>
    public string SkinPreference { get; set; } = string.Empty;

    /// <summary>
    /// Skin preference was explicitly set.
    /// </summary>
    public bool SkinPreferenceSet { get; set; }

    /// <summary>
    /// Beatmap version.
    /// </summary>
    public int BeatmapVersion { get; set; }

    /// <summary>
    /// Creates a new beatmap.
    /// </summary>
    public Beatmap()
    {
        beatmapInfo = new BeatmapInfo
        {
            Metadata = new BeatmapMetadata
            {
                Artist = @"Unknown",
                Title = @"Unknown",
                Author = { Username = @"Unknown Creator" },
            },
            DifficultyName = @"Normal",
            Difficulty = Difficulty,
        };
    }

    /// <summary>
    /// 获取指定时间的BPM值。
    /// </summary>
    /// <param name="time">时间（以毫秒为单位）。</param>
    /// <returns>BPM值。</returns>
    public double GetBPM(double time = 0)
    {
        // 查找给定时间之前的TimingPoint
        for (int i = TimingPoints.Count - 1; i >= 0; i--)
        {
            if (TimingPoints[i].Time <= time)
            {
                return 60000.0 / TimingPoints[i].BeatLength;
            }
        }
        return 120.0; // 默认BPM
    }

    /// <summary>
    /// 为分析工具构建时间-音符矩阵。
    /// </summary>
    /// <returns>映射时间到音符列表的字典。</returns>
    public Dictionary<double, List<T>> BuildMatrix()
    {
        var matrix = new Dictionary<double, List<T>>();
        foreach (var hitObject in HitObjects)
        {
            if (!matrix.ContainsKey(hitObject.StartTime))
            {
                matrix[hitObject.StartTime] = new List<T>();
            }
            matrix[hitObject.StartTime].Add(hitObject);
        }
        return matrix;
    }

    /// <summary>
    /// Clones this beatmap.
    /// </summary>
    /// <returns>The cloned beatmap.</returns>
    public Beatmap<T> Clone() => (Beatmap<T>)MemberwiseClone();

    /// <summary>
    /// Creates a deep clone of this beatmap.
    /// </summary>
    /// <returns>The cloned beatmap.</returns>
    IBeatmap IBeatmap.Clone() => Clone();

    /// <summary>
    /// Returns a string representation of this beatmap.
    /// </summary>
    /// <returns>The string representation.</returns>
    public override string ToString() => BeatmapInfo.ToString();
}

/// <summary>
/// 表示一个完整的osu!谱面。
/// </summary>
public class Beatmap : Beatmap<HitObject>
{
}
