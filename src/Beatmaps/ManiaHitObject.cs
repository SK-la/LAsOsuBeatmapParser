using System;
using LAsOsuBeatmapParser.Objects.Types;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// Represents a hit object in Mania.
/// </summary>
public class ManiaHitObject : HitObject, IHasXPosition
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
    /// For Mania, Position.X represents the calculated X coordinate from column.
    /// Position.Y is always 192 (center of playfield).
    /// </summary>
    public new (float X, float Y) Position
    {
        get => (Column * (512f / KeyCount), 192f);
        set
        {
            // When setting position, convert back to column
            if (KeyCount > 0)
                Column = (int)Math.Round(value.X / (512f / KeyCount));
        }
    }

    /// <summary>
    /// Implements IHasXPosition.X - returns the column as X position.
    /// </summary>
    public float X
    {
        get => Column;
        set => Column = (int)value;
    }

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
        // Mania hit objects: x,y,time,type,hitSound,hitSample
        // x is position, y is 192 for standard position
        // Use official osu formula: x = ceil(column * (512 / keyCount))
        const int totalWidth = 512;
        int keyCount = KeyCount > 0 ? KeyCount : 4; // Default to 4 if not set
        float ratio = totalWidth / (float)keyCount;
        int x = (int)Math.Ceiling(Column * ratio);
        int y = 192; // Standard y position for mania
        int type = 1; // Normal hit

        // Ensure hit samples has a default value for mania
        string hitSamples = string.IsNullOrEmpty(HitSamples) ? "0:0:0:0:" : HitSamples;

        return $"{x},{y},{(int)StartTime},{type},{Hitsound},{hitSamples}";
    }
}
