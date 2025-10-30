using LAsOsuBeatmapParser.Beatmaps;

namespace LAsOsuBeatmapParser.Extensions
{
    /// <summary>
    ///     应用于单个 HitObject 的转换接口。
    ///     实现此接口的类可以修改 HitObject 的属性，如位置、时间等。
    /// </summary>
    public interface IApplyToHitObject
    {
        /// <summary>
        ///     将转换应用到指定的 HitObject。
        /// </summary>
        /// <param name="hitObject">要修改的 HitObject 对象。</param>
        void ApplyToHitObject(HitObject hitObject);
    }
}
