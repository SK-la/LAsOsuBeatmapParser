namespace LAsOsuBeatmapParser.Database
{
    /// <summary>
    /// Interface for objects that have an online ID.
    /// </summary>
    public interface IHasOnlineID
    {
        /// <summary>
        /// The online ID.
        /// </summary>
        long OnlineID { get; set; }
    }
}
