using System;

namespace LAsOsuBeatmapParser.Exceptions;

/// <summary>
/// 谱面解析失败时抛出的异常。
/// </summary>
public class BeatmapParseException : Exception
{
    /// <summary>
    /// 出错的行号。
    /// </summary>
    public int? LineNumber { get; }

    /// <summary>
    /// 创建 BeatmapParseException。
    /// </summary>
    /// <param name="message">错误信息。</param>
    public BeatmapParseException(string message)
        : base(message)
    {
    }

    /// <summary>
    /// 创建 BeatmapParseException。
    /// </summary>
    /// <param name="message">错误信息。</param>
    /// <param name="lineNumber">出错的行号。</param>
    public BeatmapParseException(string message, int lineNumber)
        : base(message)
    {
        LineNumber = lineNumber;
    }

    /// <summary>
    /// 创建 BeatmapParseException。
    /// </summary>
    /// <param name="message">错误信息。</param>
    /// <param name="innerException">内部异常。</param>
    public BeatmapParseException(string message, Exception innerException)
        : base(message, innerException)
    {
    }

    /// <summary>
    /// 创建 BeatmapParseException。
    /// </summary>
    /// <param name="message">错误信息。</param>
    /// <param name="lineNumber">出错的行号。</param>
    /// <param name="innerException">内部异常。</param>
    public BeatmapParseException(string message, int lineNumber, Exception innerException)
        : base(message, innerException)
    {
        LineNumber = lineNumber;
    }
}
