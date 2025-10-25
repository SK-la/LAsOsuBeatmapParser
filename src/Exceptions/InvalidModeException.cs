using System;

namespace LAsOsuBeatmapParser.Exceptions
{
    /// <summary>
    ///     当遇到无效游戏模式时抛出的异常。
    /// </summary>
    public class InvalidModeException : Exception
    {
        /// <summary>
        ///     创建 InvalidModeException。
        /// </summary>
        /// <param name="message">错误信息。</param>
        public InvalidModeException(string message)
            : base(message)
        {
        }

        /// <summary>
        ///     创建 InvalidModeException。
        /// </summary>
        /// <param name="message">错误信息。</param>
        /// <param name="innerException">内部异常。</param>
        public InvalidModeException(string message, Exception innerException)
            : base(message, innerException)
        {
        }
    }
}
