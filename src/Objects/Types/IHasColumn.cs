// Copyright (c) SK_la. Licensed under the MIT Licence.

namespace LAsOsuBeatmapParser.Objects.Types
{
    /// <summary>
    /// A type of hit object which lies in one of a number of predetermined columns.
    /// </summary>
    public interface IHasColumn
    {
        /// <summary>
        /// The column which the hit object lies in.
        /// </summary>
        int Column { get; }
    }
}
