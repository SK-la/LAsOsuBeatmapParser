namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    /// Extension methods for <see cref="IBeatmapInfo"/>.
    /// </summary>
    public static class BeatmapInfoExtensions
    {
        /// <summary>
        /// Gets the star rating for this beatmap.
        /// </summary>
        /// <param name="beatmapInfo">The beatmap info.</param>
        /// <returns>The star rating.</returns>
        public static double GetStarRating(this IBeatmapInfo beatmapInfo)
        {
            return beatmapInfo.StarRating;
        }

        /// <summary>
        /// Gets the BPM for this beatmap.
        /// </summary>
        /// <param name="beatmapInfo">The beatmap info.</param>
        /// <returns>The BPM.</returns>
        public static double GetBPM(this IBeatmapInfo beatmapInfo)
        {
            return beatmapInfo.BPM;
        }

        /// <summary>
        /// Gets the length for this beatmap.
        /// </summary>
        /// <param name="beatmapInfo">The beatmap info.</param>
        /// <returns>The length in milliseconds.</returns>
        public static double GetLength(this IBeatmapInfo beatmapInfo)
        {
            return beatmapInfo.Length;
        }

        /// <summary>
        /// Gets the hash for this beatmap.
        /// </summary>
        /// <param name="beatmapInfo">The beatmap info.</param>
        /// <returns>The hash.</returns>
        public static string GetHash(this IBeatmapInfo beatmapInfo)
        {
            return beatmapInfo.Hash;
        }

        /// <summary>
        /// Gets the MD5 hash for this beatmap.
        /// </summary>
        /// <param name="beatmapInfo">The beatmap info.</param>
        /// <returns>The MD5 hash.</returns>
        public static string GetMD5Hash(this IBeatmapInfo beatmapInfo)
        {
            return beatmapInfo.MD5Hash;
        }

        /// <summary>
        /// Gets the total object count for this beatmap.
        /// </summary>
        /// <param name="beatmapInfo">The beatmap info.</param>
        /// <returns>The total object count.</returns>
        public static int GetTotalObjectCount(this IBeatmapInfo beatmapInfo)
        {
            return beatmapInfo.TotalObjectCount;
        }

        /// <summary>
        /// Gets the end time object count for this beatmap.
        /// </summary>
        /// <param name="beatmapInfo">The beatmap info.</param>
        /// <returns>The end time object count.</returns>
        public static int GetEndTimeObjectCount(this IBeatmapInfo beatmapInfo)
        {
            return beatmapInfo.EndTimeObjectCount;
        }

        /// <summary>
        /// Checks if this beatmap is from the same set as another.
        /// </summary>
        /// <param name="beatmapInfo">The beatmap info.</param>
        /// <param name="other">The other beatmap info.</param>
        /// <returns>True if from the same set.</returns>
        public static bool IsFromSameBeatmapSet(this IBeatmapInfo beatmapInfo, IBeatmapInfo other)
        {
            return beatmapInfo.BeatmapSet?.Equals(other.BeatmapSet) == true;
        }

        /// <summary>
        /// Gets the display title for this beatmap.
        /// </summary>
        /// <param name="beatmapInfo">The beatmap info.</param>
        /// <returns>The display title.</returns>
        public static string GetDisplayTitle(this IBeatmapInfo beatmapInfo)
        {
            return $"{beatmapInfo.Metadata.Artist} - {beatmapInfo.Metadata.Title}";
        }

        /// <summary>
        /// Gets the display title romanised for this beatmap.
        /// </summary>
        /// <param name="beatmapInfo">The beatmap info.</param>
        /// <returns>The display title romanised.</returns>
        public static string GetDisplayTitleRomanised(this IBeatmapInfo beatmapInfo)
        {
            return $"{beatmapInfo.Metadata.ArtistUnicode} - {beatmapInfo.Metadata.TitleUnicode}";
        }
    }
}
