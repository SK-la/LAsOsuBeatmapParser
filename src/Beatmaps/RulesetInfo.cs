namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    /// Represents a ruleset.
    /// </summary>
    public class RulesetInfo : IRulesetInfo
    {
        /// <summary>
        /// The ID of this ruleset.
        /// </summary>
        public int ID { get; set; }

        /// <summary>
        /// The name of this ruleset.
        /// </summary>
        public string Name { get; set; } = string.Empty;

        /// <summary>
        /// The short name of this ruleset.
        /// </summary>
        public string ShortName { get; set; } = string.Empty;

        /// <summary>
        /// The instantiation info of this ruleset.
        /// </summary>
        public string InstantiationInfo { get; set; } = string.Empty;

        /// <summary>
        /// Determines whether the specified object is equal to the current object.
        /// </summary>
        /// <param name="other">The object to compare with the current object.</param>
        /// <returns>true if the specified object is equal to the current object; otherwise, false.</returns>
        public bool Equals(IRulesetInfo? other)
        {
            if (other == null) return false;
            return ID == other.ID;
        }
    }
}
