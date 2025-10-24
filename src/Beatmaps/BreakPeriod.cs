using System;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// Represents a break period in a beatmap.
/// </summary>
public class BreakPeriod : IComparable<BreakPeriod>, IEquatable<BreakPeriod>
{
    /// <summary>
    /// The start time of the break.
    /// </summary>
    public double StartTime { get; set; }

    /// <summary>
    /// The end time of the break.
    /// </summary>
    public double EndTime { get; set; }

    /// <summary>
    /// The duration of the break.
    /// </summary>
    public double Duration => EndTime - StartTime;

    /// <summary>
    /// Constructs a new break period.
    /// </summary>
    /// <param name="startTime">The start time of the break period.</param>
    /// <param name="endTime">The end time of the break period.</param>
    public BreakPeriod(double startTime, double endTime)
    {
        StartTime = startTime;
        EndTime = endTime;
    }

    /// <summary>
    /// Compares this break period to another.
    /// </summary>
    /// <param name="other">The other break period.</param>
    /// <returns>The comparison result.</returns>
    public int CompareTo(BreakPeriod? other)
    {
        if (other == null) return 1;
        var result = StartTime.CompareTo(other.StartTime);
        return result != 0 ? result : EndTime.CompareTo(other.EndTime);
    }

    /// <summary>
    /// Determines whether this break period is equal to another.
    /// </summary>
    /// <param name="other">The other break period.</param>
    /// <returns>True if equal, false otherwise.</returns>
    public bool Equals(BreakPeriod? other) =>
        other != null && StartTime == other.StartTime && EndTime == other.EndTime;

    /// <summary>
    /// Returns the hash code for this break period.
    /// </summary>
    /// <returns>The hash code.</returns>
    public override int GetHashCode() => HashCode.Combine(StartTime, EndTime);

    /// <summary>
    /// Returns a string representation of this break period.
    /// </summary>
    /// <returns>The string representation.</returns>
    public override string ToString() => $"Break: {StartTime} - {EndTime}";
}
