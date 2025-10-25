using System.Collections.Generic;
using LAsOsuBeatmapParser.Beatmaps.ControlPoints;
using LAsOsuBeatmapParser.FakeFrameWork;

namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    ///     A materialised beatmap.
    ///     Generally this interface will be implemented alongside <see cref="IBeatmap{T}" />, which exposes the ruleset-typed
    ///     hit objects.
    /// </summary>
    public interface IBeatmap
    {
        /// <summary>
        ///     This beatmap's info.
        /// </summary>
        BeatmapInfo BeatmapInfo { get; set; }

        /// <summary>
        ///     This beatmap's metadata.
        /// </summary>
        BeatmapMetadata Metadata { get; }

        /// <summary>
        ///     This beatmap's difficulty settings.
        /// </summary>
        BeatmapDifficulty Difficulty { get; set; }

        /// <summary>
        ///     The control points in this beatmap.
        /// </summary>
        ControlPointInfo ControlPointInfo { get; set; }

        /// <summary>
        ///     The breaks in this beatmap.
        /// </summary>
        SortedList<BreakPeriod> Breaks { get; set; }

        /// <summary>
        ///     Creates a deep clone of this beatmap.
        /// </summary>
        /// <returns>The cloned beatmap.</returns>
        IBeatmap Clone();

        /// <summary>
        ///     Returns statistics for the <see cref="HitObjects" /> contained in this beatmap.
        /// </summary>
        IEnumerable<BeatmapStatistic> GetStatistics();

        /// <summary>
        ///     Finds the most common beat length represented by the control points in this beatmap.
        /// </summary>
        double GetMostCommonBeatLength();
    }

    /// <summary>
    ///     A materialised beatmap containing converted HitObjects.
    /// </summary>
    public interface IBeatmap<T> : IBeatmap
        where T : HitObject
    {
        /// <summary>
        ///     The hit objects in this beatmap.
        /// </summary>
        List<T> HitObjects { get; set; }
    }
}
