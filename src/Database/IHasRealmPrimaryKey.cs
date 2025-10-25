namespace LAsOsuBeatmapParser.Database
{
    /// <summary>
    ///     Interface for objects that have a Realm primary key.
    /// </summary>
    public interface IHasRealmPrimaryKey
    {
        /// <summary>
        ///     The Realm primary key.
        /// </summary>
        long ID { get; set; }
    }
}
