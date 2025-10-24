using System;
using System.Collections.Generic;
using LAsOsuBeatmapParser.Extensions;
using LAsOsuBeatmapParser.Beatmaps;

namespace LAsOsuBeatmapParser.Validators;

/// <summary>
/// 谱面验证器。
/// </summary>
public static class BeatmapValidator
{
    /// <summary>
    /// 验证一个谱面。
    /// </summary>
    /// <param name="beatmap">要验证的谱面。</param>
    /// <returns>验证错误列表。</returns>
    public static List<string> Validate(Beatmap beatmap)
    {
        var errors = new List<string>();

        // 验证模式 - 现在允许自定义模式，所以只检查基本有效性
        if (beatmap.Mode == null)
        {
            errors.Add("游戏模式不能为空");
        }
        else if (beatmap.Mode.Id < 0)
        {
            errors.Add($"游戏模式ID无效: {beatmap.Mode.Id}");
        }

        // 验证TimingPoint是否按时间排序
        for (int i = 1; i < beatmap.TimingPoints.Count; i++)
        {
            if (beatmap.TimingPoints[i].Time < beatmap.TimingPoints[i - 1].Time)
            {
                errors.Add("TimingPoint未按时间排序。");
                break;
            }
        }

        // 验证HitObject
        foreach (var hitObject in beatmap.HitObjects)
        {
            if (hitObject.StartTime < 0)
            {
                errors.Add($"HitObject起始时间为负数: {hitObject.StartTime}");
            }

            if (hitObject.EndTime < hitObject.StartTime)
            {
                errors.Add($"HitObject结束时间早于起始时间: {hitObject.StartTime} - {hitObject.EndTime}");
            }
        }

        // Mania模式专属验证
        if (beatmap.Mode.Id == GameMode.Mania.Id)
        {
            var maniaBeatmap = beatmap.GetManiaBeatmap();

            // 验证列数
            if (maniaBeatmap.TotalColumns <= 0 || maniaBeatmap.TotalColumns > 10)
            {
                errors.Add($"Mania谱面列数无效: {maniaBeatmap.TotalColumns} (应在1-10之间)");
            }

            // 验证ManiaHitObject的列索引
            foreach (var hitObject in maniaBeatmap.HitObjects)
            {
                if (hitObject.Column < 0 || hitObject.Column >= maniaBeatmap.TotalColumns)
                {
                    errors.Add($"{hitObject.GetType().Name}列索引超出范围: {hitObject.Column} (应在0-{maniaBeatmap.TotalColumns - 1}之间)");
                }
            }
        }

        return errors;
    }
}
