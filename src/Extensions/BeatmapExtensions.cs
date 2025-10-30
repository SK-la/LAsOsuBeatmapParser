using System;
using System.Collections.Generic;
using System.Linq;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Exceptions;

namespace LAsOsuBeatmapParser.Extensions
{
    /// <summary>
    ///     Beatmap 的扩展方法。
    /// </summary>
    public static class BeatmapExtensions
    {
        /// <summary>
        ///     从 Beatmap 获取 ManiaBeatmap，并验证模式。
        /// </summary>
        /// <typeparam name="T">HitObject类型。</typeparam>
        /// <param name="beatmap">谱面对象。</param>
        /// <returns>Beatmap&lt;ManiaHitObject&gt; 对象。</returns>
        /// <exception cref="InvalidModeException">如果模式不是 Mania 则抛出异常。</exception>
        public static Beatmap<ManiaHitObject> GetManiaBeatmap<T>(this Beatmap<T> beatmap) where T : HitObject
        {
            // 如果已经是ManiaBeatmap，直接返回
            if (typeof(T) == typeof(ManiaHitObject)) return (Beatmap<ManiaHitObject>)(object)beatmap;

            if (beatmap.Mode.Id != GameMode.Mania.Id) throw new InvalidModeException("Beatmap mode must be Mania.");

            // Set current key count for ManiaHitObject position calculations
            int keyCount = (int)beatmap.Difficulty.CircleSize;

            var mania = new Beatmap<ManiaHitObject>
            {
                BeatmapInfo = beatmap.BeatmapInfo,
                ControlPointInfo = beatmap.ControlPointInfo,
                Breaks = beatmap.Breaks,
                MetadataLegacy = beatmap.MetadataLegacy,
                DifficultyLegacy = beatmap.DifficultyLegacy,
                Mode = beatmap.Mode,
                Version = beatmap.Version,
                TimingPoints = beatmap.TimingPoints,
                Events = beatmap.Events,

                HitObjects = beatmap.HitObjects.Select(h => new ManiaHitObject(h.StartTime, 0, keyCount)).ToList() // Convert HitObject to ManiaHitObject
            };

            return mania;
        }

        /// <summary>
        ///     获取谱面的 BPM。
        /// </summary>
        /// <param name="beatmap">谱面对象。</param>
        /// <returns>BPM 数值。</returns>
        public static double GetBPM(this Beatmap beatmap)
        {
            return beatmap.BPM;
        }

        /// <summary>
        ///     获取用于分析的时间-音符矩阵。
        /// </summary>
        /// <param name="beatmap">谱面对象。</param>
        /// <returns>时间到音符列表的字典。</returns>
        public static Dictionary<double, List<HitObject>> BuildMatrix(this Beatmap beatmap)
        {
            var matrix = new Dictionary<double, List<HitObject>>();

            foreach (HitObject hitObject in beatmap.HitObjects)
            {
                if (!matrix.ContainsKey(hitObject.StartTime)) matrix[hitObject.StartTime] = new List<HitObject>();
                matrix[hitObject.StartTime].Add(hitObject);
            }

            return matrix;
        }

        /// <summary>
        ///     获取用于分析的时间-音符矩阵（按需构建）。
        /// </summary>
        /// <param name="beatmap">谱面对象。</param>
        /// <returns>时间到音符列表的字典。</returns>
        public static Dictionary<double, List<HitObject>> GetMatrix(this Beatmap beatmap)
        {
            return beatmap.BuildMatrix();
        }

        /// <summary>
        ///     根据条件过滤音符。
        /// </summary>
        /// <param name="beatmap">谱面对象。</param>
        /// <param name="predicate">过滤条件。</param>
        /// <returns>符合条件的音符集合。</returns>
        public static IEnumerable<HitObject> FilterHitObjects(this Beatmap beatmap, Func<HitObject, bool> predicate)
        {
            return beatmap.HitObjects.Where(predicate);
        }

        /// <summary>
        ///     获取最大连击数。
        /// </summary>
        /// <param name="beatmap">谱面对象。</param>
        /// <returns>最大连击数。</returns>
        public static int GetMaxCombo(this Beatmap beatmap)
        {
            return beatmap.HitObjects.Count; // 简化处理
        }

        /// <summary>
        ///     获取可玩时长。
        /// </summary>
        /// <param name="beatmap">谱面对象。</param>
        /// <returns>时长（毫秒）。</returns>
        public static double GetPlayableLength(this Beatmap beatmap)
        {
            if (!beatmap.HitObjects.Any())
                return 0;

            double firstTime = beatmap.HitObjects.Min(h => h.StartTime);
            double lastTime = beatmap.HitObjects.Max(h => h.EndTime);

            return lastTime - firstTime;
        }

        /// <summary>
        ///     按节拍网格构建时间矩阵，用于分段处理 HitObject。
        /// </summary>
        /// <param name="beatmap">谱面对象。</param>
        /// <param name="beatsPerMeasure">每小节拍数（默认4，即4/4拍）。</param>
        /// <param name="subdivisions">每拍细分数（节拍细度，默认4，即每拍4个网格）。</param>
        /// <returns>网格开始时间到 HitObject 列表的字典。</returns>
        public static Dictionary<double, List<HitObject>> BuildBeatGridMatrix(this Beatmap beatmap, int beatsPerMeasure = 4, int subdivisions = 4)
        {
            var gridMatrix = new Dictionary<double, List<HitObject>>();
            var sortedTimingPoints = beatmap.TimingPoints.OrderBy(tp => tp.Time).ToList();

            if (!sortedTimingPoints.Any())
                return gridMatrix; // 无 TimingPoint，返回空

            // 获取所有 HitObject，按 StartTime 排序
            var sortedHitObjects = beatmap.HitObjects.OrderBy(h => h.StartTime).ToList();

            // 处理每个 BPM 区间
            for (int i = 0; i < sortedTimingPoints.Count; i++)
            {
                var currentTp = sortedTimingPoints[i];
                double intervalStart = currentTp.Time;
                double intervalEnd = (i < sortedTimingPoints.Count - 1) ? sortedTimingPoints[i + 1].Time : double.MaxValue;

                // 计算 BPM（仅非继承的 TimingPoint）
                if (currentTp.Inherited) continue; // 跳过继承 TimingPoint（它们不改变 BPM）
                double bpm = 60000.0 / currentTp.BeatLength;
                double beatLengthMs = currentTp.BeatLength; // 每拍毫秒
                double gridSizeMs = beatLengthMs / subdivisions; // 每个网格的毫秒

                // 对于第一个 TimingPoint，如果 Time > 0，向前回滚到接近 0
                if (i == 0 && intervalStart > 0)
                {
                    double rollbackStart = 0;

                    // 计算从 0 到 intervalStart 的网格
                    for (double t = rollbackStart; t < intervalStart; t += gridSizeMs)
                    {
                        gridMatrix[t] = new List<HitObject>();
                    }

                    // 余数区间：intervalStart 到下一个网格
                    double nextGrid = Math.Ceiling(intervalStart / gridSizeMs) * gridSizeMs;

                    if (nextGrid > intervalStart)
                    {
                        // TODO: 处理余数区间
                    }
                }

                // 计算区间内的网格
                for (double t = intervalStart; t < intervalEnd; t += gridSizeMs)
                {
                    gridMatrix[t] = new List<HitObject>();
                }

                // 处理余数：如果区间末尾不能整除网格
                double lastGridEnd = Math.Floor(intervalEnd / gridSizeMs) * gridSizeMs;

                if (lastGridEnd < intervalEnd)
                {
                    // TODO: 处理余数区间
                }
            }

            // 将 HitObject 分配到网格
            foreach (var hitObject in sortedHitObjects)
            {
                // 找到 hitObject.StartTime 所属的网格
                var possibleGrids = gridMatrix.Keys.Where(t => t <= hitObject.StartTime).OrderByDescending(t => t);
                double gridStart = possibleGrids.FirstOrDefault();

                if (gridStart != default)
                {
                    gridMatrix[gridStart].Add(hitObject);
                }
            }

            return gridMatrix;
        }
    }
}
