using System;
using System.Collections.Generic;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// Provides functionality to convert a <see cref="IBeatmap"/> for a <see cref="IRulesetInfo"/>.
/// 提供将 <see cref="IBeatmap"/> 转换为特定 <see cref="IRulesetInfo"/> 的功能。
/// </summary>
public interface IBeatmapConverter
{
    /// <summary>
    /// Invoked when a <see cref="HitObject"/> has been converted.
    /// The first argument contains the <see cref="HitObject"/> that was converted.
    /// The second argument contains the <see cref="HitObject"/>s that were output from the conversion process.
    /// </summary>
    event Action<HitObject, IEnumerable<HitObject>> ObjectConverted;

    /// <summary>
    /// The <see cref="IBeatmap"/> to convert.
    /// </summary>
    IBeatmap Beatmap { get; }

    /// <summary>
    /// Whether <see cref="Beatmap"/> can be converted by this <see cref="IBeatmapConverter"/>.
    /// </summary>
    bool CanConvert();

    /// <summary>
    /// Converts <see cref="Beatmap"/>.
    /// </summary>
    /// <param name="cancellationToken">The cancellation token.</param>
    /// <returns>The converted Beatmap.</returns>
    IBeatmap Convert(System.Threading.CancellationToken cancellationToken = default);
}
