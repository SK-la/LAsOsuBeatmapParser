using System;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// 游戏模式。
/// </summary>
public enum GameMode
{
    /// <summary>
    /// STD模式
    /// </summary>
    Standard = 0,
    /// <summary>
    /// Taiko模式
    /// </summary>
    Taiko = 1,
    /// <summary>
    /// Catch模式
    /// </summary>
    Catch = 2,
    /// <summary>
    /// Mania模式
    /// </summary>
    Mania = 3
}

/// <summary>
/// HitObject类型。
/// </summary>
[Flags]
public enum HitObjectType
{
    /// <summary>
    /// 通用类型，表示没有任何类型。
    /// </summary>
    Note = 1,
    /// <summary>
    /// 非Mania专用类型，表示滑条。
    /// </summary>
    Slider = 2,
    /// <summary>
    /// 非Mania专用类型，表示新连打。
    /// </summary>
    NewCombo = 4,
    /// <summary>
    /// 非Mania专用类型，表示转盘/果盘/打鼓彩球
    /// </summary>
    Spinner = 8,
    /// <summary>
    /// Mania专用类型，表示强制单击。
    /// </summary>
    ManiaHold = 128
}

/// <summary>
/// Countdown type.
/// </summary>
public enum CountdownType
{
    /// <summary>
    /// No countdown.
    /// </summary>
    None,
    /// <summary>
    /// Normal countdown.
    /// </summary>
    Normal,
    /// <summary>
    /// Half speed countdown.
    /// </summary>
    Half,
    /// <summary>
    /// Double speed countdown.
    /// </summary>
    Double
}
