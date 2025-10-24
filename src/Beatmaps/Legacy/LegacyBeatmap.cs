using System.Collections.Generic;

namespace LAsOsuBeatmapParser.Beatmaps.Legacy
{
    /// <summary>
    /// Represents a legacy beatmap format.
    /// </summary>
    public class LegacyBeatmap
    {
        /// <summary>
        /// The format version.
        /// </summary>
        public int FormatVersion { get; set; }

        /// <summary>
        /// The metadata.
        /// </summary>
        public LegacyMetadata Metadata { get; set; } = new LegacyMetadata();

        /// <summary>
        /// The difficulty.
        /// </summary>
        public LegacyDifficulty Difficulty { get; set; } = new LegacyDifficulty();

        /// <summary>
        /// The timing points.
        /// </summary>
        public List<LegacyTimingPoint> TimingPoints { get; set; } = new List<LegacyTimingPoint>();

        /// <summary>
        /// The hit objects.
        /// </summary>
        public List<LegacyHitObject> HitObjects { get; set; } = new List<LegacyHitObject>();
    }

    /// <summary>
    /// Legacy metadata.
    /// 旧版元数据。
    /// </summary>
    public class LegacyMetadata
    {
        /// <summary>
        /// The title of the beatmap.
        /// 谱面的标题。
        /// </summary>
        public string Title { get; set; } = string.Empty;

        /// <summary>
        /// The artist of the beatmap.
        /// 谱面的艺术家。
        /// </summary>
        public string Artist { get; set; } = string.Empty;

        /// <summary>
        /// The creator of the beatmap.
        /// 谱面的创建者。
        /// </summary>
        public string Creator { get; set; } = string.Empty;

        /// <summary>
        /// The version/difficulty name of the beatmap.
        /// 谱面的版本/难度名称。
        /// </summary>
        public string Version { get; set; } = string.Empty;
    }

    /// <summary>
    /// Legacy difficulty.
    /// 旧版难度设置。
    /// </summary>
    public class LegacyDifficulty
    {
        /// <summary>
        /// HP drain rate.
        /// HP消耗速率。
        /// </summary>
        public float HPDrainRate { get; set; }

        /// <summary>
        /// Circle size. For Mania mode, this represents key count.
        /// 圆圈大小。对于Mania模式，此值为键数。
        /// </summary>
        public float CircleSize { get; set; }

        /// <summary>
        /// Overall difficulty.
        /// 总体难度。
        /// </summary>
        public float OverallDifficulty { get; set; }

        /// <summary>
        /// Approach rate.
        /// 预判条出现速率。
        /// </summary>
        public float ApproachRate { get; set; }

        /// <summary>
        /// Slider multiplier.
        /// 滑条倍率。
        /// </summary>
        public float SliderMultiplier { get; set; }

        /// <summary>
        /// Slider tick rate.
        /// 滑条刻度。
        /// </summary>
        public float SliderTickRate { get; set; }
    }

    /// <summary>
    /// Legacy timing point.
    /// 旧版时间点。
    /// </summary>
    public class LegacyTimingPoint
    {
        /// <summary>
        /// Time in milliseconds.
        /// 时间（毫秒）。
        /// </summary>
        public double Time { get; set; }

        /// <summary>
        /// Beat length in milliseconds.
        /// 节拍长度（毫秒）。
        /// </summary>
        public double BeatLength { get; set; }

        /// <summary>
        /// Time signature numerator.
        /// 拍号分子。
        /// </summary>
        public int Meter { get; set; }

        /// <summary>
        /// Sample set.
        /// 采样集。
        /// </summary>
        public int SampleSet { get; set; }

        /// <summary>
        /// Sample index.
        /// 采样索引。
        /// </summary>
        public int SampleIndex { get; set; }

        /// <summary>
        /// Volume.
        /// 音量。
        /// </summary>
        public int Volume { get; set; }

        /// <summary>
        /// Whether this timing point is inherited.
        /// 此时间点是否被继承。
        /// </summary>
        public bool Inherited { get; set; }

        /// <summary>
        /// Effects.
        /// 效果。
        /// </summary>
        public int Effects { get; set; }
    }

    /// <summary>
    /// Legacy hit object.
    /// 旧版打击对象。
    /// </summary>
    public class LegacyHitObject
    {
        /// <summary>
        /// X position.
        /// X坐标。
        /// </summary>
        public int X { get; set; }

        /// <summary>
        /// Y position.
        /// Y坐标。
        /// </summary>
        public int Y { get; set; }

        /// <summary>
        /// Time in milliseconds.
        /// 时间（毫秒）。
        /// </summary>
        public double Time { get; set; }

        /// <summary>
        /// Object type.
        /// 对象类型。
        /// </summary>
        public int Type { get; set; }

        /// <summary>
        /// Hit sound.
        /// 打击音效。
        /// </summary>
        public int HitSound { get; set; }

        /// <summary>
        /// Extra parameters.
        /// 额外参数。
        /// </summary>
        public string Extras { get; set; } = string.Empty;
    }
}
