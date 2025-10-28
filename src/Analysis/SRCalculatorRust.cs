// Copyright (c) ppy Pty Ltd <contact@ppy.sh>. Licensed under the MIT Licence.
// See the LICENCE file in the repository root for full licence text.

using System;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using LAsOsuBeatmapParser.Beatmaps;

namespace LAsOsuBeatmapParser.Analysis
{
    public static class SRCalculatorRust
    {
        private const string DllName = "rust_sr_calculator.dll";

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true, CharSet = CharSet.Ansi)]
        private static extern double calculate_sr_from_osu_file(IntPtr pathPtr, UIntPtr len);

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
                if (result < 0.0)
                {
                    Console.WriteLine($"Rust SR calculation returned negative value: {result}");
                    return null;
                }

                return result;
            }
            catch (Exception ex)
            {
                // Return null on any error (including Rust panics)
                throw new Exception($"Error calling Rust DLL: {ex.Message}");
            }
        }
    }
}
