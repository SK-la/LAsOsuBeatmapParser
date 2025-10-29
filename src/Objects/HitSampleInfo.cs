// Copyright (c) SK_la. Licensed under the MIT Licence.

using System;

namespace LAsOsuBeatmapParser.Objects
{
    /// <summary>
    ///     Describes a gameplay hit sample.
    /// </summary>
    public class HitSampleInfo : IEquatable<HitSampleInfo>
    {
        /// <summary>
        /// </summary>
        public const string HIT_NORMAL  = @"hitnormal";
        /// <summary>
        /// </summary>
        public const string HIT_WHISTLE = @"hitwhistle";
        /// <summary>
        /// </summary>
        public const string HIT_FINISH  = @"hitfinish";
        /// <summary>
        /// </summary>
        public const string HIT_CLAP    = @"hitclap";

        /// <summary>
        ///     打击音效信息的构造函数。
        /// </summary>
        /// <param name="name"></param>
        /// <param name="bank"></param>
        /// <param name="volume"></param>
        /// <param name="isLayered"></param>
        /// <param name="customSampleBank"></param>
        public HitSampleInfo(string name, string bank = "normal", int volume = 100, bool isLayered = false, string? customSampleBank = null)
        {
            Name             = name;
            Bank             = bank;
            Volume           = volume;
            IsLayered        = isLayered;
            CustomSampleBank = customSampleBank;
        }

        /// <summary>
        ///     The name of the sample to load.
        /// </summary>
        public string Name { get; }

        /// <summary>
        ///     The bank to load the sample from.
        /// </summary>
        public string Bank { get; }

        /// <summary>
        ///     The volume of the sample.
        /// </summary>
        public int Volume { get; }

        /// <summary>
        ///     公开指示此样本是否为分层样本的属性。
        /// </summary>
        public bool IsLayered { get; }

        /// <summary>
        ///     The custom sample bank to use.
        /// </summary>
        public string? CustomSampleBank { get; }

        /// <summary>
        ///     判断两个 HitSampleInfo 对象是否相等。
        /// </summary>
        /// <param name="other"></param>
        /// <returns></returns>
        public bool Equals(HitSampleInfo? other)
        {
            if (other is null) return false;

            return Name == other.Name &&
                   Bank == other.Bank &&
                   Volume == other.Volume &&
                   IsLayered == other.IsLayered &&
                   CustomSampleBank == other.CustomSampleBank;
        }

        /// <summary>
        ///     覆盖基类方法，判断对象是否相等。
        /// </summary>
        /// <param name="obj"></param>
        /// <returns></returns>
        public override bool Equals(object? obj)
        {
            return Equals(obj as HitSampleInfo);
        }

        /// <summary>
        ///     获取哈希代码。
        /// </summary>
        /// <returns></returns>
        public override int GetHashCode()
        {
            return HashCode.Combine(Name, Bank, Volume, IsLayered, CustomSampleBank);
        }
    }
}
