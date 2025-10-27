// Copyright (c) ppy Pty Ltd <contact@ppy.sh>. Licensed under the MIT Licence.
// See the LICENCE file in the repository root for full licence text.

using System;
using System.Diagnostics;
using System.IO;
using System.Text.RegularExpressions;
using LAsOsuBeatmapParser.Beatmaps;

namespace LAsOsuBeatmapParser.Analysis
{
    public static class SRCalculatorPython
    {
        // Python脚本路径 - 相对于项目根目录
        private static readonly string PythonScriptPath = Path.Combine(
            AppDomain.CurrentDomain.BaseDirectory,
            "..", "..", "..", "..", "..", "Star-Rating-Rebirth", "srcalc-script.py"
        );

        // Python可执行文件路径 - 使用系统PATH中的python
        private static readonly string PythonExecutable = "python";

        public static double? CalculateSR_FromFile(string filePath)
        {
            Console.WriteLine("Starting Python SR calculation");
            if (string.IsNullOrEmpty(filePath) || !File.Exists(filePath))
            {
                throw new Exception($"File not found: {filePath}");
            }

            try
            {
                // 检查Python脚本是否存在
                if (!File.Exists(PythonScriptPath))
                {
                    throw new Exception($"Python脚本不存在: {PythonScriptPath}");
                }

                Console.WriteLine($"Python script exists: {PythonScriptPath}");

                // 创建临时目录
                string tempDir = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString());
                Directory.CreateDirectory(tempDir);

                try
                {
                    // 将.osu文件复制到临时目录
                    string fileName = Path.GetFileName(filePath);
                    string tempFilePath = Path.Combine(tempDir, fileName);
                    File.Copy(filePath, tempFilePath);

                    // 创建进程启动信息
                    var startInfo = new ProcessStartInfo
                    {
                        FileName = PythonExecutable,
                        Arguments = $"\"{PythonScriptPath}\" \"{tempDir}\" --single-run",
                        UseShellExecute = false,
                        RedirectStandardOutput = true,
                        RedirectStandardError = true,
                        CreateNoWindow = true,
                        WorkingDirectory = Path.GetDirectoryName(PythonScriptPath) ?? ""
                    };

                    // 启动Python进程
                    using var process = Process.Start(startInfo);
                    if (process == null)
                    {
                        Console.WriteLine("无法启动Python进程");
                        return null;
                    }

                    // 读取输出
                    string output = process.StandardOutput.ReadToEnd();
                    string error = process.StandardError.ReadToEnd();

                    // 等待进程完成或超时
                    bool exited = process.WaitForExit(10000); // 10秒超时
                    if (!exited)
                    {
                        Console.WriteLine("Python进程超时，强制终止");
                        process.Kill();
                        return null;
                    }

                    // 检查退出码
                    if (process.ExitCode != 0)
                    {
                        Console.WriteLine($"Python进程以错误码退出: {process.ExitCode}");
                        Console.WriteLine($"Stdout: {output}");
                        Console.WriteLine($"Stderr: {error}");
                        return null;
                    }

                    // 解析输出中的SR值
                    // 输出格式: "(NM) filename | 6.1037"
                    var match = Regex.Match(output, @"\([A-Z]+\)\s+.*?\s+\|\s+([0-9]+\.?[0-9]*)");
                    if (match.Success && double.TryParse(match.Groups[1].Value, out double sr))
                    {
                        return sr;
                    }

                    Console.WriteLine($"无法解析SR值，输出: {output}");
                    throw new Exception($"无法解析SR值，输出: {output}");
                }
                finally
                {
                    // 清理临时目录
                    try
                    {
                        Directory.Delete(tempDir, true);
                    }
                    catch
                    {
                        // 忽略清理错误
                    }
                }
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Python SR计算错误: {ex}");
                throw; // 重新抛出异常，让测试显示
            }
        }
    }
}