// Copyright (c) SK_la. Licensed under the MIT Licence.

namespace LAsOsuBeatmapParser.Objects
{
    /// <summary>
    /// Base class for parsing hit objects from strings.
    /// </summary>
    public abstract class HitObjectParser
    {
        /// <summary>
        /// Parses a hit object from a string.
        /// </summary>
        /// <param name="text">The string to parse.</param>
        /// <returns>The parsed hit object, or null if parsing failed.</returns>
        public abstract HitObject? Parse(string text);
    }
}
