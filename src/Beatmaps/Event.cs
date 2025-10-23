namespace LAsOsuBeatmapParser.Beatmaps;

/// <summary>
/// 表示谱面中的事件。
/// </summary>
public class Event
{
    /// <summary>
    /// 事件类型。
    /// </summary>
    public string Type { get; set; } = string.Empty;

    /// <summary>
    /// 起始时间。
    /// </summary>
    public double StartTime { get; set; }

    /// <summary>
    /// 事件参数。
    /// </summary>
    public string Params { get; set; } = string.Empty;

    /// <summary>
    /// 是否为注释行。
    /// </summary>
    public bool IsComment { get; set; }

    /// <summary>
    /// 返回事件的字符串表示。
    /// </summary>
    /// <returns>字符串表示。</returns>
    public override string ToString()
    {
        if (IsComment)
        {
            return Params;
        }
        return $"{Type},{StartTime},{Params}";
    }
}
