using System;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// Represents a ruleset.
/// </summary>
public interface IRulesetInfo : IEquatable<IRulesetInfo>
{
    /// <summary>
    /// The ID of this ruleset.
    /// </summary>
    int ID { get; }

    /// <summary>
    /// The name of this ruleset.
    /// </summary>
    string Name { get; }

    /// <summary>
    /// The short name of this ruleset.
    /// </summary>
    string ShortName { get; }

    /// <summary>
    /// The instantiation info of this ruleset.
    /// </summary>
    string InstantiationInfo { get; }
}
