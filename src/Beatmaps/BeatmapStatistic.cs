using System;
using System.Collections.Generic;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// Represents a statistic for display in the beatmap.
/// </summary>
public class BeatmapStatistic
{
    /// <summary>
    /// A function to create the icon for display purposes.
    /// </summary>
    public Func<object>? CreateIcon; // Simplified, since no Drawable

    /// <summary>
    /// The name of this statistic.
    /// </summary>
    public string Name { get; set; } = string.Empty;

    /// <summary>
    /// The text representing the value of this statistic.
    /// </summary>
    public string Content { get; set; } = string.Empty;

    /// <summary>
    /// The length of a bar which visually represents this statistic's relevance in the beatmap.
    /// </summary>
    public float? BarDisplayLength { get; set; }
}
