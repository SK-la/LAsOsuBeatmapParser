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
                return null;

            try
            {
                byte[] pathBytes = Encoding.UTF8.GetBytes(filePath);
                IntPtr pathPtr   = Marshal.AllocHGlobal(pathBytes.Length);
                Marshal.Copy(pathBytes, 0, pathPtr, pathBytes.Length);

                double result = calculate_sr_from_osu_file(pathPtr, (UIntPtr)pathBytes.Length);

                Marshal.FreeHGlobal(pathPtr);

                // Check for error (negative values indicate errors)
                if (result < 0.0 || double.IsNaN(result))
                {
                    Console.WriteLine($"Rust SR calculation returned invalid value: {result}");
                    return null;
                }

                return result;
            }
            catch (Exception)
            {
                // Return null on any error (including Rust panics)
                // throw new Exception($"Error calling Rust DLL: {ex.Message}");
                return null;
            }
        }
    }
}
