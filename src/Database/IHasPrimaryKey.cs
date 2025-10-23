namespace LAsOsuBeatmapParser.Database;

/// <summary>
/// Interface for objects that have a primary key.
/// </summary>
public interface IHasPrimaryKey
{
    /// <summary>
    /// The primary key.
    /// </summary>
    object PrimaryKey { get; }
}
