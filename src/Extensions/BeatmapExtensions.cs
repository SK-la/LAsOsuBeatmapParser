using System;
using System.Collections.Generic;
using System.Linq;
using LAsOsuBeatmapParser.Analysis;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Beatmaps.ControlPoints;
using LAsOsuBeatmapParser.Exceptions;

namespace LAsOsuBeatmapParser.Extensions
{
    /// <summary>
    /// Beatmap 的扩展方法。
    /// </summary>
    public static class BeatmapExtensions
    {
        /// <summary>
        /// 从 Beatmap 获取 ManiaBeatmap，并验证模式。
        /// </summary>
        /// <typeparam name="T">HitObject类型。</typeparam>
        /// <param name="beatmap">谱面对象。</param>
        /// <returns>ManiaBeatmap 对象。</returns>
        /// <exception cref="InvalidModeException">如果模式不是 Mania 则抛出异常。</exception>
        public static ManiaBeatmap GetManiaBeatmap<T>(this Beatmap<T> beatmap) where T : HitObject
        {
            // 如果已经是ManiaBeatmap，直接返回
            if (typeof(T) == typeof(ManiaHitObject) && beatmap is ManiaBeatmap maniaBeatmap)
            {
                maniaBeatmap.ValidateMode();
                return maniaBeatmap;
            }

            if (beatmap.Mode.Id != GameMode.Mania.Id) throw new InvalidModeException("Beatmap mode must be Mania.");

            var mania = new ManiaBeatmap
            {
                BeatmapInfo = beatmap.BeatmapInfo,
                ControlPointInfo = beatmap.ControlPointInfo,
                Breaks = beatmap.Breaks,
                MetadataLegacy = beatmap.MetadataLegacy,
                DifficultyLegacy = beatmap.DifficultyLegacy,
                TimingPoints = beatmap.TimingPoints,
                HitObjects = beatmap.HitObjects.Select(h => new ManiaHitObject(h.StartTime, 0, (int)beatmap.Difficulty.CircleSize)).ToList(), // Convert HitObject to ManiaHitObject
                Events = beatmap.Events,
                Mode = beatmap.Mode,
                Version = beatmap.Version,
                TotalColumns = (int)beatmap.Difficulty.CircleSize
            };

            return mania;
        }

        /// <summary>
        /// 获取谱面的 BPM。
        /// </summary>
        /// <param name="beatmap">谱面对象。</param>
        /// <returns>BPM 数值。</returns>
        public static double GetBPM(this Beatmap beatmap)
        {
            return beatmap.BPM;
        }

        /// <summary>
        /// 获取用于分析的时间-音符矩阵。
        /// </summary>
        /// <param name="beatmap">谱面对象。</param>
        /// <returns>时间到音符列表的字典。</returns>
        public static Dictionary<double, List<HitObject>> GetMatrix(this Beatmap beatmap)
        {
            return beatmap.Matrix;
        }

        /// <summary>
        /// 根据条件过滤音符。
        /// </summary>
        /// <param name="beatmap">谱面对象。</param>
        /// <param name="predicate">过滤条件。</param>
        /// <returns>符合条件的音符集合。</returns>
        public static IEnumerable<HitObject> FilterHitObjects(this Beatmap beatmap, Func<HitObject, bool> predicate)
        {
            return beatmap.HitObjects.Where(predicate);
        }

        /// <summary>
        /// 获取最大连击数。
        /// </summary>
        /// <param name="beatmap">谱面对象。</param>
        /// <returns>最大连击数。</returns>
        public static int GetMaxCombo(this Beatmap beatmap)
        {
            return beatmap.HitObjects.Count; // 简化处理
        }

        /// <summary>
        /// 获取可玩时长。
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
        /// 计算谱面分析数据
        /// </summary>
        /// <param name="beatmap">谱面对象</param>
        /// <param name="calculateSR">是否计算SR（创建SRsNote数组），默认为false</param>
        /// <returns>计算后的分析数据</returns>
        public static BeatmapAnalysisData CalculateAnalysisData(this Beatmap beatmap, bool calculateSR = false)
        {
            // 直接转换为泛型调用
            return ((Beatmap<HitObject>)beatmap).CalculateAnalysisData(calculateSR);
        }

        /// <summary>
        /// 计算谱面分析数据
        /// </summary>
        /// <typeparam name="T">HitObject类型</typeparam>
        /// <param name="beatmap">谱面对象</param>
        /// <param name="calculateSR">是否计算SR（创建SRsNote数组），默认为false</param>
        /// <returns>计算后的分析数据</returns>
        public static BeatmapAnalysisData CalculateAnalysisData<T>(this Beatmap<T> beatmap, bool calculateSR = false) where T : HitObject
        {
            BeatmapAnalysisData analysisData = beatmap.AnalysisData;
            analysisData.SetParentBeatmap(beatmap); // 设置父谱面引用用于延迟计算
            List<T> hitObjects = beatmap.HitObjects;
            int cs = (int)beatmap.Difficulty.CircleSize;

            if (hitObjects.Count == 0)
            {
                analysisData.IsPrecomputed = true;
                return analysisData;
            }

            // 基础统计
            analysisData.TotalNotes = hitObjects.Count;
            analysisData.LongNotes = hitObjects.Count(ho => ho.EndTime > ho.StartTime);

            // 时长计算
            double minTime = hitObjects.Min(ho => ho.StartTime);
            double maxTime = hitObjects.Max(ho => Math.Max(ho.StartTime, ho.EndTime));
            analysisData.TotalDuration = maxTime - minTime;

            // KPS计算 - 基于BPM的优化版本 O(m + n log n)
            if (analysisData.TotalDuration > 0)
            {
                analysisData.AverageKPS = analysisData.TotalNotes / (analysisData.TotalDuration / 1000.0);

                // 基于BPM和timing points计算最大KPS
                // 每4/4拍（一个完整小节）采样一次KPS
                double maxKps = 0;

                // 按时间排序的HitObjects，用于二分查找
                List<T> sortedHitObjects = hitObjects.OrderBy(ho => ho.StartTime).ToList();

                // 按时间排序的TimingPoints
                List<TimingPoint> sortedTimingPoints = beatmap.TimingPoints.OrderBy(tp => tp.Time).ToList();

                if (sortedTimingPoints.Count == 0)
                {
                    // 如果没有timing points，使用默认BPM
                    sortedTimingPoints.Add(new TimingPoint { Time = 0, BeatLength = 60000.0 / 120.0 }); // 默认120 BPM
                }

                // 为每个timing point段计算KPS采样点
                for (int i = 0; i < sortedTimingPoints.Count; i++)
                {
                    TimingPoint currentTp = sortedTimingPoints[i];
                    double segmentStart = currentTp.Time;
                    double segmentEnd = i < sortedTimingPoints.Count - 1 ? sortedTimingPoints[i + 1].Time : maxTime;

                    // 计算BPM和节拍间隔
                    double beatInterval = currentTp.BeatLength; // 一拍的时间（毫秒）
                    double measureInterval = beatInterval * 4; // 4/4拍的时间（毫秒）

                    // 在这个timing point段内，每隔一个完整小节采样一次
                    for (double sampleTime = segmentStart; sampleTime < segmentEnd; sampleTime += measureInterval)
                    {
                        // 计算该采样点前后measureInterval/2范围内的音符数量
                        double windowStart = sampleTime - measureInterval / 2;
                        double windowEnd = sampleTime + measureInterval / 2;

                        // 使用二分查找找到窗口内的音符
                        int left = sortedHitObjects.FindIndex(ho => ho.StartTime >= windowStart);
                        if (left == -1) left = sortedHitObjects.Count;

                        int right = sortedHitObjects.FindLastIndex(ho => ho.StartTime <= windowEnd);
                        if (right == -1) right = -1;

                        int notesInWindow = Math.Max(0, right - left + 1);
                        maxKps = Math.Max(maxKps, notesInWindow);
                    }
                }

                analysisData.MaxKPS = maxKps;
            }

            // SR相关预计算（只在需要计算SR时进行）
            Console.WriteLine($"检查calculateSR条件: {calculateSR}");

            if (calculateSR)
            {
                Console.WriteLine("进入SR计算分支");
                Console.WriteLine($"SR计算分支执行，hitObjects.Count: {hitObjects.Count}");
                // 从HitObjects创建SRsNotes
                analysisData.SRsNotes = new SRsNote[hitObjects.Count];

                for (int i = 0; i < hitObjects.Count; i++)
                {
                    T hitObject = hitObjects[i];
                    int col = hitObject is ManiaHitObject maniaHit ? maniaHit.Column : (int)Math.Floor(hitObject.Position.X * cs / 512.0);
                    int time = (int)hitObject.StartTime;
                    int tail = hitObject.EndTime > hitObject.StartTime ? (int)hitObject.EndTime : -1;
                    analysisData.SRsNotes[i] = new SRsNote(col, time, tail);
                }

                // 键数分布
                if (cs > 0)
                {
                    analysisData.KeyDistribution = new int[cs];

                    foreach (SRsNote note in analysisData.SRsNotes)
                    {
                        if (note.Index >= 0 && note.Index < cs)
                            analysisData.KeyDistribution[note.Index]++;
                    }
                }

                // 直接计算SR值，避免延迟计算问题
                analysisData.StarRating = SRCalculator.Instance.CalculateSR(beatmap, out _);
            }

            analysisData.IsPrecomputed = true;
            return analysisData;
        }
    }
}
