using System.Collections.Generic;

namespace LAsOsuBeatmapParser.Beatmaps.ControlPoints;

/// <summary>
/// Contains information about control points.
/// </summary>
public partial class ControlPoint
{
    /// <summary>
    /// Gets the timing points.
    /// </summary>
    public IEnumerable<TimingPoint> TimingPoints { get; set; } = new List<TimingPoint>();

    /// <summary>
    /// Gets the effect points.
    /// </summary>
    public IEnumerable<EffectPoint> EffectPoints { get; set; } = new List<EffectPoint>();
}

/// <summary>
/// Represents an effect control point.
/// </summary>
public class EffectPoint
{
    /// <summary>
    /// The time of this control point.
    /// </summary>
    public double Time { get; set; }

    /// <summary>
    /// Whether kiai mode is enabled.
    /// </summary>
    public bool KiaiMode { get; set; }
}

/// <summary>
/// Contains difficulty information for a beatmap.
/// </summary>
public interface IBeatmapDifficultyInfoInternal
{
    /// <summary>
    /// The overall difficulty.
    /// </summary>
    double OverallDifficulty { get; }
}
