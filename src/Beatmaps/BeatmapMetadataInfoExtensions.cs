namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    ///     Extension methods for <see cref="IBeatmapMetadataInfo" />.
    /// </summary>
    public static class BeatmapMetadataInfoExtensions
    {
        /// <summary>
        ///     Gets the romanised title.
        /// </summary>
        /// <param name="metadata">The metadata.</param>
        /// <returns>The romanised title.</returns>
        public static string GetTitle(this IBeatmapMetadataInfo metadata)
        {
            return metadata.Title;
        }

        /// <summary>
        ///     Gets the unicode title.
        /// </summary>
        /// <param name="metadata">The metadata.</param>
        /// <returns>The unicode title.</returns>
        public static string GetTitleUnicode(this IBeatmapMetadataInfo metadata)
        {
            return metadata.TitleUnicode;
        }

        /// <summary>
        ///     Gets the romanised artist.
        /// </summary>
        /// <param name="metadata">The metadata.</param>
        /// <returns>The romanised artist.</returns>
        public static string GetArtist(this IBeatmapMetadataInfo metadata)
        {
            return metadata.Artist;
        }

        /// <summary>
        ///     Gets the unicode artist.
        /// </summary>
        /// <param name="metadata">The metadata.</param>
        /// <returns>The unicode artist.</returns>
        public static string GetArtistUnicode(this IBeatmapMetadataInfo metadata)
        {
            return metadata.ArtistUnicode;
        }

        /// <summary>
        ///     Gets the author.
        /// </summary>
        /// <param name="metadata">The metadata.</param>
        /// <returns>The author.</returns>
        public static IUser GetAuthor(this IBeatmapMetadataInfo metadata)
        {
            return metadata.Author;
        }

        /// <summary>
        ///     Gets the source.
        /// </summary>
        /// <param name="metadata">The metadata.</param>
        /// <returns>The source.</returns>
        public static string GetSource(this IBeatmapMetadataInfo metadata)
        {
            return metadata.Source;
        }

        /// <summary>
        ///     Gets the tags.
        /// </summary>
        /// <param name="metadata">The metadata.</param>
        /// <returns>The tags.</returns>
        public static string GetTags(this IBeatmapMetadataInfo metadata)
        {
            return metadata.Tags;
        }

        /// <summary>
        ///     Gets the preview time.
        /// </summary>
        /// <param name="metadata">The metadata.</param>
        /// <returns>The preview time.</returns>
        public static int GetPreviewTime(this IBeatmapMetadataInfo metadata)
        {
            return metadata.PreviewTime;
        }

        /// <summary>
        ///     Gets the audio file.
        /// </summary>
        /// <param name="metadata">The metadata.</param>
        /// <returns>The audio file.</returns>
        public static string GetAudioFile(this IBeatmapMetadataInfo metadata)
        {
            return metadata.AudioFile;
        }

        /// <summary>
        ///     Gets the background file.
        /// </summary>
        /// <param name="metadata">The metadata.</param>
        /// <returns>The background file.</returns>
        public static string GetBackgroundFile(this IBeatmapMetadataInfo metadata)
        {
            return metadata.BackgroundFile;
        }

        /// <summary>
        ///     Gets the display title.
        /// </summary>
        /// <param name="metadata">The metadata.</param>
        /// <returns>The display title.</returns>
        public static string GetDisplayTitle(this IBeatmapMetadataInfo metadata)
        {
            return $"{metadata.Artist} - {metadata.Title}";
        }

        /// <summary>
        ///     Gets the display title romanised.
        /// </summary>
        /// <param name="metadata">The metadata.</param>
        /// <returns>The display title romanised.</returns>
        public static string GetDisplayTitleRomanised(this IBeatmapMetadataInfo metadata)
        {
            return $"{metadata.ArtistUnicode} - {metadata.TitleUnicode}";
        }
    }
}
