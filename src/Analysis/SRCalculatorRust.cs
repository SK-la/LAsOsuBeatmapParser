// Copyright (c) ppy Pty Ltd <contact@ppy.sh>. Licensed under the MIT Licence.
// See the LICENCE file in the repository root for full licence text.

using System;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.IO;

namespace LAsOsuBeatmapParser.Analysis
{
    public static class SRCalculatorRust
    {
        private const string DllName = "rust_sr_calculator.dll";

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true, CharSet = CharSet.Ansi)]
        private static extern IntPtr calculate_sr_from_osu_file(IntPtr pathPtr, UIntPtr len);

        public static double? CalculateSR(string filePath)
        {
            if (string.IsNullOrEmpty(filePath))
                return null;

            try
            {
                byte[] pathBytes = System.Text.Encoding.UTF8.GetBytes(filePath);
                IntPtr pathPtr = Marshal.AllocHGlobal(pathBytes.Length);
                Marshal.Copy(pathBytes, 0, pathPtr, pathBytes.Length);

                IntPtr resultPtr = calculate_sr_from_osu_file(pathPtr, (UIntPtr)pathBytes.Length);

                Marshal.FreeHGlobal(pathPtr);

                if (resultPtr == IntPtr.Zero)
                    return null;

                string resultJson = Marshal.PtrToStringUTF8(resultPtr)!;
                Marshal.FreeHGlobal(resultPtr);

                var result = JsonSerializer.Deserialize<JsonElement>(resultJson);
                if (result.TryGetProperty("sr", out JsonElement srElement) && srElement.ValueKind == JsonValueKind.Number)
                    return srElement.GetDouble();
            }
            catch (Exception)
            {
                // Return null on any error (including Rust panics)
                return null;
            }

            return null;
        }
    }
}
