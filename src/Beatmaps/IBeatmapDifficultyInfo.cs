using System;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// A representation of all top-level difficulty settings for a beatmap.
/// </summary>
public interface IBeatmapDifficultyInfo : IEquatable<IBeatmapDifficultyInfo>
{
    /// <summary>
    /// The drain rate of the beatmap.
    /// </summary>
    float DrainRate { get; }

    /// <summary>
    /// The circle size of the beatmap.
    /// </summary>
    float CircleSize { get; }

    /// <summary>
    /// The overall difficulty of the beatmap.
    /// </summary>
    float OverallDifficulty { get; }

    /// <summary>
    /// The approach rate of the beatmap.
    /// </summary>
    float ApproachRate { get; }

    /// <summary>
    /// The slider multiplier of the beatmap.
    /// </summary>
    float SliderMultiplier { get; }

    /// <summary>
    /// The slider tick rate of the beatmap.
    /// </summary>
    float SliderTickRate { get; }
}
