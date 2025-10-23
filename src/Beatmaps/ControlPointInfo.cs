using System.Collections.Generic;

namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// Contains all the control points in a beatmap.
/// </summary>
public class ControlPointInfo
{
    /// <summary>
    /// All timing points.
    /// </summary>
    public IReadOnlyList<TimingControlPoint> TimingPoints => timingPoints;

    private readonly List<TimingControlPoint> timingPoints = new();

    /// <summary>
    /// All difficulty points.
    /// </summary>
    public IReadOnlyList<DifficultyControlPoint> DifficultyPoints => difficultyPoints;

    private readonly List<DifficultyControlPoint> difficultyPoints = new();

    /// <summary>
    /// All effect points.
    /// </summary>
    public IReadOnlyList<EffectControlPoint> EffectPoints => effectPoints;

    private readonly List<EffectControlPoint> effectPoints = new();

    /// <summary>
    /// All sample points.
    /// </summary>
    public IReadOnlyList<SampleControlPoint> SamplePoints => samplePoints;

    private readonly List<SampleControlPoint> samplePoints = new();

    /// <summary>
    /// Adds a timing control point.
    /// </summary>
    /// <param name="point">The point to add.</param>
    public void Add(TimingControlPoint point) => timingPoints.Add(point);

    /// <summary>
    /// Adds a difficulty control point.
    /// </summary>
    /// <param name="point">The point to add.</param>
    public void Add(DifficultyControlPoint point) => difficultyPoints.Add(point);

    /// <summary>
    /// Adds an effect control point.
    /// </summary>
    /// <param name="point">The point to add.</param>
    public void Add(EffectControlPoint point) => effectPoints.Add(point);

    /// <summary>
    /// Adds a sample control point.
    /// </summary>
    /// <param name="point">The point to add.</param>
    public void Add(SampleControlPoint point) => samplePoints.Add(point);
}
