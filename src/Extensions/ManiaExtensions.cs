using System;

namespace LAsOsuBeatmapParser.Extensions
{
    /// <summary>
    /// Mania 相关的扩展方法。
    /// </summary>
    public static class ManiaExtensions
    {
        /// <summary>
        /// 从列号计算x坐标，使用拟合公式，和编辑器结果基本一致。
        /// </summary>
        /// <param name="totalColumn">总列数。</param>
        /// <param name="index">列号（0-based）。</param>
        /// <returns>x坐标。</returns>
        public static int GetPositionX(int totalColumn, int index)
        {
            // 拟合公式，结果四舍五入，和编辑器结果基本一致
            double result = (index - 1) * 512.0 / totalColumn + 256.0 / totalColumn;
            return (int)Math.Round(result);
        }

        /// <summary>
        /// 从x坐标计算列号，返回0-based列号。
        /// </summary>
        /// <param name="totalColumn">总列数。</param>
        /// <param name="x">x坐标。</param>
        /// <returns>列号（0-based）。</returns>
        public static int GetColumnFromX(int totalColumn, float x)
        {
            // 逆运算：index = round((x - 256.0 / totalColumn) / (512.0 / totalColumn) + 1)
            double offset = 256.0 / totalColumn;
            double ratio = 512.0 / totalColumn;
            double result = (x - offset) / ratio + 1;
            return (int)Math.Round(result) - 1; // 转换为0-based
        }
    }
}