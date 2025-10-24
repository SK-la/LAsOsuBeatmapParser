using System.Collections.Generic;
using System.Linq;

namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    /// 所有HitObject的基类。
    /// </summary>
    public abstract class HitObject
    {
        /// <summary>
        /// 起始时间（毫秒）。
        /// </summary>
        public double StartTime { get; set; }

        /// <summary>
        /// 结束时间（毫秒）。
        /// </summary>
        public virtual double EndTime
        {
            get => StartTime;
            set => StartTime = value;
        }

        /// <summary>
        /// 坐标（x, y）。
        /// </summary>
        public (float X, float Y) Position { get; set; }

        /// <summary>
        /// HitObject类型。
        /// </summary>
        public HitObjectType Type { get; set; }

        /// <summary>
        /// 击打音效。
        /// </summary>
        public int Hitsound { get; set; }

        /// <summary>
        /// 对象参数。
        /// </summary>
        public string ObjectParams { get; set; } = string.Empty;

        /// <summary>
        /// 击打采样。
        /// </summary>
        public string HitSamples { get; set; } = string.Empty;
    }

    /// <summary>
    /// 表示一个HitCircle。
    /// </summary>
    public partial class Note : HitObject
    {
        /// <summary>
        ///
        /// </summary>
        public Note()
        {
            Type = HitObjectType.Note;
        }

        /// <summary>
        /// Returns a string representation of this hit circle in osu format.
        /// </summary>
        /// <returns>The string representation.</returns>
        public override string ToString()
        {
            return $"{(int)Position.X},{(int)Position.Y},{(int)StartTime},{(int)Type},{Hitsound},{HitSamples}";
        }
    }

    /// <summary>
    /// 表示一个Slider。
    /// </summary>
    public class Slider : HitObject
    {
        /// <summary>
        /// Slider的结束时间。
        /// </summary>
        public override double EndTime
        {
            get => StartTime + Duration;
        }

        /// <summary>
        /// Slider的持续时间。
        /// </summary>
        public double Duration { get; set; }

        /// <summary>
        /// 曲线点。
        /// </summary>
        public List<(float X, float Y)> CurvePoints { get; set; } = new List<(float X, float Y)>();

        /// <summary>
        /// 重复次数。
        /// </summary>
        public int Slides { get; set; } = 1;

        /// <summary>
        /// 长度。
        /// </summary>
        public double Length { get; set; }

        /// <summary>
        ///
        /// </summary>
        public Slider()
        {
            Type = HitObjectType.Slider;
        }

        /// <summary>
        /// Returns a string representation of this slider in osu format.
        /// </summary>
        /// <returns>The string representation.</returns>
        public override string ToString()
        {
            // Format: x,y,time,type,hitSound,curveType|curvePoints,slides,length,edgeSounds,edgeSets,hitSample
            var curvePoints = new List<string> { "B" }; // Default to Bezier curve
            curvePoints.AddRange(CurvePoints.Select(p => $"{(int)p.X}:{(int)p.Y}"));
            string curveString = string.Join("|", curvePoints);

            // Default edge sounds and edge sets (empty)
            string edgeSounds = "";
            string edgeSets = "";

            return $"{(int)Position.X},{(int)Position.Y},{(int)StartTime},{(int)Type},{Hitsound},{curveString},{Slides},{Length:0.##},{edgeSounds},{edgeSets},{HitSamples}";
        }
    }

    /// <summary>
    /// 表示一个Spinner。
    /// </summary>
    public class Spinner : HitObject
    {
        /// <summary>
        ///
        /// </summary>
        public Spinner()
        {
            Type = HitObjectType.Spinner;
        }

        /// <summary>
        /// Spinner的结束时间。/ Spinner end time.
        /// </summary>
        public override double EndTime { get; set; }

        /// <summary>
        /// Spinner的结束时间值。/ Spinner end time value.
        /// </summary>
        public double EndTimeValue { get; set; }

        /// <summary>
        /// Returns a string representation of this spinner in osu format.
        /// </summary>
        /// <returns>The string representation.</returns>
        public override string ToString()
        {
            return $"{(int)Position.X},{(int)Position.Y},{(int)StartTime},{(int)Type},{Hitsound},{(int)EndTime},{HitSamples}";
        }
    }
}
