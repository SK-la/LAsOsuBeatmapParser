using System;
using System.IO;
using System.Linq;
using System.Text;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Beatmaps.Formats;

namespace LAsOsuBeatmapParser.Tests
{
    public class TestResourceLoader
    {
        static string ResourcePath = Path.Combine("Resource");

        public static void Run(string[] args)
        {
            while (true)
            {
                Console.WriteLine($"\n列出 {ResourcePath} 下的内容:");
                var entries = Directory.GetFileSystemEntries(ResourcePath);
                for (int i = 0; i < entries.Length; i++)
                {
                    var name = Path.GetFileName(entries[i]);
                    var type = Directory.Exists(entries[i]) ? "[文件夹]" : "[文件]";
                    Console.WriteLine($"{i + 1}. {name} {type}");
                }
                Console.WriteLine("0. 退出");
                Console.Write("请输入数字选择: ");
                if (!int.TryParse(Console.ReadLine(), out int choice) || choice < 0 || choice > entries.Length)
                {
                    Console.WriteLine("输入无效，请重试。");
                    continue;
                }
                if (choice == 0) break;
                var selected = entries[choice - 1];
                if (Directory.Exists(selected))
                {
                    HandleDirectory(selected);
                }
                else if (selected.EndsWith(".osu", StringComparison.OrdinalIgnoreCase))
                {
                    ParseAndPrintOsu(selected);
                }
                else
                {
                    Console.WriteLine("不是osu谱面文件，无法解析。");
                }
            }
        }

        static void HandleDirectory(string dir)
        {
            var osuFiles = Directory.GetFiles(dir, "*.osu");
            if (osuFiles.Length == 0)
            {
                Console.WriteLine("该文件夹下没有osu谱面文件。");
                return;
            }
            Console.WriteLine($"\n列出 {dir} 下的osu文件:");
            for (int i = 0; i < osuFiles.Length; i++)
            {
                Console.WriteLine($"{i + 1}. {Path.GetFileName(osuFiles[i])}");
            }
            Console.WriteLine("0. 返回上级");
            Console.Write("请输入数字选择: ");
            if (!int.TryParse(Console.ReadLine(), out int choice) || choice < 0 || choice > osuFiles.Length)
            {
                Console.WriteLine("输入无效。");
                return;
            }
            if (choice == 0) return;
            ParseAndPrintOsu(osuFiles[choice - 1]);
        }

        static void ParseAndPrintOsu(string osuPath)
        {
            try
            {
                using var stream = File.OpenRead(osuPath);
                var decoder = new LegacyBeatmapDecoder();
                var beatmap = decoder.Decode(stream);
                Console.WriteLine($"\n解析结果: {Path.GetFileName(osuPath)}");
                Console.WriteLine($"Mode: {beatmap.Mode}");
                Console.WriteLine($"Title: {beatmap.Metadata?.Title}");
                Console.WriteLine($"Artist: {beatmap.Metadata?.Artist}");
                Console.WriteLine($"Creator: {beatmap.Metadata?.Creator}");
                Console.WriteLine($"Version: {beatmap.Metadata?.Version}");
                Console.WriteLine($"CircleSize: {beatmap.Difficulty?.CircleSize}");
                Console.WriteLine($"HPDrainRate: {beatmap.Difficulty?.HPDrainRate}");
                Console.WriteLine($"OverallDifficulty: {beatmap.Difficulty?.OverallDifficulty}");
                Console.WriteLine($"ApproachRate: {beatmap.Difficulty?.ApproachRate}");
                Console.WriteLine($"BPM: {beatmap.BPM}");
                Console.WriteLine($"HitObject数: {beatmap.HitObjects?.Count}");
            }
            catch (Exception ex)
            {
                Console.WriteLine($"解析失败: {ex.Message}");
            }
        }
    }
}
