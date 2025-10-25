using System;

namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    /// 表示谱面的难度设置。
    /// </summary>
    public class BeatmapDifficulty : IBeatmapDifficultyInfo
    {
        /// <summary>
        /// HP减少速率。
        /// </summary>
        public float HPDrainRate { get; set; }

        /// <summary>
        /// 圆圈大小。对于Mania模式，此值为键数。
        /// </summary>
        public float CircleSize { get; set; }

        /// <summary>
        /// 总体难度。
        /// </summary>
        public float OverallDifficulty { get; set; }

        /// <summary>
        /// 预判条出现速率。
        /// </summary>
        public float ApproachRate { get; set; }

        /// <summary>
        /// 滑条倍率。
        /// </summary>
        public float SliderMultiplier { get; set; } = 1.4f;

        /// <summary>
        /// 滑条刻度。
        /// </summary>
        public float SliderTickRate { get; set; } = 1.0f;

        /// <summary>
        /// Explicit interface implementation.
        /// </summary>
        float IBeatmapDifficultyInfo.DrainRate
        {
            get => HPDrainRate;
        }

        /// <summary>
        /// Clones this difficulty.
        /// </summary>
        /// <returns>The cloned difficulty.</returns>
        public BeatmapDifficulty Clone()
        {
            return new BeatmapDifficulty
            {
                HPDrainRate = HPDrainRate,
                CircleSize = CircleSize,
                OverallDifficulty = OverallDifficulty,
                ApproachRate = ApproachRate,
                SliderMultiplier = SliderMultiplier,
                SliderTickRate = SliderTickRate
            };
        }

        /// <summary>
        /// Determines whether the specified object is equal to the current object.
        /// </summary>
        /// <param name="other">The object to compare with the current object.</param>
        /// <returns>true if the specified object is equal to the current object; otherwise, false.</returns>
        public bool Equals(IBeatmapDifficultyInfo? other)
        {
            if (other == null) return false;
            float TOLERANCE = 0.001f;
            return Math.Abs(HPDrainRate - other.DrainRate) < TOLERANCE &&
                   Math.Abs(CircleSize - other.CircleSize) < TOLERANCE &&
                   Math.Abs(OverallDifficulty - other.OverallDifficulty) < TOLERANCE &&
                   Math.Abs(ApproachRate - other.ApproachRate) < TOLERANCE &&
                   Math.Abs(SliderMultiplier - other.SliderMultiplier) < TOLERANCE &&
                   Math.Abs(SliderTickRate - other.SliderTickRate) < TOLERANCE;
        }
    }
}
