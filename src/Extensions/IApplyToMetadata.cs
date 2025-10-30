using LAsOsuBeatmapParser.Beatmaps;

namespace LAsOsuBeatmapParser.Extensions
{
    /// <summary>
    ///     应用于 Beatmap Metadata 的转换接口。
    ///     实现此接口的类可以修改 Metadata 的属性，如标题、艺术家等。
    /// </summary>
    public interface IApplyToMetadata
    {
        /// <summary>
        ///     将转换应用到指定的 Metadata。
        /// </summary>
        /// <param name="metadata">要修改的 BeatmapMetadata 对象。</param>
        void ApplyToMetadata(BeatmapMetadata metadata);
    }
}
