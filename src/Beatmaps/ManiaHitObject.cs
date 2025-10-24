using System;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// Represents a hit object in Mania.
/// </summary>
public class ManiaHitObject : HitObject
{
    /// <summary>
    /// The column this hit object is in (0-based).
    /// </summary>
    public int Column { get; set; }

    /// <summary>
    /// The total number of columns (key count) for this beatmap.
    /// </summary>
    public int KeyCount { get; set; }

    /// <summary>
    /// Creates a new ManiaHitObject.
    /// </summary>
    public ManiaHitObject()
    {
    }

    /// <summary>
    /// Creates a new ManiaHitObject with the specified time and column.
    /// </summary>
    /// <param name="startTime">The start time.</param>
    /// <param name="column">The column.</param>
    public ManiaHitObject(double startTime, int column)
    {
        StartTime = startTime;
        Column = column;
    }

    /// <summary>
    /// Creates a new ManiaHitObject with the specified time, column, and key count.
    /// </summary>
    /// <param name="startTime">The start time.</param>
    /// <param name="column">The column.</param>
    /// <param name="keyCount">The total number of columns.</param>
    public ManiaHitObject(double startTime, int column, int keyCount)
    {
        StartTime = startTime;
        Column = column;
        KeyCount = keyCount;
    }

    /// <summary>
    /// Returns a string representation of this hit object.
    /// </summary>
    /// <returns>The string representation.</returns>
    public override string ToString()
    {
        // Mania hit objects: x,y,time,type,hitSound,endTime:hitSample
        // x is position, y is 192 for standard position
        // Use official osu formula: x = ceil(column * (512 / keyCount))
        const int totalWidth = 512;
        int keyCount = KeyCount > 0 ? KeyCount : 4; // Default to 4 if not set
        float ratio = totalWidth / (float)keyCount;
        int x = (int)Math.Ceiling(Column * ratio);
        int y = 192; // Standard y position for mania
        int type = 1; // Normal hit
        int hitSound = 0;
        string hitSample = "0:0:0:0:";

        return $"{x},{y},{(int)StartTime},{type},{hitSound},{hitSample}";
    }
}
