using System.Collections.Generic;

namespace LAsOsuBeatmapParser.Analysis
{
    /// <summary>
    ///     SR计算错误代码定义
    /// </summary>
    public static class SRErrorCodes
    {
        /// <summary>
        ///     错误代码与消息映射
        /// </summary>
        public static readonly Dictionary<double, string> ErrorMessages = new Dictionary<double, string>
        {
            [-2.0] = "路径字符串无效",
            [-3.0] = "文件打开失败",
            [-4.0] = "解析失败",
            [-5.0] = "数据非法",
            [-6.0] = "SR计算失败",
            [-7.0] = "SR计算panic"
        };

        /// <summary>
        ///     获取错误消息
        /// </summary>
        /// <param name="code">错误码</param>
        /// <returns>错误消息</returns>
        public static string GetErrorMessage(double code)
        {
            return ErrorMessages.TryGetValue(code, out var msg) ? msg : "未知错误";
        }
    }
}
