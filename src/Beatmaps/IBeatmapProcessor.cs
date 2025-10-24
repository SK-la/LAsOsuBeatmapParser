namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    /// Provides functionality to alter a <see cref="IBeatmap"/> after it has been converted.
    /// 提供在转换后修改 <see cref="IBeatmap"/> 的功能。
    /// </summary>
    public interface IBeatmapProcessor
    {
        /// <summary>
        /// The <see cref="IBeatmap"/> to process. This should already be converted to the applicable <see cref="IRulesetInfo"/>.
        /// 要处理的 <see cref="IBeatmap"/>。这应该已经被转换为适用的 <see cref="IRulesetInfo"/>。
        /// </summary>
        IBeatmap Beatmap { get; }

        /// <summary>
        /// Processes the converted <see cref="Beatmap"/> prior to <see cref="HitObject"/> defaults being applied.
        /// 在调用 <see cref="HitObject"/> 默认设置之前处理已转换的 <see cref="Beatmap"/>。
        /// <para>
        /// Nested <see cref="HitObject"/>s generated during defaults application will not be present by this point,
        /// and no mods will have been applied to the <see cref="HitObject"/>s.
        /// </para>
        /// <para>
        /// 在应用默认设置期间生成的嵌套 <see cref="HitObject"/> 在此时不会存在，
        /// 并且没有mod会被应用到 <see cref="HitObject"/> 上。
        /// </para>
        /// </summary>
        /// <remarks>
        /// This method should be called once immediately after beatmap conversion and before any post-processing.
        /// 此方法应该在谱面转换后立即调用一次，在任何后处理之前。
        /// </remarks>
        void PreProcess();

        /// <summary>
        /// Processes the converted <see cref="Beatmap"/> after defaults have been applied to <see cref="HitObject"/>s.
        /// 在对 <see cref="HitObject"/> 应用默认设置后处理已转换的 <see cref="Beatmap"/>。
        /// <para>
        /// Nested <see cref="HitObject"/>s generated during defaults application will be present by this point,
        /// and mods will have been applied to the <see cref="HitObject"/>s.
        /// </para>
        /// <para>
        /// 在应用默认设置期间生成的嵌套 <see cref="HitObject"/> 在此时会存在，
        /// 并且mod已经被应用到 <see cref="HitObject"/> 上。
        /// </para>
        /// </summary>
        /// <remarks>
        /// This method should be called once immediately after beatmap post-processing.
        /// 此方法应该在谱面后处理后立即调用一次。
        /// </remarks>
        void PostProcess();
    }
}
