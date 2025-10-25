using System;

namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    ///     Metadata representing a beatmap. May be shared between multiple beatmap difficulties.
    /// </summary>
    public interface IBeatmapMetadataInfo : IEquatable<IBeatmapMetadataInfo>
    {
        /// <summary>
        ///     The romanised title of this beatmap.
        /// </summary>
        string Title { get; }

        /// <summary>
        ///     The unicode title of this beatmap.
        /// </summary>
        string TitleUnicode { get; }

        /// <summary>
        ///     The romanised artist of this beatmap.
        /// </summary>
        string Artist { get; }

        /// <summary>
        ///     The unicode artist of this beatmap.
        /// </summary>
        string ArtistUnicode { get; }

        /// <summary>
        ///     The author of this beatmap.
        /// </summary>
        IUser Author { get; }

        /// <summary>
        ///     The source of this beatmap.
        /// </summary>
        string Source { get; }

        /// <summary>
        ///     The tags of this beatmap.
        /// </summary>
        string Tags { get; }

        /// <summary>
        ///     The preview time of this beatmap.
        /// </summary>
        int PreviewTime { get; }

        /// <summary>
        ///     The audio file of this beatmap.
        /// </summary>
        string AudioFile { get; }

        /// <summary>
        ///     The background file of this beatmap.
        /// </summary>
        string BackgroundFile { get; }
    }
}
