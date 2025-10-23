namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// 表示谱面的元数据。
/// </summary>
public class BeatmapMetadata : IBeatmapMetadataInfo
{
    /// <summary>
    /// 谱面标题。
    /// </summary>
    public string Title { get; set; } = string.Empty;

    /// <summary>
    /// The unicode title of this beatmap.
    /// </summary>
    public string TitleUnicode { get; set; } = string.Empty;

    /// <summary>
    /// 谱面艺术家。
    /// </summary>
    public string Artist { get; set; } = string.Empty;

    /// <summary>
    /// The unicode artist of this beatmap.
    /// </summary>
    public string ArtistUnicode { get; set; } = string.Empty;

    /// <summary>
    /// 谱面制作者（谱师）。
    /// </summary>
    public string Creator { get; set; } = string.Empty;

    /// <summary>
    /// 难度名/版本。
    /// </summary>
    public string Version { get; set; } = string.Empty;

    /// <summary>
    /// 歌曲来源。
    /// </summary>
    public string Source { get; set; } = string.Empty;

    /// <summary>
    /// 标签。
    /// </summary>
    public string Tags { get; set; } = string.Empty;

    /// <summary>
    /// 谱面ID。
    /// </summary>
    public int BeatmapID { get; set; }

    /// <summary>
    /// 谱面集ID。
    /// </summary>
    public int BeatmapSetID { get; set; }

    /// <summary>
    /// The author of this beatmap.
    /// </summary>
    public BeatmapAuthor Author { get; set; } = new();

    /// <summary>
    /// The preview time of this beatmap.
    /// </summary>
    public int PreviewTime { get; set; }

    /// <summary>
    /// The audio file of this beatmap.
    /// </summary>
    public string AudioFile { get; set; } = string.Empty;

    /// <summary>
    /// The background file of this beatmap.
    /// </summary>
    public string BackgroundFile { get; set; } = string.Empty;

    /// <summary>
    /// Explicit interface implementation.
    /// </summary>
    IUser IBeatmapMetadataInfo.Author => Author;

    /// <summary>
    /// Determines whether the specified object is equal to the current object.
    /// </summary>
    /// <param name="other">The object to compare with the current object.</param>
    /// <returns>true if the specified object is equal to the current object; otherwise, false.</returns>
    public bool Equals(IBeatmapMetadataInfo? other)
    {
        if (other == null) return false;
        return Title == other.Title && Artist == other.Artist && Creator == other.Author.Username;
    }
}

/// <summary>
/// Represents the author of a beatmap.
/// </summary>
public class BeatmapAuthor : IUser
{
    /// <summary>
    /// The username of the author.
    /// </summary>
    public string Username { get; set; } = string.Empty;
}
