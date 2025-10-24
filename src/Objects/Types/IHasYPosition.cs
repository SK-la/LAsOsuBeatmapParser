// Copyright (c) SK_la. Licensed under the MIT Licence.

namespace LAsOsuBeatmapParser.Objects.Types;

/// <summary>
/// A HitObject that has a starting Y-position.
/// </summary>
public interface IHasYPosition
{
    /// <summary>
    /// The starting Y-position of this HitObject.
    /// </summary>
    float Y { get; set; }
}
