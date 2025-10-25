using LAsOsuBeatmapParser.Exceptions;

namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    ///     表示Mania专用谱面。
    /// </summary>
    public class ManiaBeatmap : Beatmap<ManiaHitObject>
    {
        /// <summary>
        ///     创建一个新的ManiaBeatmap。
        /// </summary>
        public ManiaBeatmap()
        {
            Mode                = GameMode.Mania;
            BeatmapInfo.Ruleset = new RulesetInfo { ID = 3, Name = "Mania", ShortName = "mania" };
        }

        /// <summary>
        ///     谱面键数（列数）。
        /// </summary>
        public int TotalColumns { get; set; }

        /// <summary>
        ///     验证模式是否为Mania。
        /// </summary>
        /// <exception cref="InvalidModeException">如果模式不是Mania则抛出异常。</exception>
        public void ValidateMode()
        {
            if (Mode.Id != GameMode.Mania.Id) throw new InvalidModeException("Beatmap mode must be Mania for ManiaBeatmap.");
        }
    }
}
