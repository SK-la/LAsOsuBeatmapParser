// Copyright (c) ppy Pty Ltd <contact@ppy.sh>. Licensed under the MIT Licence.
// See the LICENCE file in the repository root for full licence text.

using System;
using System.Runtime.InteropServices;
using System.Text;

namespace LAsOsuBeatmapParser.Analysis
{
    /// <summary>
    ///     Rust实现的SR计算器
    /// </summary>
    public static class SRCalculatorRust
    {
        private const string DllName = "rust_sr_calculator.dll";

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true, CharSet = CharSet.Ansi)]
        private static extern double calculate_sr_from_osu_file(IntPtr pathPtr, UIntPtr len);

        /// <summary>
        ///     文件解析SR算法，rust实现，失败返回null
        /// </summary>
        /// <param name="filePath"></param>
        /// <returns></returns>
        public static double? CalculateSR_FromFile(string filePath)
        {
            if (string.IsNullOrEmpty(filePath))
            {
                Console.Error.WriteLine($"[SR][ERROR] 文件路径为空");
                return null;
            }

            try
            {
                byte[] pathBytes = Encoding.UTF8.GetBytes(filePath);
                IntPtr pathPtr   = Marshal.AllocHGlobal(pathBytes.Length);
                Marshal.Copy(pathBytes, 0, pathPtr, pathBytes.Length);

                double result = calculate_sr_from_osu_file(pathPtr, (UIntPtr)pathBytes.Length);

                Marshal.FreeHGlobal(pathPtr);

                // Check for error (negative values indicate errors)
                if (result < 0.0)
                {
                    string reason = result switch
                    {
                        -2.0 => "路径字符串无效",
                        -3.0 => "文件打开失败",
                        -4.0 => "解析失败",
                        -5.0 => "数据非法",
                        -6.0 => "SR计算失败",
                        -7.0 => "SR计算panic",
                        _ => "未知错误"
                    };
                    Console.Error.WriteLine($"[SR][ERROR] 文件: {filePath}, 错误: {reason} (错误码: {result})");
                    return null;
                }

                if (double.IsNaN(result))
                {
                    Console.Error.WriteLine($"[SR][ERROR] 文件: {filePath}, 错误: 计算结果为NaN");
                    return null;
                }

                return result;
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"[SR][ERROR] 文件: {filePath}, 异常: {ex.Message}");
                return null;
            }
        }
    }
}
