using LAsOsuBeatmapParser.Beatmaps;

namespace LAsOsuBeatmapParser.Extensions
{
    /// <summary>
    ///     应用于 Beatmap 的转换接口，类似于官方的 IApplyBeatmap。
    ///     实现此接口的类可以修改 Beatmap 的属性，如 HitObject、TimingPoints 等。
    /// </summary>
    public interface IApplyBeatmap
    {
        /// <summary>
        ///     将转换应用到指定的 Beatmap。
        /// </summary>
        /// <param name="beatmap">要修改的 Beatmap 对象。</param>
        void ApplyToBeatmap(Beatmap beatmap);
    }
}