using System;
using System.Collections.Generic;
using LAsOsuBeatmapParser.Beatmaps;

namespace LAsOsuBeatmapParser.Extensions
{
    /// <summary>
    ///     基于节拍网格的 Beatmap 转换类。
    ///     使用指定的 IApplyToHitObject 转换器对网格中的 HitObject 应用转换。
    /// </summary>
    public class ApplyBeatGridTransformation : ApplyBeatmapBase
    {
        /// <summary>
        ///     每小节拍数（默认4，即4/4拍）。
        /// </summary>
        public int BeatsPerMeasure { get; set; } = 4;

        /// <summary>
        ///     每拍细分数（节拍细度，默认4，即每拍4个网格）。
        /// </summary>
        public int Subdivisions { get; set; } = 4;

        /// <summary>
        ///     用于转换 HitObject 的转换器。
        /// </summary>
        public IApplyToHitObject Transformer { get; }

        /// <summary>
        ///     创建一个新的 ApplyBeatGridTransformation 实例。
        /// </summary>
        /// <param name="transformer">用于转换 HitObject 的 IApplyToHitObject 实现。</param>
        public ApplyBeatGridTransformation(IApplyToHitObject transformer)
        {
            Transformer = transformer ?? throw new ArgumentNullException(nameof(transformer));
        }

        /// <summary>
        ///     将节拍网格转换应用到指定的 Beatmap。
        ///     构建网格矩阵，然后对每段内的 HitObject 调用 Transformer.ApplyToHitObject。
        /// </summary>
        /// <param name="beatmap">要修改的 Beatmap 对象。</param>
        public override void ApplyToBeatmap(Beatmap beatmap)
        {
            base.ApplyToBeatmap(beatmap); // 调用基础功能

            var gridMatrix = beatmap.BuildBeatGridMatrix(BeatsPerMeasure, Subdivisions);

            foreach (var kvp in gridMatrix)
            {
                double gridStartTime = kvp.Key;
                List<HitObject> notesInSegment = kvp.Value;

                // 对每段内的 HitObject 应用转换
                foreach (var note in notesInSegment)
                {
                    Transformer.ApplyToHitObject(note);
                }
            }
        }
    }
}
