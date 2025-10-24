using System;
using System.Collections.Generic;
using System.Linq;

namespace LAsOsuBeatmapParser.Beatmaps;

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

/// <summary>
/// 游戏模式接口，允许自定义游戏模式的扩展。
/// </summary>
public interface IGameMode
{
    /// <summary>
    /// 游戏模式的唯一标识符。
    /// </summary>
    int Id { get; }

    /// <summary>
    /// 游戏模式的名称。
    /// </summary>
    string Name { get; }
}

/// <summary>
/// 默认游戏模式实现，包含osu!标准游戏模式。
/// </summary>
public class GameMode : IGameMode, IEquatable<GameMode>
{
    /// <summary>
    /// STD模式
    /// </summary>
    public static readonly GameMode Standard = new(0, "Standard");

    /// <summary>
    /// Taiko模式
    /// </summary>
    public static readonly GameMode Taiko = new(1, "Taiko");

    /// <summary>
    /// Catch模式
    /// </summary>
    public static readonly GameMode Catch = new(2, "Catch");

    /// <summary>
    /// Mania模式
    /// </summary>
    public static readonly GameMode Mania = new(3, "Mania");

    /// <summary>
    /// 获取所有预定义的游戏模式。
    /// </summary>
    public static IEnumerable<GameMode> AllModes => new[] { Standard, Taiko, Catch, Mania };

    /// <summary>
    /// 根据ID获取游戏模式，只返回官方预定义的游戏模式（ID 0-3）。
    /// 对于自定义游戏模式，请直接创建新的IGameMode实现。
    /// </summary>
    /// <param name="id">游戏模式ID</param>
    /// <returns>对应的官方游戏模式或null</returns>
    public static GameMode? FromId(int id)
    {
        return id switch
        {
            0 => Standard,
            1 => Taiko,
            2 => Catch,
            3 => Mania,
            _ => null // 自定义模式ID应大于3
        };
    }

    internal GameMode(int id, string name)
    {
        Id = id;
        Name = name;
    }

    /// <inheritdoc />
    public int Id { get; }

    /// <inheritdoc />
    public string Name { get; }

    /// <inheritdoc />
    public override string ToString() => Name;

    /// <inheritdoc />
    public override bool Equals(object? obj) => Equals(obj as GameMode);

    /// <inheritdoc />
    public bool Equals(GameMode? other) => other is not null && Id == other.Id;

    /// <inheritdoc />
    public override int GetHashCode() => Id.GetHashCode();
}

/// <summary>
/// 自定义游戏模式实现，用于处理未知的游戏模式ID。
/// </summary>
public class CustomGameMode : IGameMode, IEquatable<CustomGameMode>
{
    public CustomGameMode(int id, string name)
    {
        Id = id;
        Name = name;
    }

    public int Id { get; }
    public string Name { get; }

    public override string ToString() => Name;
    public override bool Equals(object? obj) => Equals(obj as CustomGameMode);
    public bool Equals(CustomGameMode? other) => other is not null && Id == other.Id;
    public override int GetHashCode() => Id.GetHashCode();
}
