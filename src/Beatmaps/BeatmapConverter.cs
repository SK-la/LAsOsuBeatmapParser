using System;
using System.Collections.Generic;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// Converts a Beatmap for another mode.
/// </summary>
/// <typeparam name="T">The type of HitObject stored in the Beatmap.</typeparam>
public abstract class BeatmapConverter<T> : IBeatmapConverter
    where T : HitObject
{
    /// <summary>
    /// Invoked when a <see cref="HitObject"/> has been converted.
    /// </summary>
    event Action<HitObject, IEnumerable<HitObject>>? IBeatmapConverter.ObjectConverted
    {
        add { }
        remove { }
    }

    /// <summary>
    /// The <see cref="IBeatmap"/> to convert.
    /// </summary>
    public IBeatmap Beatmap { get; }

    /// <summary>
    /// Creates a new <see cref="BeatmapConverter{T}"/>.
    /// </summary>
    /// <param name="beatmap">The beatmap to convert.</param>
    protected BeatmapConverter(IBeatmap beatmap)
    {
        Beatmap = beatmap;
    }

    /// <summary>
    /// Whether <see cref="Beatmap"/> can be converted by this <see cref="BeatmapConverter{T}"/>.
    /// </summary>
    public abstract bool CanConvert();

    /// <summary>
    /// Converts <see cref="Beatmap"/>.
    /// </summary>
    /// <param name="cancellationToken">The cancellation token.</param>
    /// <returns>The converted beatmap.</returns>
    public IBeatmap Convert(System.Threading.CancellationToken cancellationToken = default)
    {
        // We always operate on a clone of the original beatmap, to not modify it game-wide
        var original = Beatmap.Clone();

        // Used in osu!mania conversion.
        // Shallow clone isn't enough to ensure we don't mutate beatmap info unexpectedly.
        // Can potentially be removed after `Beatmap.Difficulty` doesn't save back to `Beatmap.BeatmapInfo`.
        original.BeatmapInfo = new BeatmapInfo
        {
            DifficultyName = original.BeatmapInfo.DifficultyName,
            Ruleset = original.BeatmapInfo.Ruleset,
            Difficulty = original.BeatmapInfo.Difficulty.Clone(),
            Metadata = original.BeatmapInfo.Metadata,
            LastPlayed = original.BeatmapInfo.LastPlayed,
            BeatDivisor = original.BeatmapInfo.BeatDivisor,
            EditorTimestamp = original.BeatmapInfo.EditorTimestamp
        };
        original.ControlPointInfo = new ControlPointInfo(); // Deep clone would be needed

        // Apply conversion
        return ConvertBeatmap(original, cancellationToken);
    }

    /// <summary>
    /// Performs the conversion of a beatmap.
    /// </summary>
    /// <param name="original">The original beatmap to convert.</param>
    /// <param name="cancellationToken">The cancellation token.</param>
    /// <returns>The converted beatmap.</returns>
    protected abstract IBeatmap ConvertBeatmap(IBeatmap original, System.Threading.CancellationToken cancellationToken);
}
