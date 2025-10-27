using System;

namespace LAsOsuBeatmapParser.Extensions
{
    /// <summary>
    ///     Mania 相关的扩展方法。
    /// </summary>
    public static class ManiaExtensions
    {
        /// <summary>
        ///     从列号计算x坐标，使用拟合公式，和编辑器结果基本一致。
        /// </summary>
        /// <param name="totalColumn">总列数。</param>
        /// <param name="index">列号（0-based）。</param>
        /// <returns>x坐标。</returns>
        public static int GetPositionX(int totalColumn, int index)
        {
            // 修正公式：index从0开始
            double result = (index * 512.0 / totalColumn) + (256.0 / totalColumn);
            return (int)Math.Round(result);
        }

        /// <summary>
        ///     从x坐标计算列号，返回0-based列号。
        /// </summary>
        /// <param name="totalColumn">总列数。</param>
        /// <param name="x">x坐标。</param>
        /// <returns>列号（0-based）。</returns>
        public static int GetColumnFromX(int totalColumn, float x)
        {
            // 正确公式：floor(x * totalColumn / 512)
            return (int)Math.Floor(x * totalColumn / 512.0);
        }
    }
}
