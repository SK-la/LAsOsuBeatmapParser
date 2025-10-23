using System.Collections.Generic;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// A more expensive representation of a beatmap which allows access to various associated resources.
/// - Create a playable <see cref="Beatmap"/> via <see cref="GetPlayableBeatmap(IRulesetInfo,System.Collections.Generic.IReadOnlyList{Mod})"/>.
/// </summary>
public interface IWorkingBeatmap
{
    /// <summary>
    /// The <see cref="IBeatmapInfo"/> representing this working beatmap.
    /// </summary>
    IBeatmapInfo BeatmapInfo { get; }

    /// <summary>
    /// Whether the Beatmap has finished loading.
    /// </summary>
    bool BeatmapLoaded { get; }

    /// <summary>
    /// Whether the Track has finished loading.
    /// </summary>
    bool TrackLoaded { get; }

    /// <summary>
    /// Retrieves the <see cref="IBeatmap"/> which this <see cref="IWorkingBeatmap"/> represents.
    /// </summary>
    IBeatmap Beatmap { get; }

    /// <summary>
    /// Constructs a playable <see cref="IBeatmap"/> from <see cref="Beatmap"/> using the applicable converters for a specific <see cref="IRulesetInfo"/>.
    /// <para>
    /// The returned <see cref="IBeatmap"/> is in a playable state - all <see cref="HitObject"/> and <see cref="BeatmapDifficulty"/> <see cref="Mod"/>s
    /// have been applied, and <see cref="HitObject"/>s have been fully constructed.
    /// </para>
    /// </summary>
    /// <remarks>
    /// By default, the beatmap load process will be interrupted after 10 seconds.
    /// For finer-grained control over the load process, use the
    /// <see cref="GetPlayableBeatmap(IRulesetInfo,System.Collections.Generic.IReadOnlyList{Mod},System.Threading.CancellationToken)"/>
    /// overload instead.
    /// </remarks>
    /// <param name="ruleset">The <see cref="IRulesetInfo"/> to create a playable <see cref="IBeatmap"/> for.</param>
    /// <param name="mods">The <see cref="Mod"/>s to apply to the <see cref="IBeatmap"/>.</param>
    /// <returns>The converted <see cref="IBeatmap"/>.</returns>
    /// <exception cref="BeatmapInvalidForRulesetException">If <see cref="Beatmap"/> could not be converted to <paramref name="ruleset"/>.</exception>
    IBeatmap GetPlayableBeatmap(IRulesetInfo ruleset, IReadOnlyList<Mod>? mods = null);

    /// <summary>
    /// Constructs a playable <see cref="IBeatmap"/> from <see cref="Beatmap"/> using the applicable converters for a specific <see cref="IRulesetInfo"/>.
    /// <para>
    /// The returned <see cref="IBeatmap"/> is in a playable state - all <see cref="HitObject"/> and <see cref="BeatmapDifficulty"/> <see cref="Mod"/>s
    /// have been applied, and <see cref="HitObject"/>s have been fully constructed.
    /// </para>
    /// </summary>
    /// <param name="ruleset">The <see cref="IRulesetInfo"/> to create a playable <see cref="IBeatmap"/> for.</param>
    /// <param name="mods">The <see cref="Mod"/>s to apply to the <see cref="IBeatmap"/>.</param>
    /// <param name="cancellationToken">Cancellation token that cancels the beatmap loading process.</param>
    /// <returns>The converted <see cref="IBeatmap"/>.</returns>
    /// <exception cref="BeatmapInvalidForRulesetException">If <see cref="Beatmap"/> could not be converted to <paramref name="ruleset"/>.</exception>
    IBeatmap GetPlayableBeatmap(IRulesetInfo ruleset, IReadOnlyList<Mod> mods, System.Threading.CancellationToken cancellationToken);
}
