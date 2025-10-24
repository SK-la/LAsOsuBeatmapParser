// Copyright (c) SK_la. Licensed under the MIT Licence.

namespace LAsOsuBeatmapParser.Objects.Types
{
    /// <summary>
    /// A HitObject that has a starting X-position.
    /// </summary>
    public interface IHasXPosition
    {
        /// <summary>
        /// The starting X-position of this HitObject.
        /// </summary>
        float X { get; set; }
    }
}
