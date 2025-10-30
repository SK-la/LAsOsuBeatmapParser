using LAsOsuBeatmapParser.Beatmaps;

namespace LAsOsuBeatmapParser.Extensions
{
    /// <summary>
    ///     应用于 Beatmap Difficulty 的转换接口。
    ///     实现此接口的类可以修改 Difficulty 的属性，如 OverallDifficulty、ApproachRate 等。
    /// </summary>
    public interface IApplyToDifficulty
    {
        /// <summary>
        ///     将转换应用到指定的 Difficulty。
        /// </summary>
        /// <param name="difficulty">要修改的 BeatmapDifficulty 对象。</param>
        void ApplyToDifficulty(BeatmapDifficulty difficulty);
    }
}
