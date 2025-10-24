using System;
using System.Collections.Generic;
using System.Linq;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Beatmaps.ControlPoints;
using LAsOsuBeatmapParser.Exceptions;

namespace LAsOsuBeatmapParser.Extensions;

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

        if (beatmap.Mode.Id != GameMode.Mania.Id)
        {
            throw new InvalidModeException("Beatmap mode must be Mania.");
        }

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

        var firstTime = beatmap.HitObjects.Min(h => h.StartTime);
        var lastTime = beatmap.HitObjects.Max(h => h.EndTime);

        return lastTime - firstTime;
    }
}
