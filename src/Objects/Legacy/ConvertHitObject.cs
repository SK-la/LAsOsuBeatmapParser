// Copyright (c) SK_la. Licensed under the MIT Licence.

using LAsOsuBeatmapParser.Objects.Types;

namespace LAsOsuBeatmapParser.Objects.Legacy
{
    /// <summary>
    ///     Represents a legacy hit object.
    /// </summary>
    public class ConvertHitObject : HitObject, IHasPosition
    {
        /// <summary>
        ///     The X position of this hit object.
        /// </summary>
        public float X { get; set; }

        /// <summary>
        ///     The Y position of this hit object.
        /// </summary>
        public float Y { get; set; }

        /// <summary>
        ///     The position of this hit object.
        /// </summary>
        public (float X, float Y) Position
        {
            get => (X, Y);
            set => (X, Y) = value;
        }
    }
}
