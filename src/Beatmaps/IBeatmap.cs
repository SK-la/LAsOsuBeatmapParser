using System.Collections.Generic;
using LAsOsuBeatmapParser.Beatmaps;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// A materialised beatmap.
/// Generally this interface will be implemented alongside <see cref="IBeatmap{T}"/>, which exposes the ruleset-typed hit objects.
/// </summary>
public interface IBeatmap
{
    /// <summary>
    /// This beatmap's info.
    /// </summary>
    BeatmapInfo BeatmapInfo { get; set; }

    /// <summary>
    /// This beatmap's metadata.
    /// </summary>
    BeatmapMetadata Metadata { get; }

    /// <summary>
    /// This beatmap's difficulty settings.
    /// </summary>
    BeatmapDifficulty Difficulty { get; set; }

    /// <summary>
    /// The control points in this beatmap.
    /// </summary>
    ControlPointInfo ControlPointInfo { get; set; }

    /// <summary>
    /// The breaks in this beatmap.
    /// </summary>
    SortedSet<BreakPeriod> Breaks { get; set; }

    /// <summary>
    /// Creates a deep clone of this beatmap.
    /// </summary>
    /// <returns>The cloned beatmap.</returns>
    IBeatmap Clone();
}

/// <summary>
/// A materialised beatmap containing converted HitObjects.
/// </summary>
public interface IBeatmap<T> : IBeatmap
    where T : HitObject
{
    /// <summary>
    /// The hit objects in this beatmap.
    /// </summary>
    List<T> HitObjects { get; set; }
}
