using System;
using System.IO;
using System.Linq;
using System.Text;
using LAsOsuBeatmapParser.Beatmaps;

namespace LAsOsuBeatmapParser.Beatmaps.Formats;

/// <summary>
/// osu! .osu 文件编码器。
/// </summary>
public class LegacyBeatmapEncoder
{
    /// <summary>
    /// 是否使用 lazer 版本格式 (v128)。
    /// </summary>
    public bool UseLazerVersion { get; set; }

    /// <summary>
    /// 创建 LegacyBeatmapEncoder。
    /// </summary>
    public LegacyBeatmapEncoder()
    {
    }

    /// <summary>
    /// 创建 LegacyBeatmapEncoder。
    /// </summary>
    /// <param name="useLazerVersion">是否使用 lazer 版本格式。</param>
    public LegacyBeatmapEncoder(bool useLazerVersion)
    {
        UseLazerVersion = useLazerVersion;
    }

    /// <summary>
    /// 将谱面编码为字符串。
    /// </summary>
    /// <param name="beatmap">谱面对象。</param>
    /// <returns>编码后的 .osu 格式字符串。</returns>
    public string EncodeToString(Beatmap beatmap)
    {
        var sb = new StringBuilder();

        // Version
        int version = UseLazerVersion ? 128 : beatmap.Version;
        sb.AppendLine($"osu file format v{version}");

        // General
        sb.AppendLine();
        sb.AppendLine("[General]");
        sb.AppendLine($"AudioFilename: {beatmap.Metadata.AudioFile}");
        sb.AppendLine($"AudioLeadIn: {beatmap.AudioLeadIn}");
        sb.AppendLine($"PreviewTime: {beatmap.Metadata.PreviewTime}");
        if (beatmap.CountdownSet)
            sb.AppendLine($"Countdown: {(int)beatmap.Countdown}");
        if (beatmap.SampleSetSet)
            sb.AppendLine($"SampleSet: {beatmap.SampleSet}");
        if (beatmap.StackLeniencySet)
            sb.AppendLine($"StackLeniency: {beatmap.StackLeniency}");
        sb.AppendLine($"Mode: {beatmap.Mode.Id}");
        if (beatmap.LetterboxInBreaksSet)
            sb.AppendLine($"LetterboxInBreaks: {(beatmap.LetterboxInBreaks ? 1 : 0)}");
        if (beatmap.StoryFireInFrontSet)
            sb.AppendLine($"StoryFireInFront: {(beatmap.StoryFireInFront ? 1 : 0)}");
        if (beatmap.UseSkinSpritesSet)
            sb.AppendLine($"UseSkinSprites: {(beatmap.UseSkinSprites ? 1 : 0)}");
        if (beatmap.AlwaysShowPlayfieldSet)
            sb.AppendLine($"AlwaysShowPlayfield: {(beatmap.AlwaysShowPlayfield ? 1 : 0)}");
        if (beatmap.OverlayPositionSet)
            sb.AppendLine($"OverlayPosition: {beatmap.OverlayPosition}");
        if (beatmap.SkinPreferenceSet)
            sb.AppendLine($"SkinPreference:{beatmap.SkinPreference}");
        if (beatmap.EpilepsyWarningSet)
            sb.AppendLine($"EpilepsyWarning: {(beatmap.EpilepsyWarning ? 1 : 0)}");
        if (beatmap.CountdownOffsetSet)
            sb.AppendLine($"CountdownOffset: {beatmap.CountdownOffset}");
        if (beatmap.SpecialStyleSet)
            sb.AppendLine($"SpecialStyle: {(beatmap.SpecialStyle ? 1 : 0)}");
        if (beatmap.WidescreenStoryboardSet)
            sb.AppendLine($"WidescreenStoryboard: {(beatmap.WidescreenStoryboard ? 1 : 0)}");
        if (beatmap.SamplesMatchPlaybackRateSet)
            sb.AppendLine($"SamplesMatchPlaybackRate: {(beatmap.SamplesMatchPlaybackRate ? 1 : 0)}");

        // Editor
        sb.AppendLine();
        sb.AppendLine("[Editor]");
        if (beatmap.BookmarksSet)
            sb.AppendLine($"Bookmarks: {string.Join(",", beatmap.Bookmarks)}");
        if (beatmap.DistanceSpacingSet)
            sb.AppendLine($"DistanceSpacing: {beatmap.DistanceSpacing}");
        sb.AppendLine($"BeatDivisor: {beatmap.BeatmapInfo.BeatDivisor}");
        if (beatmap.GridSizeSet)
            sb.AppendLine($"GridSize: {beatmap.GridSize}");
        if (beatmap.TimelineZoomSet)
            sb.AppendLine($"TimelineZoom: {beatmap.TimelineZoom}");

        // Metadata
        sb.AppendLine();
        sb.AppendLine("[Metadata]");
        sb.AppendLine($"Title:{beatmap.Metadata.Title}");
        sb.AppendLine($"TitleUnicode:{beatmap.Metadata.TitleUnicode}");
        sb.AppendLine($"Artist:{beatmap.Metadata.Artist}");
        sb.AppendLine($"ArtistUnicode:{beatmap.Metadata.ArtistUnicode}");
        sb.AppendLine($"Creator:{beatmap.Metadata.Author.Username}");
        sb.AppendLine($"Version:{beatmap.BeatmapInfo.DifficultyName}");
        sb.AppendLine($"Source:{beatmap.Metadata.Source}");
        sb.AppendLine($"Tags:{beatmap.Metadata.Tags}");
        sb.AppendLine($"BeatmapID:{beatmap.Metadata.BeatmapID}");
        sb.AppendLine($"BeatmapSetID:{beatmap.Metadata.BeatmapSetID}");

        // Difficulty
        sb.AppendLine();
        sb.AppendLine("[Difficulty]");
        sb.AppendLine($"HPDrainRate:{beatmap.Difficulty.HPDrainRate}");
        sb.AppendLine($"CircleSize:{beatmap.Difficulty.CircleSize}");
        sb.AppendLine($"OverallDifficulty:{beatmap.Difficulty.OverallDifficulty}");
        sb.AppendLine($"ApproachRate:{beatmap.Difficulty.ApproachRate}");
        sb.AppendLine($"SliderMultiplier:{beatmap.Difficulty.SliderMultiplier}");
        sb.AppendLine($"SliderTickRate:{beatmap.Difficulty.SliderTickRate}");

        // Events
        sb.AppendLine();
        sb.AppendLine("[Events]");
        foreach (var ev in beatmap.Events)
        {
            sb.AppendLine(ev.ToString());
        }

        // TimingPoints
        sb.AppendLine();
        sb.AppendLine("[TimingPoints]");
        foreach (var tp in beatmap.TimingPoints)
        {
            int uninherited = tp.Inherited ? 0 : 1;
            sb.AppendLine($"{tp.Time},{tp.BeatLength},{tp.Meter},{tp.SampleSet},{tp.SampleIndex},{tp.Volume},{uninherited},{tp.Effects}");
        }

        // HitObjects
        sb.AppendLine();
        sb.AppendLine("[HitObjects]");
        foreach (var hitObject in beatmap.HitObjects)
        {
            sb.AppendLine(hitObject.ToString());
        }

        return sb.ToString();
    }

    /// <summary>
    /// 将谱面编码到流。
    /// </summary>
    /// <param name="beatmap">谱面对象。</param>
    /// <param name="stream">目标流。</param>
    public void EncodeToStream(Beatmap beatmap, Stream stream)
    {
        using var writer = new StreamWriter(stream, Encoding.UTF8, leaveOpen: true);
        writer.Write(EncodeToString(beatmap));
    }

    /// <summary>
    /// 将谱面编码到文件。
    /// </summary>
    /// <param name="beatmap">谱面对象。</param>
    /// <param name="filePath">目标文件路径。</param>
    public void EncodeToFile(Beatmap beatmap, string filePath)
    {
        using var stream = File.Create(filePath);
        EncodeToStream(beatmap, stream);
    }
}
