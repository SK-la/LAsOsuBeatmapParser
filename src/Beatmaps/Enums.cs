using System;
using System.Collections.Generic;

namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    ///     HitObject类型。
    /// </summary>
    [Flags]
    public enum HitObjectType
    {
        /// <summary>
        ///     通用类型，表示没有任何类型。
        /// </summary>
        Note = 1,

        /// <summary>
        ///     非Mania专用类型，表示滑条。
        /// </summary>
        Slider = 2,

        /// <summary>
        ///     非Mania专用类型，表示新连打。
        /// </summary>
        NewCombo = 4,

        /// <summary>
        ///     非Mania专用类型，表示转盘/果盘/打鼓彩球
        /// </summary>
        Spinner = 8,

        /// <summary>
        ///     Mania专用类型，表示强制单击。
        /// </summary>
        ManiaHold = 128
    }

    /// <summary>
    ///     Countdown type.
    /// </summary>
    public enum CountdownType
    {
        /// <summary>
        ///     No countdown.
        /// </summary>
        None,

        /// <summary>
        ///     Normal countdown.
        /// </summary>
        Normal,

        /// <summary>
        ///     Half speed countdown.
        /// </summary>
        Half,

        /// <summary>
        ///     Double speed countdown.
        /// </summary>
        Double
    }

    /// <summary>
    ///     游戏模式接口，允许自定义游戏模式的扩展。
    /// </summary>
    public interface IGameMode
    {
        /// <summary>
        ///     游戏模式的唯一标识符。
        /// </summary>
        int Id { get; }

        /// <summary>
        ///     游戏模式的名称。
        /// </summary>
        string Name { get; }
    }

    /// <summary>
    ///     默认游戏模式实现，包含osu!标准游戏模式。
    /// </summary>
    public class GameMode : IGameMode, IEquatable<GameMode>
    {
        /// <summary>
        ///     STD模式
        /// </summary>
        public static readonly GameMode Standard = new GameMode(0, "Standard");

        /// <summary>
        ///     Taiko模式
        /// </summary>
        public static readonly GameMode Taiko = new GameMode(1, "Taiko");

        /// <summary>
        ///     Catch模式
        /// </summary>
        public static readonly GameMode Catch = new GameMode(2, "Catch");

        /// <summary>
        ///     Mania模式
        /// </summary>
        public static readonly GameMode Mania = new GameMode(3, "Mania");

        internal GameMode(int id, string name)
        {
            Id   = id;
            Name = name;
        }

        /// <summary>
        ///     获取所有预定义的游戏模式。
        /// </summary>
        public static IEnumerable<GameMode> AllModes
        {
            get => new[] { Standard, Taiko, Catch, Mania };
        }

        /// <inheritdoc />
        public bool Equals(GameMode? other)
        {
            return other is not null && Id == other.Id;
        }

        /// <inheritdoc />
        public int Id { get; }

        /// <inheritdoc />
        public string Name { get; }

        /// <summary>
        ///     根据ID获取游戏模式，只返回官方预定义的游戏模式（ID 0-3）。
        ///     对于自定义游戏模式，请直接创建新的IGameMode实现。
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

        /// <inheritdoc />
        public override string ToString()
        {
            return Name;
        }

        /// <inheritdoc />
        public override bool Equals(object? obj)
        {
            return Equals(obj as GameMode);
        }

        /// <inheritdoc />
        public override int GetHashCode()
        {
            return Id.GetHashCode();
        }
    }

    /// <summary>
    ///     自定义游戏模式实现，用于处理未知的游戏模式ID。
    ///     Custom game mode implementation for handling unknown game mode IDs.
    /// </summary>
    public class CustomGameMode : IGameMode, IEquatable<CustomGameMode>
    {
        /// <summary>
        ///     创建自定义游戏模式实例。
        ///     Creates a custom game mode instance.
        /// </summary>
        /// <param name="id">游戏模式ID / Game mode ID</param>
        /// <param name="name">游戏模式名称 / Game mode name</param>
        public CustomGameMode(int id, string name)
        {
            Id   = id;
            Name = name;
        }

        /// <summary>
        ///     确定指定的 CustomGameMode 是否等于当前 CustomGameMode。
        ///     Determines whether the specified CustomGameMode is equal to the current CustomGameMode.
        /// </summary>
        /// <param name="other">要比较的 CustomGameMode / The CustomGameMode to compare</param>
        /// <returns>如果对象相等则返回 true，否则返回 false / true if the objects are equal, otherwise false</returns>
        public bool Equals(CustomGameMode? other)
        {
            return other is not null && Id == other.Id;
        }

        /// <summary>
        ///     获取游戏模式的唯一标识符。
        ///     Gets the unique identifier of the game mode.
        /// </summary>
        public int Id { get; }

        /// <summary>
        ///     获取游戏模式的显示名称。
        ///     Gets the display name of the game mode.
        /// </summary>
        public string Name { get; }

        /// <summary>
        ///     返回游戏模式的字符串表示。
        ///     Returns the string representation of the game mode.
        /// </summary>
        /// <returns>游戏模式名称 / Game mode name</returns>
        public override string ToString()
        {
            return Name;
        }

        /// <summary>
        ///     确定指定的对象是否等于当前对象。
        ///     Determines whether the specified object is equal to the current object.
        /// </summary>
        /// <param name="obj">要比较的对象 / The object to compare</param>
        /// <returns>如果对象相等则返回 true，否则返回 false / true if the objects are equal, otherwise false</returns>
        public override bool Equals(object? obj)
        {
            return Equals(obj as CustomGameMode);
        }

        /// <summary>
        ///     返回当前对象的哈希码。
        ///     Returns the hash code for the current object.
        /// </summary>
        /// <returns>哈希码 / Hash code</returns>
        public override int GetHashCode()
        {
            return Id.GetHashCode();
        }
    }
}
