using System;

namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    ///     Represents a statistic for display in the beatmap.
    /// </summary>
    public class BeatmapStatistic
    {
        /// <summary>
        ///     A function to create the icon for display purposes.
        /// </summary>
        public Func<object>? CreateIcon; // Simplified, since no Drawable

        /// <summary>
        ///     The name of this statistic.
        /// </summary>
        public string Name { get; set; } = string.Empty;

        /// <summary>
        ///     The text representing the value of this statistic.
        /// </summary>
        public string Content { get; set; } = string.Empty;

        /// <summary>
        ///     The length of a bar which visually represents this statistic's relevance in the beatmap.
        /// </summary>
        public float? BarDisplayLength { get; set; }

        /// <summary>
        ///     Star Rating (SR) value.
        /// </summary>
        public double SR { get; set; } = -1;

        /// <summary>
        ///     Maximum Keys Per Second.
        /// </summary>
        public double MaxKPS { get; set; }

        /// <summary>
        ///     Average Keys Per Second.
        /// </summary>
        public double AverageKPS { get; set; }

        /// <summary>
        ///     Total number of notes.
        /// </summary>
        public int TotalNotes { get; set; }

        /// <summary>
        ///     Number of long notes.
        /// </summary>
        public int LongNotes { get; set; }

        /// <summary>
        ///     Total duration of the beatmap in milliseconds.
        /// </summary>
        public double TotalDuration { get; set; }
    }
}
