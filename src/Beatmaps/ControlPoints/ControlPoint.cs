namespace LAsOsuBeatmapParser.Beatmaps.ControlPoints
{
    /// <summary>
    ///     A control point.
    /// </summary>
    public abstract partial class ControlPoint
    {
        /// <summary>
        ///     The time at which this control point takes effect.
        /// </summary>
        public double Time { get; set; }
    }

    /// <summary>
    ///     A timing control point.
    /// </summary>
    public class TimingControlPoint : ControlPoint
    {
        /// <summary>
        ///     The beat length.
        /// </summary>
        public double BeatLength { get; set; }

        /// <summary>
        ///     The time signature.
        /// </summary>
        public int TimeSignature { get; set; } = 4;
    }

    /// <summary>
    ///     A difficulty control point.
    /// </summary>
    public class DifficultyControlPoint : ControlPoint
    {
        /// <summary>
        ///     The speed multiplier.
        /// </summary>
        public double SpeedMultiplier { get; set; } = 1.0;
    }

    /// <summary>
    ///     An effect control point.
    /// </summary>
    public class EffectControlPoint : ControlPoint
    {
        /// <summary>
        ///     Whether kiai is enabled.
        /// </summary>
        public bool KiaiMode { get; set; }

        /// <summary>
        ///     Whether the first bar line should be omitted.
        /// </summary>
        public bool OmitFirstBarLine { get; set; }
    }

    /// <summary>
    ///     A sample control point.
    /// </summary>
    public class SampleControlPoint : ControlPoint
    {
        /// <summary>
        ///     The sample bank.
        /// </summary>
        public int SampleBank { get; set; }

        /// <summary>
        ///     The sample volume.
        /// </summary>
        public int SampleVolume { get; set; } = 100;
    }
}
