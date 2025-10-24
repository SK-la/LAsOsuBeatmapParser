namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    /// 表示谱面中的TimingPoint。
    /// </summary>
    public class TimingPoint
    {
        /// <summary>
        /// 时间（毫秒）。
        /// </summary>
        public double Time { get; set; }

        /// <summary>
        /// 拍长（毫秒）。
        /// </summary>
        public double BeatLength { get; set; }

        /// <summary>
        /// 拍号（拍子记号）。
        /// </summary>
        public int Meter { get; set; } = 4;

        /// <summary>
        /// 采样集。
        /// </summary>
        public int SampleSet { get; set; }

        /// <summary>
        /// 采样索引。
        /// </summary>
        public int SampleIndex { get; set; }

        /// <summary>
        /// 音量。
        /// </summary>
        public int Volume { get; set; } = 100;

        /// <summary>
        /// 是否为继承TimingPoint。
        /// </summary>
        public bool Inherited { get; set; }

        /// <summary>
        /// 特效（kiai、首小节线省略等）。
        /// </summary>
        public int Effects { get; set; }
    }
}
