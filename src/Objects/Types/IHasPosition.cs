// Copyright (c) SK_la. Licensed under the MIT Licence.

namespace LAsOsuBeatmapParser.Objects.Types
{
    /// <summary>
    /// A HitObject that has a starting position.
    /// </summary>
    public interface IHasPosition : IHasXPosition, IHasYPosition
    {
        /// <summary>
        /// The starting position of the HitObject.
        /// </summary>
        (float X, float Y) Position { get; set; }
    }
}
