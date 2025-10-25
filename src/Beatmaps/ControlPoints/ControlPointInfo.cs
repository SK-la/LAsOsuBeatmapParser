using System.Collections.Generic;

namespace LAsOsuBeatmapParser.Beatmaps.ControlPoints
{
    /// <summary>
    ///     Contains all the control points in a beatmap.
    /// </summary>
    public class ControlPointInfo
    {
        private readonly List<DifficultyControlPoint> difficultyPoints = new List<DifficultyControlPoint>();

        private readonly List<EffectControlPoint> effectPoints = new List<EffectControlPoint>();

        private readonly List<SampleControlPoint> samplePoints = new List<SampleControlPoint>();

        private readonly List<TimingControlPoint> timingPoints = new List<TimingControlPoint>();
        /// <summary>
        ///     All timing points.
        /// </summary>
        public IReadOnlyList<TimingControlPoint> TimingPoints
        {
            get => timingPoints;
        }

        /// <summary>
        ///     All difficulty points.
        /// </summary>
        public IReadOnlyList<DifficultyControlPoint> DifficultyPoints
        {
            get => difficultyPoints;
        }

        /// <summary>
        ///     All effect points.
        /// </summary>
        public IReadOnlyList<EffectControlPoint> EffectPoints
        {
            get => effectPoints;
        }

        /// <summary>
        ///     All sample points.
        /// </summary>
        public IReadOnlyList<SampleControlPoint> SamplePoints
        {
            get => samplePoints;
        }

        /// <summary>
        ///     Adds a timing control point.
        /// </summary>
        /// <param name="point">The point to add.</param>
        public void Add(TimingControlPoint point)
        {
            timingPoints.Add(point);
        }

        /// <summary>
        ///     Adds a difficulty control point.
        /// </summary>
        /// <param name="point">The point to add.</param>
        public void Add(DifficultyControlPoint point)
        {
            difficultyPoints.Add(point);
        }

        /// <summary>
        ///     Adds an effect control point.
        /// </summary>
        /// <param name="point">The point to add.</param>
        public void Add(EffectControlPoint point)
        {
            effectPoints.Add(point);
        }

        /// <summary>
        ///     Adds a sample control point.
        /// </summary>
        /// <param name="point">The point to add.</param>
        public void Add(SampleControlPoint point)
        {
            samplePoints.Add(point);
        }
    }
}
