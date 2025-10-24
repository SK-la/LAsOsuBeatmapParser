using System;
using System.Collections.Generic;
using LAsOsuBeatmapParser.Database;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// A realm model containing metadata for a single beatmap difficulty.
/// This should generally include anything which is required to be filtered on at song select, or anything pertaining to storage of beatmaps in the client.
/// </summary>
public class BeatmapInfo : IBeatmapInfo, IHasRealmPrimaryKey, IHasOnlineID
{
    /// <summary>
    /// The difficulty name of this beatmap.
    /// </summary>
    public string DifficultyName { get; set; } = string.Empty;

    /// <summary>
    /// The Realm primary key.
    /// </summary>
    public long ID { get; set; }

    /// <summary>
    /// The online ID.
    /// </summary>
    public long OnlineID { get; set; }

    /// <summary>
    /// The ruleset of this beatmap.
    /// </summary>
    public RulesetInfo Ruleset { get; set; } = null!;

    /// <summary>
    /// The difficulty settings of this beatmap.
    /// </summary>
    public BeatmapDifficulty Difficulty { get; set; } = null!;

    /// <summary>
    /// The metadata of this beatmap.
    /// </summary>
    public BeatmapMetadata Metadata { get; set; } = null!;

    /// <summary>
    /// The time at which this beatmap was last played by the local user.
    /// </summary>
    public DateTimeOffset? LastPlayed { get; set; }

    /// <summary>
    /// The beat divisor for the editor.
    /// </summary>
    public int BeatDivisor { get; set; } = 4;

    /// <summary>
    /// The time in milliseconds when last exiting the editor with this beatmap loaded.
    /// </summary>
    public double? EditorTimestamp { get; set; }

    /// <summary>
    /// The total length in milliseconds of this beatmap.
    /// </summary>
    public double Length { get; set; }

    /// <summary>
    /// The most common BPM of this beatmap.
    /// </summary>
    public double BPM { get; set; }

    /// <summary>
    /// The SHA-256 hash representing this beatmap's contents.
    /// </summary>
    public string Hash { get; } = string.Empty;

    /// <summary>
    /// MD5 is kept for legacy support (matching against replays etc.).
    /// </summary>
    public string MD5Hash { get; set; } = string.Empty;

    /// <summary>
    /// The basic star rating for this beatmap (with no mods applied).
    /// Defaults to -1 (meaning not-yet-calculated).
    /// </summary>
    public double StarRating { get; set; } = -1;

    /// <summary>
    /// The number of hitobjects in the beatmap with a distinct end time.
    /// Defaults to -1 (meaning not-yet-calculated).
    /// </summary>
    /// <remarks>
    /// Canonically, these are hitobjects are either sliders or spinners.
    /// </remarks>
    public int EndTimeObjectCount { get; set; } = -1;

    /// <summary>
    /// The total number of hitobjects in the beatmap.
    /// Defaults to -1 (meaning not-yet-calculated).
    /// </summary>
    public int TotalObjectCount { get; set; } = -1;

    /// <summary>
    /// The beatmap set this beatmap is part of.
    /// </summary>
    public IBeatmapSetInfo? BeatmapSet { get; set; }

    /// <summary>
    /// Explicit interface implementations.
    /// </summary>
    IBeatmapMetadataInfo IBeatmapInfo.Metadata => Metadata;
    IBeatmapSetInfo? IBeatmapInfo.BeatmapSet => BeatmapSet;
    IRulesetInfo IBeatmapInfo.Ruleset => Ruleset;
    IBeatmapDifficultyInfo IBeatmapInfo.Difficulty => Difficulty;

    /// <summary>
    /// Returns a string representation of this beatmap.
    /// </summary>
    /// <returns>The string representation.</returns>
    public override string ToString() => $"{Metadata.Artist} - {Metadata.Title} [{DifficultyName}]";

    /// <summary>
    /// Determines whether the specified object is equal to the current object.
    /// </summary>
    /// <param name="other">The object to compare with the current object.</param>
    /// <returns>true if the specified object is equal to the current object; otherwise, false.</returns>
    public bool Equals(IBeatmapInfo? other)
    {
        if (other == null) return false;
        return Hash == other.Hash;
    }

    /// <summary>
    /// Determines whether the specified object is equal to the current object.
    /// </summary>
    /// <param name="obj">The object to compare with the current object.</param>
    /// <returns>true if the specified object is equal to the current object; otherwise, false.</returns>
    public override bool Equals(object? obj) => obj is IBeatmapInfo other && Equals(other);

    /// <summary>
    /// Returns a hash code for the current object.
    /// </summary>
    /// <returns>A hash code for the current object.</returns>
    public override int GetHashCode() => Hash.GetHashCode();
}
