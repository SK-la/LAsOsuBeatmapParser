// Copyright (c) SK_la. Licensed under the MIT Licence.

using System.Collections.Generic;

namespace LAsOsuBeatmapParser.Objects
{
    /// <summary>
    /// A HitObject describes an object in a Beatmap.
    /// <para>
    /// HitObjects may contain more properties for which you should be checking through the IHas* types.
    /// </para>
    /// </summary>
    public abstract class HitObject
    {
        /// <summary>
        /// The time at which the HitObject starts.
        /// </summary>
        public double StartTime { get; set; }

        /// <summary>
        /// The samples to be played when this hit object is hit.
        /// </summary>
        public IList<HitSampleInfo> Samples { get; set; } = new List<HitSampleInfo>();
    }
}
