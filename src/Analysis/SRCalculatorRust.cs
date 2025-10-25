// Copyright (c) ppy Pty Ltd <contact@ppy.sh>. Licensed under the MIT Licence.
// See the LICENCE file in the repository root for full licence text.

using System;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.IO;
using System.Collections.Generic;
using System.Linq;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Extensions;

namespace LAsOsuBeatmapParser.Analysis
{
    public static class SRCalculatorRust
    {
        private const string DllName = "rust_sr_calculator.dll";

        // C-compatible structures matching Rust definitions
        [StructLayout(LayoutKind.Sequential)]
        public struct CHitObject
        {
            public int col;
            public int start_time;
            public int end_time;
        }

        [StructLayout(LayoutKind.Sequential)]
        public struct CBeatmapData
        {
            public double overall_difficulty;
            public double circle_size;
            public UIntPtr hit_objects_count; // usize in Rust
            public IntPtr hit_objects_ptr; // *const CHitObject in Rust
        }

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true, CharSet = CharSet.Ansi)]
        private static extern IntPtr calculate_sr_from_osu_file(IntPtr pathPtr, UIntPtr len);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true, CharSet = CharSet.Ansi)]
        private static extern IntPtr calculate_sr_from_osu_content(IntPtr contentPtr, UIntPtr len);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true, CharSet = CharSet.Ansi)]
        private static extern IntPtr calculate_sr_from_json(IntPtr jsonPtr, UIntPtr len);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        private static extern double calculate_sr_from_struct(CBeatmapData data);

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

                using var doc = JsonDocument.Parse(resultJson);
                var root = doc.RootElement;
                if (root.TryGetProperty("sr", out JsonElement srElement) && srElement.ValueKind == JsonValueKind.Number)
                    return srElement.GetDouble();
            }
            catch (Exception)
            {
                // Return null on any error (including Rust panics)
                return null;
            }

            return null;
        }

        public static double? CalculateSR_FromFile(string filePath)
        {
            return CalculateSR(filePath);
        }

        public static double? CalculateSR_FromContent(string content)
        {
            if (string.IsNullOrEmpty(content))
                return null;

            try
            {
                byte[] contentBytes = System.Text.Encoding.UTF8.GetBytes(content);
                IntPtr contentPtr = Marshal.AllocHGlobal(contentBytes.Length);
                Marshal.Copy(contentBytes, 0, contentPtr, contentBytes.Length);

                IntPtr resultPtr = calculate_sr_from_osu_content(contentPtr, (UIntPtr)contentBytes.Length);

                Marshal.FreeHGlobal(contentPtr);

                if (resultPtr == IntPtr.Zero)
                    return null;

                string resultJson = Marshal.PtrToStringUTF8(resultPtr)!;
                Marshal.FreeHGlobal(resultPtr);

                using var doc = JsonDocument.Parse(resultJson);
                var root = doc.RootElement;
                if (root.TryGetProperty("sr", out JsonElement srElement) && srElement.ValueKind == JsonValueKind.Number)
                    return srElement.GetDouble();
            }
            catch (Exception)
            {
                return null;
            }

            return null;
        }

        public static double? CalculateSR_FromJson(string json)
        {
            if (string.IsNullOrEmpty(json))
                return null;

            try
            {
                byte[] jsonBytes = System.Text.Encoding.UTF8.GetBytes(json);
                IntPtr jsonPtr = Marshal.AllocHGlobal(jsonBytes.Length);
                Marshal.Copy(jsonBytes, 0, jsonPtr, jsonBytes.Length);

                IntPtr resultPtr = calculate_sr_from_json(jsonPtr, (UIntPtr)jsonBytes.Length);

                Marshal.FreeHGlobal(jsonPtr);

                if (resultPtr == IntPtr.Zero)
                    return null;

                string resultJson = Marshal.PtrToStringUTF8(resultPtr)!;
                Marshal.FreeHGlobal(resultPtr);

                using var doc = JsonDocument.Parse(resultJson);
                var root = doc.RootElement;
                if (root.TryGetProperty("sr", out JsonElement srElement) && srElement.ValueKind == JsonValueKind.Number)
                    return srElement.GetDouble();
            }
            catch (Exception)
            {
                return null;
            }

            return null;
        }

        public static double CalculateSR_FromStruct(CBeatmapData data)
        {
            return calculate_sr_from_struct(data);
        }

        #region 单元测试用检查

        public static string ConvertBeatmapToJson(Beatmap beatmap)
        {
            var jsonBeatmap = new
            {
                difficulty_section = new
                {
                    overall_difficulty = beatmap.BeatmapInfo.Difficulty.OverallDifficulty,
                    circle_size = beatmap.BeatmapInfo.Difficulty.CircleSize
                },
                hit_objects = beatmap.HitObjects.Select(ho => new
                {
                    position = new { x = ho.Position.X, y = ho.Position.Y },
                    start_time = ho.StartTime,
                    end_time = ho.EndTime
                })
            };

            var options = new JsonSerializerOptions { PropertyNamingPolicy = JsonNamingPolicy.CamelCase };
            return JsonSerializer.Serialize(jsonBeatmap, options);
        }

        public static CBeatmapData ConvertBeatmapToStruct(Beatmap beatmap)
        {
            var hitObjects = beatmap.HitObjects.Select(ho => new CHitObject
            {
                col = ho is ManiaHitObject mania ? mania.Column : 0,
                start_time = (int)ho.StartTime,
                end_time = (int)ho.EndTime
            }).ToArray();

            IntPtr hitObjectsPtr = Marshal.AllocHGlobal(Marshal.SizeOf<CHitObject>() * hitObjects.Length);
            for (int i = 0; i < hitObjects.Length; i++)
            {
                Marshal.StructureToPtr(hitObjects[i], hitObjectsPtr + i * Marshal.SizeOf<CHitObject>(), false);
            }

            return new CBeatmapData
            {
                overall_difficulty = beatmap.BeatmapInfo.Difficulty.OverallDifficulty,
                circle_size = beatmap.BeatmapInfo.Difficulty.CircleSize,
                hit_objects_count = (UIntPtr)hitObjects.Length,
                hit_objects_ptr = hitObjectsPtr
            };
        }

        #endregion
    }
}
