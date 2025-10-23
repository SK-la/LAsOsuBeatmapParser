using System;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// Exception thrown when a beatmap cannot be converted for a specific ruleset.
/// </summary>
public class BeatmapInvalidForRulesetException : Exception
{
    /// <summary>
    /// Creates a new <see cref="BeatmapInvalidForRulesetException"/>.
    /// </summary>
    public BeatmapInvalidForRulesetException()
    {
    }

    /// <summary>
    /// Creates a new <see cref="BeatmapInvalidForRulesetException"/> with a message.
    /// </summary>
    /// <param name="message">The message.</param>
    public BeatmapInvalidForRulesetException(string message) : base(message)
    {
    }

    /// <summary>
    /// Creates a new <see cref="BeatmapInvalidForRulesetException"/> with a message and inner exception.
    /// </summary>
    /// <param name="message">The message.</param>
    /// <param name="innerException">The inner exception.</param>
    public BeatmapInvalidForRulesetException(string message, Exception innerException) : base(message, innerException)
    {
    }
}
