using System;

namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    ///     Represents a break period in a beatmap.
    /// </summary>
    public class BreakPeriod : IComparable<BreakPeriod>, IEquatable<BreakPeriod>
    {
        /// <summary>
        ///     Constructs a new break period.
        /// </summary>
        /// <param name="startTime">The start time of the break period.</param>
        /// <param name="endTime">The end time of the break period.</param>
        public BreakPeriod(double startTime, double endTime)
        {
            StartTime = startTime;
            EndTime   = endTime;
        }

        /// <summary>
        ///     The start time of the break.
        /// </summary>
        public double StartTime { get; init; }

        /// <summary>
        ///     The end time of the break.
        /// </summary>
        public double EndTime { get; init; }

        /// <summary>
        ///     The duration of the break.
        /// </summary>
        public double Duration
        {
            get => EndTime - StartTime;
        }

        /// <summary>
        ///     Compares this break period to another.
        /// </summary>
        /// <param name="other">The other break period.</param>
        /// <returns>The comparison result.</returns>
        public int CompareTo(BreakPeriod? other)
        {
            if (other == null) return 1;

            int result = StartTime.CompareTo(other.StartTime);
            return result != 0 ? result : EndTime.CompareTo(other.EndTime);
        }

        /// <summary>
        ///     Determines whether this break period is equal to another.
        /// </summary>
        /// <param name="other">The other break period.</param>
        /// <returns>True if equal, false otherwise.</returns>
        public bool Equals(BreakPeriod? other)
        {
            if (other is null) return false;

            const double epsilon = 1e-7; // Small tolerance for floating point comparison
            return Math.Abs(StartTime - other.StartTime) < epsilon &&
                   Math.Abs(EndTime - other.EndTime) < epsilon;
        }

        /// <summary>
        ///     Returns the hash code for this break period.
        /// </summary>
        /// <returns>The hash code.</returns>
        public override int GetHashCode()
        {
            const double epsilon = 1e-7;
            // Round to the nearest multiple of epsilon to ensure equal objects have equal hash codes
            long startRounded = (long)Math.Round(StartTime / epsilon);
            long endRounded   = (long)Math.Round(EndTime / epsilon);
            return HashCode.Combine(startRounded, endRounded);
        }

        /// <summary>
        ///     Returns a string representation of this break period.
        /// </summary>
        /// <returns>The string representation.</returns>
        public override string ToString()
        {
            return $"Break: {StartTime} - {EndTime}";
        }
    }
}
