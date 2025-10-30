using LAsOsuBeatmapParser.Beatmaps;

namespace LAsOsuBeatmapParser.Extensions
{
    /// <summary>
    ///     Beatmap 转换的基类，提供基础功能如复制属性。
    ///     子类可以重写 ApplyToBeatmap，并在开头调用 base.ApplyToBeatmap() 以利用基础功能。
    /// </summary>
    public abstract class ApplyBeatmapBase : IApplyBeatmap
    {
        /// <summary>
        ///     将转换应用到指定的 Beatmap。
        ///     基础实现：可以在这里复制或初始化属性（例如创建 HitObject 的副本）。
        ///     子类应调用 base.ApplyToBeatmap(beatmap) 以确保基础功能执行。
        /// </summary>
        /// <param name="beatmap">要修改的 Beatmap 对象。</param>
        public virtual void ApplyToBeatmap(Beatmap beatmap)
        {
            // 基础功能：复制 HitObject 属性（例如创建新列表以避免修改原对象）
            // 注意：Beatmap 是引用类型，这里假设子类负责具体修改
            // 如果需要深拷贝，可以在这里实现
        }
    }
}
