using System;
using System.Collections.Generic;

namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    ///     A representation of a collection of beatmap difficulties, generally packaged as an ".osz" archive.
    /// </summary>
    public interface IBeatmapSetInfo : IEquatable<IBeatmapSetInfo>
    {
        /// <summary>
        ///     The date when this beatmap was imported.
        /// </summary>
        DateTimeOffset DateAdded { get; }

        /// <summary>
        ///     The best-effort metadata representing this set. In the case metadata differs between contained beatmaps, one is
        ///     arbitrarily chosen.
        /// </summary>
        IBeatmapMetadataInfo Metadata { get; }

        /// <summary>
        ///     All beatmaps contained in this set.
        /// </summary>
        IEnumerable<IBeatmapInfo> Beatmaps { get; }

        /// <summary>
        ///     The maximum star difficulty of all beatmaps in this set.
        /// </summary>
        double MaxStarDifficulty { get; }

        /// <summary>
        ///     The maximum playable length in milliseconds of all beatmaps in this set.
        /// </summary>
        double MaxLength { get; }

        /// <summary>
        ///     The maximum BPM of all beatmaps in this set.
        /// </summary>
        double MaxBPM { get; }
    }
}
