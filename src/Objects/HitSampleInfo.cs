// Copyright (c) SK_la. Licensed under the MIT Licence.

using System;

namespace LAsOsuBeatmapParser.Objects;

/// <summary>
/// Describes a gameplay hit sample.
/// </summary>
public class HitSampleInfo : IEquatable<HitSampleInfo>
{
    public const string HIT_NORMAL = @"hitnormal";
    public const string HIT_WHISTLE = @"hitwhistle";
    public const string HIT_FINISH = @"hitfinish";
    public const string HIT_CLAP = @"hitclap";

    /// <summary>
    /// The name of the sample to load.
    /// </summary>
    public string Name { get; }

    /// <summary>
    /// The bank to load the sample from.
    /// </summary>
    public string Bank { get; }

    /// <summary>
    /// The volume of the sample.
    /// </summary>
    public int Volume { get; }

    /// <summary>
    /// Whether this is a layered sample.
    /// </summary>
    public bool IsLayered { get; }

    /// <summary>
    /// The custom sample bank to use.
    /// </summary>
    public string? CustomSampleBank { get; }

    public HitSampleInfo(string name, string bank = "normal", int volume = 100, bool isLayered = false, string? customSampleBank = null)
    {
        Name = name;
        Bank = bank;
        Volume = volume;
        IsLayered = isLayered;
        CustomSampleBank = customSampleBank;
    }

    public bool Equals(HitSampleInfo? other)
    {
        if (other is null) return false;
        return Name == other.Name &&
               Bank == other.Bank &&
               Volume == other.Volume &&
               IsLayered == other.IsLayered &&
               CustomSampleBank == other.CustomSampleBank;
    }

    public override bool Equals(object? obj) => Equals(obj as HitSampleInfo);

    public override int GetHashCode() => HashCode.Combine(Name, Bank, Volume, IsLayered, CustomSampleBank);
}
