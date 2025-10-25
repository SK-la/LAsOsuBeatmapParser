using System.Collections.Generic;
using System.Linq;

namespace LAsOsuBeatmapParser.Beatmaps.Timing
{
    /// <summary>
    ///     Utility class for timing calculations.
    /// </summary>
    public static class TimingCalculator
    {
        /// <summary>
        ///     Calculates the BPM at a given time from timing points.
        /// </summary>
        /// <param name="timingPoints">The timing points.</param>
        /// <param name="time">The time.</param>
        /// <returns>The BPM.</returns>
        public static double CalculateBPM(IEnumerable<TimingPoint> timingPoints, double time)
        {
            TimingPoint? tp = timingPoints.LastOrDefault(t => t.Time <= time);
            if (tp == null) return 120.0; // Default BPM
            return 60000.0 / tp.BeatLength;
        }

        /// <summary>
        ///     Gets the timing point at a given time.
        /// </summary>
        /// <param name="timingPoints">The timing points.</param>
        /// <param name="time">The time.</param>
        /// <returns>The timing point.</returns>
        public static TimingPoint? GetTimingPointAt(IEnumerable<TimingPoint> timingPoints, double time)
        {
            return timingPoints.LastOrDefault(t => t.Time <= time);
        }
    }
}
