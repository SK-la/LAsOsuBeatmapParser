using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using LAsOsuBeatmapParser.Analysis;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Beatmaps.Formats;
using Xunit;
using Xunit.Abstractions;

namespace LAsOsuBeatmapParser.Tests
{
    public class SRCalculatorComparisonTests
    {
        private readonly ITestOutputHelper _output;

        // SR显示精度控制常量
        private const int SR_DECIMAL_PLACES = 4;

        public SRCalculatorComparisonTests(ITestOutputHelper output)
        {
            _output = output;
        }

        [Fact]
        public void CompareDirectSRCalculationVsExtensionMethodPerformance()
        {
            // 加载测试谱面
            string resourcePath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", "..", "Resource");
            string[] osuFiles = Directory.GetFiles(resourcePath, "*.osu")
                                         .Where(f => !f.Contains("encoded_output"))
                                         .OrderBy(f => Path.GetFileName(f))
                                         .ToArray();

            if (osuFiles.Length < 1)
            {
                _output.WriteLine("⚠️  需要至少1个测试谱面文件，跳过性能对比测试");
                return;
            }

            // 选择所有可用谱面进行测试
            string[] testFiles = osuFiles.ToArray();

            _output.WriteLine($"=== SR计算性能对比测试 ===");
            _output.WriteLine($"测试谱面数量: {testFiles.Length} 个");
            _output.WriteLine("");

            const int iterations = 5; // 每个谱面测试多次取平均值

            // 预热JIT编译
            _output.WriteLine("🔥 预热JIT编译...");
            foreach (string file in testFiles)
            {
                var beatmap = new LegacyBeatmapDecoder().Decode(file);
                SRCalculator.Instance.CalculateSR(beatmap, out _);
                beatmap.GetStatistics().First().SR = SRCalculator.Instance.CalculateSR(beatmap, out _);
            }
            _output.WriteLine("✅ 预热完成");
            _output.WriteLine("");

            var directTimes = new List<long>();
            var extensionTimes = new List<long>();
            var directSRs = new List<double>();
            var extensionSRs = new List<double>();

            foreach (string file in testFiles)
            {
                var beatmap = new LegacyBeatmapDecoder().Decode(file);

                // 测试直接调用
                long directTime = 0;
                double directSR = 0;
                for (int i = 0; i < iterations; i++)
                {
                    var stopwatch = Stopwatch.StartNew();
                    directSR = SRCalculator.Instance.CalculateSR(beatmap, out _);
                    stopwatch.Stop();
                    directTime += stopwatch.ElapsedMilliseconds;
                }
                directTimes.Add(directTime / iterations);
                directSRs.Add(directSR);

                // 测试扩展方法
                long extensionTime = 0;
                double extensionSR = 0;
                for (int i = 0; i < iterations; i++)
                {
                    var stopwatch = Stopwatch.StartNew();
                    extensionSR = beatmap.GetStatistics().First().SR;
                    stopwatch.Stop();
                    extensionTime += stopwatch.ElapsedMilliseconds;
                }
                extensionTimes.Add(extensionTime / iterations);
                extensionSRs.Add(extensionSR);
            }

            // 输出结果表格
            _output.WriteLine("性能对比结果:");
            _output.WriteLine("文件名".PadRight(50) + " | " + "直接调用(ms)".PadRight(12) + " | " + "扩展方法(ms)".PadRight(12) + " | " + "直接SR".PadRight(8) + " | " + "扩展SR".PadRight(8));
            _output.WriteLine(new string('-', 110));

            for (int i = 0; i < testFiles.Length; i++)
            {
                string fileName = Path.GetFileName(testFiles[i]);
                if (fileName.Length > 47) fileName = fileName.Substring(0, 44) + "...";

                _output.WriteLine($"{fileName.PadRight(50)} | {directTimes[i].ToString().PadRight(12)} | {extensionTimes[i].ToString().PadRight(12)} | {directSRs[i]:F4} | {extensionSRs[i]:F4}");
            }

            // 计算统计
            double avgDirectTime = directTimes.Average();
            double avgExtensionTime = extensionTimes.Average();
            double timeDifference = avgExtensionTime - avgDirectTime;
            double timePercentChange = (timeDifference / avgDirectTime) * 100;

            _output.WriteLine("");
            _output.WriteLine($"平均直接调用时间: {avgDirectTime:F2}ms");
            _output.WriteLine($"平均扩展方法时间: {avgExtensionTime:F2}ms");
            _output.WriteLine($"时间差异: {timeDifference:F2}ms ({timePercentChange:+0.00;-0.00}%)");

            if (timePercentChange > 50)
            {
                _output.WriteLine("⚠️  扩展方法性能显著下降，可能存在缓存或计算问题");
            }
            else if (timePercentChange < -10)
            {
                _output.WriteLine("✅ 扩展方法性能良好");
            }
            else
            {
                _output.WriteLine("ℹ️  扩展方法性能与直接调用相当");
            }

            // 验证SR值一致性
            bool allEqual = true;
            for (int i = 0; i < directSRs.Count; i++)
            {
                double diff = Math.Abs(directSRs[i] - extensionSRs[i]);
                if (diff > Math.Pow(10, -SR_DECIMAL_PLACES))
                {
                    allEqual = false;
                    _output.WriteLine($"❌ SR值不一致: {Path.GetFileName(testFiles[i])} - 直接: {directSRs[i]:F4}, 扩展: {extensionSRs[i]:F4}, 差异: {diff:F8}");
                }
            }

            if (allEqual)
            {
                _output.WriteLine("✅ SR值一致性验证通过");
            }

            _output.WriteLine("✅ 性能对比测试完成");
        }

        [Fact]
        public void CompareCSRVsRustSR()
        {
            // 加载测试谱面
            string resourcePath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", "..", "Resource");
            string[] osuFiles = Directory.GetFiles(resourcePath, "*.osu")
                                         .Where(f => !f.Contains("encoded_output"))
                                         .OrderBy(f => Path.GetFileName(f))
                                         .ToArray();

            if (osuFiles.Length < 1)
            {
                _output.WriteLine("⚠️  需要至少1个测试谱面文件，跳过C# vs Rust对比测试");
                return;
            }

            // 选择所有可用谱面进行测试
            string[] testFiles = osuFiles.ToArray();

            _output.WriteLine($"=== C# vs Rust SR计算对比测试 ===");
            _output.WriteLine($"测试谱面数量: {testFiles.Length} 个");
            _output.WriteLine("");

            const int iterations = 5; // 每个谱面测试多次取平均值

            // 预热JIT编译
            _output.WriteLine("🔥 预热JIT编译...");
            foreach (string file in testFiles)
            {
                var beatmap = new LegacyBeatmapDecoder().Decode(file);
                SRCalculator.Instance.CalculateSR(beatmap, out _);
                SRCalculator.Instance.CalculateSRRust(beatmap);
                SRCalculator.Instance.CalculateSRFromFile(file);
                string content = File.ReadAllText(file);
                SRCalculator.Instance.CalculateSRFromContent(content);
            }
            _output.WriteLine("✅ 预热完成");
            _output.WriteLine("");

            var csTimes = new List<long>();
            var rustTimes = new List<long>();
            var fileTimes = new List<long>();
            var contentTimes = new List<long>();
            var csMemoryDeltas = new List<long>();
            var rustMemoryDeltas = new List<long>();
            var fileMemoryDeltas = new List<long>();
            var contentMemoryDeltas = new List<long>();
            var csSRs = new List<double>();
            var rustSRs = new List<double>();
            var fileSRs = new List<double>();
            var contentSRs = new List<double>();
            var circleSizes = new List<float>();

            foreach (string file in testFiles)
            {
                var beatmap = new LegacyBeatmapDecoder().Decode(file);
                circleSizes.Add(beatmap.Difficulty.CircleSize);
                string fileContent = File.ReadAllText(file);

                // 测试C# SR计算
                long csTime = 0;
                double csSR = 0;
                long csMemoryBefore = 0;
                long csMemoryAfter = 0;
                for (int i = 0; i < iterations; i++)
                {
                    GC.Collect();
                    GC.WaitForPendingFinalizers();
                    csMemoryBefore = GC.GetTotalMemory(false);

                    var stopwatch = Stopwatch.StartNew();
                    csSR = SRCalculator.Instance.CalculateSR(beatmap, out _);
                    stopwatch.Stop();

                    csMemoryAfter = GC.GetTotalMemory(false);
                    csTime += stopwatch.ElapsedMilliseconds;
                }
                csTimes.Add(csTime / iterations);
                csMemoryDeltas.Add((csMemoryAfter - csMemoryBefore) / iterations);
                csSRs.Add(csSR);

                // 测试Rust SR计算
                long rustTime = 0;
                double rustSR = 0;
                long rustMemoryBefore = 0;
                long rustMemoryAfter = 0;
                for (int i = 0; i < iterations; i++)
                {
                    GC.Collect();
                    GC.WaitForPendingFinalizers();
                    rustMemoryBefore = GC.GetTotalMemory(false);

                    var stopwatch = Stopwatch.StartNew();
                    rustSR = SRCalculator.Instance.CalculateSRRust(beatmap);
                    stopwatch.Stop();

                    rustMemoryAfter = GC.GetTotalMemory(false);
                    rustTime += stopwatch.ElapsedMilliseconds;
                }
                rustTimes.Add(rustTime / iterations);
                rustMemoryDeltas.Add((rustMemoryAfter - rustMemoryBefore) / iterations);
                rustSRs.Add(rustSR);

                // 测试基于文件的SR计算
                long fileTime = 0;
                double fileSR = 0;
                long fileMemoryBefore = 0;
                long fileMemoryAfter = 0;
                for (int i = 0; i < iterations; i++)
                {
                    GC.Collect();
                    GC.WaitForPendingFinalizers();
                    fileMemoryBefore = GC.GetTotalMemory(false);

                    var stopwatch = Stopwatch.StartNew();
                    fileSR = SRCalculator.Instance.CalculateSRFromFile(file);
                    stopwatch.Stop();

                    fileMemoryAfter = GC.GetTotalMemory(false);
                    fileTime += stopwatch.ElapsedMilliseconds;
                }
                fileTimes.Add(fileTime / iterations);
                fileMemoryDeltas.Add((fileMemoryAfter - fileMemoryBefore) / iterations);
                fileSRs.Add(fileSR);

                // 测试基于内容的SR计算
                long contentTime = 0;
                double contentSR = 0;
                long contentMemoryBefore = 0;
                long contentMemoryAfter = 0;
                for (int i = 0; i < iterations; i++)
                {
                    GC.Collect();
                    GC.WaitForPendingFinalizers();
                    contentMemoryBefore = GC.GetTotalMemory(false);

                    var stopwatch = Stopwatch.StartNew();
                    contentSR = SRCalculator.Instance.CalculateSRFromContent(fileContent);
                    stopwatch.Stop();

                    contentMemoryAfter = GC.GetTotalMemory(false);
                    contentTime += stopwatch.ElapsedMilliseconds;
                }
                contentTimes.Add(contentTime / iterations);
                contentMemoryDeltas.Add((contentMemoryAfter - contentMemoryBefore) / iterations);
                contentSRs.Add(contentSR);
            }

            // 输出结果表格
            _output.WriteLine("SR计算方法对比结果:");
            _output.WriteLine("CS".PadRight(5) + " | " + "C#时间(ms)".PadRight(10) + " | " + "Rust时间(ms)".PadRight(11) + " | " + "文件时间(ms)".PadRight(12) + " | " + "内容时间(ms)".PadRight(12) + " | " + "C#内存(MB)".PadRight(10) + " | " + "Rust内存(MB)".PadRight(11) + " | " + "文件内存(MB)".PadRight(12) + " | " + "内容内存(MB)".PadRight(12) + " | " + "C# SR".PadRight(8) + " | " + "Rust SR".PadRight(8) + " | " + "文件 SR".PadRight(9) + " | " + "内容 SR".PadRight(9));
            _output.WriteLine(new string('-', 160));

            for (int i = 0; i < testFiles.Length; i++)
            {
                double csMemoryMB = csMemoryDeltas[i] / 1000000.0;
                double rustMemoryMB = rustMemoryDeltas[i] / 1000000.0;
                double fileMemoryMB = fileMemoryDeltas[i] / 1000000.0;
                double contentMemoryMB = contentMemoryDeltas[i] / 1000000.0;
                _output.WriteLine($"{circleSizes[i]:F1}".PadRight(5) + " | " +
                                $"{csTimes[i].ToString().PadRight(10)} | " +
                                $"{rustTimes[i].ToString().PadRight(11)} | " +
                                $"{fileTimes[i].ToString().PadRight(12)} | " +
                                $"{contentTimes[i].ToString().PadRight(12)} | " +
                                $"{csMemoryMB:F2}".PadRight(10) + " | " +
                                $"{rustMemoryMB:F2}".PadRight(11) + " | " +
                                $"{fileMemoryMB:F2}".PadRight(12) + " | " +
                                $"{contentMemoryMB:F2}".PadRight(12) + " | " +
                                $"{csSRs[i]:F4} | " +
                                $"{rustSRs[i]:F4} | " +
                                $"{fileSRs[i]:F4} | " +
                                $"{contentSRs[i]:F4}");
            }

            // 计算统计
            double avgCsTime = csTimes.Average();
            double avgRustTime = rustTimes.Average();
            double timeDifference = avgRustTime - avgCsTime;
            double timePercentChange = (timeDifference / avgCsTime) * 100;

            double avgCsMemory = csMemoryDeltas.Average();
            double avgRustMemory = rustMemoryDeltas.Average();
            double memoryDifference = avgRustMemory - avgCsMemory;
            double memoryPercentChange = (memoryDifference / avgCsMemory) * 100;

            _output.WriteLine("");
            _output.WriteLine($"平均C#计算时间: {avgCsTime:F2}ms");
            _output.WriteLine($"平均Rust计算时间: {avgRustTime:F2}ms");
            _output.WriteLine($"时间差异: {timeDifference:F2}ms ({timePercentChange:+0.00;-0.00}%)");

            _output.WriteLine($"平均C#内存增量: {avgCsMemory / 1000000.0:F2} MB");
            _output.WriteLine($"平均Rust内存增量: {avgRustMemory / 1000000.0:F2} MB");
            _output.WriteLine($"内存差异: {memoryDifference / 1000000.0:F2} MB ({memoryPercentChange:+0.00;-0.00}%)");

            if (timePercentChange < -10)
            {
                _output.WriteLine("✅ Rust SR计算性能优于C#");
            }
            else if (timePercentChange > 50)
            {
                _output.WriteLine("⚠️  Rust SR计算性能显著下降");
            }
            else
            {
                _output.WriteLine("ℹ️  Rust SR计算性能与C#相当");
            }

            // 验证SR值一致性（仅报告，不断言）
            const double tolerance = 0.0001;
            bool allWithinTolerance = true;
            for (int i = 0; i < csSRs.Count; i++)
            {
                double diff = Math.Abs(csSRs[i] - rustSRs[i]);
                if (diff >= tolerance)
                {
                    allWithinTolerance = false;
                    _output.WriteLine($"ℹ️  SR值差异 (CS={circleSizes[i]:F1}): C#: {csSRs[i]:F4}, Rust: {rustSRs[i]:F4}, 差异: {diff:F8}");
                }
            }

            if (allWithinTolerance)
            {
                _output.WriteLine("✅ C# vs Rust SR值一致性验证通过");
            }
            else
            {
                _output.WriteLine("ℹ️  C# vs Rust SR值存在差异（这是预期的，用于对比分析）");
            }
        }

        [Fact]
        public void CompareRustSRMethods()
        {
            // 加载测试谱面
            string resourcePath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", "..", "Resource");
            string[] osuFiles = Directory.GetFiles(resourcePath, "*.osu")
                                         .Where(f => !f.Contains("encoded_output"))
                                         .OrderBy(f => Path.GetFileName(f))
                                         .ToArray();

            if (osuFiles.Length < 1)
            {
                _output.WriteLine("⚠️  需要至少1个测试谱面文件，跳过Rust方法对比测试");
                return;
            }

            // 按CS分组测试谱面
            var groupedByCS = osuFiles
                .Select(file =>
                {
                    var beatmap = new LegacyBeatmapDecoder().Decode(file);
                    return new { File = file, CS = beatmap.Difficulty.CircleSize, Beatmap = beatmap };
                })
                .GroupBy(x => x.CS)
                .OrderBy(g => g.Key)
                .ToList();

            _output.WriteLine($"=== Rust SR计算方法对比测试 ===");
            _output.WriteLine($"测试谱面数量: {osuFiles.Length} 个，CS分组数: {groupedByCS.Count} 个");
            _output.WriteLine("");

            const int iterations = 5; // 每个谱面测试多次取平均值

            // 预热JIT编译
            _output.WriteLine("🔥 预热JIT编译...");
            foreach (var group in groupedByCS)
            {
                foreach (var item in group)
                {
                    SRCalculator.Instance.CalculateSRRust(item.Beatmap);
                    try
                    {
                        SRCalculator.Instance.CalculateSRFromFile(item.File);
                        string content = File.ReadAllText(item.File);
                        SRCalculator.Instance.CalculateSRFromContent(content);
                    }
                    catch (Exception)
                    {
                        // 忽略未实现的函数
                    }
                }
            }
            _output.WriteLine("✅ 预热完成");
            _output.WriteLine("");

            // 输出结果表格
            _output.WriteLine("Rust SR计算方法对比结果:");
            _output.WriteLine("CS".PadRight(5) + " | " + "对象时间(ms)".PadRight(11) + " | " + "文件时间(ms)".PadRight(11) + " | " + "内容时间(ms)".PadRight(11) + " | " + "对象内存(MB)".PadRight(11) + " | " + "文件内存(MB)".PadRight(11) + " | " + "内容内存(MB)".PadRight(11) + " | " + "对象SR".PadRight(8) + " | " + "文件SR".PadRight(8) + " | " + "内容SR".PadRight(8));
            _output.WriteLine(new string('-', 120));

            foreach (var group in groupedByCS)
            {
                float cs = group.Key;
                var items = group.ToList();

                // 为每个CS计算平均值
                var objectTimes = new List<long>();
                var fileTimes = new List<long>();
                var contentTimes = new List<long>();
                var objectMemories = new List<long>();
                var fileMemories = new List<long>();
                var contentMemories = new List<long>();
                var objectSRs = new List<double>();
                var fileSRs = new List<double>();
                var contentSRs = new List<double>();

                foreach (var item in items)
                {
                    string fileContent = File.ReadAllText(item.File);

                    // 测试对象方法
                    long objTime = 0, objMemBefore = 0, objMemAfter = 0;
                    double objSR = 0;
                    for (int i = 0; i < iterations; i++)
                    {
                        GC.Collect(); GC.WaitForPendingFinalizers();
                        objMemBefore = GC.GetTotalMemory(false);
                        var sw = Stopwatch.StartNew();
                        objSR = SRCalculator.Instance.CalculateSRRust(item.Beatmap);
                        sw.Stop();
                        objMemAfter = GC.GetTotalMemory(false);
                        objTime += sw.ElapsedMilliseconds;
                    }
                    objectTimes.Add(objTime / iterations);
                    objectMemories.Add((objMemAfter - objMemBefore) / iterations);
                    objectSRs.Add(objSR);

                    // 测试文件方法
                    long fileTime = 0, fileMemBefore = 0, fileMemAfter = 0;
                    double fileSR = 0;
                    try
                    {
                        for (int i = 0; i < iterations; i++)
                        {
                            GC.Collect(); GC.WaitForPendingFinalizers();
                            fileMemBefore = GC.GetTotalMemory(false);
                            var sw = Stopwatch.StartNew();
                            fileSR = SRCalculator.Instance.CalculateSRFromFile(item.File);
                            sw.Stop();
                            fileMemAfter = GC.GetTotalMemory(false);
                            fileTime += sw.ElapsedMilliseconds;
                        }
                        fileTimes.Add(fileTime / iterations);
                        fileMemories.Add((fileMemAfter - fileMemBefore) / iterations);
                        fileSRs.Add(fileSR);
                    }
                    catch (Exception)
                    {
                        // 函数未实现，跳过
                        fileTimes.Add(-1);
                        fileMemories.Add(0);
                        fileSRs.Add(-1);
                    }

                    // 测试内容方法
                    long contentTime = 0, contentMemBefore = 0, contentMemAfter = 0;
                    double contentSR = 0;
                    try
                    {
                        for (int i = 0; i < iterations; i++)
                        {
                            GC.Collect(); GC.WaitForPendingFinalizers();
                            contentMemBefore = GC.GetTotalMemory(false);
                            var sw = Stopwatch.StartNew();
                            contentSR = SRCalculator.Instance.CalculateSRFromContent(fileContent);
                            sw.Stop();
                            contentMemAfter = GC.GetTotalMemory(false);
                            contentTime += sw.ElapsedMilliseconds;
                        }
                        contentTimes.Add(contentTime / iterations);
                        contentMemories.Add((contentMemAfter - contentMemBefore) / iterations);
                        contentSRs.Add(contentSR);
                    }
                    catch (Exception)
                    {
                        // 函数未实现，跳过
                        contentTimes.Add(-1);
                        contentMemories.Add(0);
                        contentSRs.Add(-1);
                    }
                }

                // 计算该CS的平均值
                double avgObjTime = objectTimes.Average();
                double avgFileTime = fileTimes.Where(t => t >= 0).DefaultIfEmpty(-1).Average();
                double avgContentTime = contentTimes.Where(t => t >= 0).DefaultIfEmpty(-1).Average();
                double avgObjMem = objectMemories.Average() / 1000000.0;
                double avgFileMem = fileMemories.Where(m => m >= 0).DefaultIfEmpty(0).Average() / 1000000.0;
                double avgContentMem = contentMemories.Where(m => m >= 0).DefaultIfEmpty(0).Average() / 1000000.0;
                double avgObjSR = objectSRs.Average();
                double avgFileSR = fileSRs.Where(s => s >= 0).DefaultIfEmpty(-1).Average();
                double avgContentSR = contentSRs.Where(s => s >= 0).DefaultIfEmpty(-1).Average();

                string fileTimeStr = avgFileTime >= 0 ? $"{avgFileTime:F1}" : "N/A";
                string contentTimeStr = avgContentTime >= 0 ? $"{avgContentTime:F1}" : "N/A";
                string fileMemStr = avgFileMem >= 0 ? $"{avgFileMem:F3}" : "N/A";
                string contentMemStr = avgContentMem >= 0 ? $"{avgContentMem:F3}" : "N/A";
                string fileSRStr = avgFileSR >= 0 ? $"{avgFileSR:F4}" : "N/A";
                string contentSRStr = avgContentSR >= 0 ? $"{avgContentSR:F4}" : "N/A";

                _output.WriteLine($"{cs:F1}".PadRight(5) + " | " +
                                $"{avgObjTime:F1}".PadRight(11) + " | " +
                                $"{fileTimeStr}".PadRight(11) + " | " +
                                $"{contentTimeStr}".PadRight(11) + " | " +
                                $"{avgObjMem:F3}".PadRight(11) + " | " +
                                $"{fileMemStr}".PadRight(11) + " | " +
                                $"{contentMemStr}".PadRight(11) + " | " +
                                $"{avgObjSR:F4} | " +
                                $"{fileSRStr} | " +
                                $"{contentSRStr}");
            }

            _output.WriteLine("");
            _output.WriteLine("✅ Rust SR方法对比测试完成");
        }
    }
}
