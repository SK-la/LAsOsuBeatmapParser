using System.Numerics;
using LAsOsuBeatmapParser.Extensions;
using LAsOsuBeatmapParser.Objects.Types;

namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    ///     Represents a hit object in Mania.
    /// </summary>
    public class ManiaHitObject : HitObject, IHasXPosition
    {
        /// <summary>
        ///     Creates a new ManiaHitObject.
        /// </summary>
        public ManiaHitObject()
        {
        }

        /// <summary>
        ///     Creates a new ManiaHitObject with the specified time and column.
        /// </summary>
        /// <param name="startTime">The start time.</param>
        /// <param name="column">The column.</param>
        public ManiaHitObject(double startTime, int column)
        {
            StartTime = startTime;
            Column    = column;
        }

        /// <summary>
        ///     Creates a new ManiaHitObject with the specified time, column, and key count.
        /// </summary>
        /// <param name="startTime">The start time.</param>
        /// <param name="column">The column.</param>
        /// <param name="keyCount">The total number of keys.</param>
        public ManiaHitObject(double startTime, int column, int keyCount)
        {
            StartTime = startTime;
            Column    = column;
            KeyCount  = keyCount;
        }

        /// <summary>
        ///     The column this hit object is in (0-based).
        /// </summary>
        public int Column { get; set; }

        /// <summary>
        ///     The total number of keys (columns) in this mania beatmap.
        /// </summary>
        public int KeyCount { get; set; }

        /// <summary>
        ///     For Mania, Position.X represents the normalized X coordinate from column.
        ///     Position.Y is always 192 (center of playfield).
        /// </summary>
        public override Vector2 Position
        {
            get => new Vector2(ManiaExtensions.GetPositionX(KeyCount, Column), 192f);
            set => Column = ManiaExtensions.GetColumnFromX(KeyCount, value.X);
        }

        /// <summary>
        ///     Implements IHasXPosition.X - returns the X coordinate.
        /// </summary>
        public float X
        {
            get => Position.X;
            set => Position = new Vector2(value, Position.Y);
        }

        /// <summary>
        ///     Returns a string representation of this hit object. 使用算法规范化坐标
        /// </summary>
        /// <returns>The string representation.</returns>
        public override string ToString()
        {
            // Mania hit objects: x,y,time,type,hitSound,hitSample
            // x is position, y is 192 for standard position
            int x    = (int)Position.X;
            int y    = (int)Position.Y;
            int type = 1; // Normal hit

            // Ensure hit samples has a default value for mania
            string hitSamples = string.IsNullOrEmpty(HitSamples) ? "0:0:0:0:" : HitSamples;

            return $"{x},{y},{(int)StartTime},{type},{Hitsound},{hitSamples}";
        }
    }
}
