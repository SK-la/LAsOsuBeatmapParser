namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// 表示Mania模式下的Hold。
/// </summary>
public class ManiaHoldNote : HitObject
{
    /// <summary>
    /// Mania长按note
    /// </summary>
    public ManiaHoldNote()
    {
        Type = HitObjectType.ManiaHold;
    }

    /// <summary>
    /// 创建Mania长按音符
    /// </summary>
    /// <param name="startTime">开始时间</param>
    /// <param name="endTime">结束时间</param>
    /// <param name="column">列索引</param>
    /// <param name="keyCount">总键数</param>
    public ManiaHoldNote(double startTime, double endTime, int column, int keyCount)
    {
        StartTime = startTime;
        EndTime = endTime;
        Column = column;
        KeyCount = keyCount;
        Type = HitObjectType.ManiaHold;
    }

    /// <summary>
    /// Hold的结束时间。
    /// </summary>
    public override double EndTime { get; set; }
    /// <summary>
    /// 所在列（键）。
    /// </summary>
    public int Column { get; set; }

    /// <summary>
    /// 总键数。
    /// </summary>
    public int KeyCount { get; set; }

    /// <summary>
    /// Returns a string representation of this hold note.
    /// </summary>
    /// <returns>The string representation.</returns>
    public override string ToString()
    {
        // Mania hold notes: x,y,time,type,hitSound,endTime:hitSample
        // Use official osu formula: x = (column + 0.5) * (512 / keyCount)
        const int totalWidth = 512;
        int keyCount = KeyCount > 0 ? KeyCount : 4; // Default to 4 if not set
        float columnWidth = totalWidth / (float)keyCount;
        int x = (int)((Column + 0.5f) * columnWidth);
        int y = 192; // Standard y position
        int type = 128; // Hold note type
        int hitSound = 0;
        string hitSample = $"{(int)EndTime}:0:0:0:";

        return $"{x},{y},{(int)StartTime},{type},{hitSound},{hitSample}";
    }
}
