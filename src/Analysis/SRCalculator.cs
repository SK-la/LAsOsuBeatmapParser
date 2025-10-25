using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Threading.Tasks;
using LAsOsuBeatmapParser.Beatmaps;
using System.Runtime.InteropServices;
using System.Text.Json;
using LAsOsuBeatmapParser.Extensions;

namespace LAsOsuBeatmapParser.Analysis
{
    /// <summary>
    /// SR计算器，用于计算谱面的xxy SR值
    /// <para></para>
    /// From: https://github.com/sunnyxxy/Star-Rating-Rebirth
    /// </summary>
    public class SRCalculator
    {
        /// <summary>
        /// 单例模式：无状态类，线程安全
        /// </summary>
        public static SRCalculator Instance { get; } = new SRCalculator();

        private SRCalculator() { } // 私有构造函数

        private const double lambda_n = 5;
        private const double lambda_1 = 0.11;
        private const double lambda_3 = 24;
        private const double lambda_2 = 7.0;
        private const double lambda_4 = 0.1;
        private const double w_0 = 0.4;
        private const double w_1 = 2.7;
        private const double p_1 = 1.5;
        private const double w_2 = 0.27;
        private const double p_0 = 1.0;

        private const int granularity = 1; // 只能保持为1，确保精度不变，不可修改

        /// <summary>
        /// 异步SR计算核心，已并行化所有section
        /// </summary>
        /// <param name="beatmap"></param>
        /// <param name="times"></param>
        /// <typeparam name="T"></typeparam>
        /// <returns></returns>
        public double CalculateSR<T>(IBeatmap<T> beatmap, out Dictionary<string, long> times) where T : HitObject
        {
            Task<(double sr, Dictionary<string, long> times)> task = CalculateSRAsync(beatmap);
            (double sr, Dictionary<string, long> t) = task.Result; // 同步等待（用于兼容旧接口）
            times = t;
            return sr;
        }

        /// <summary>
        /// 异步SR计算核心，已并行化所有section
        /// </summary>
        /// <param name="beatmap"></param>
        /// <typeparam name="T"></typeparam>
        /// <returns></returns>
        public async Task<(double sr, Dictionary<string, long> times)> CalculateSRAsync<T>(IBeatmap<T> beatmap) where T : HitObject
        {
            double od = beatmap.BeatmapInfo.Difficulty.OverallDifficulty;
            int keyCount = (int)beatmap.BeatmapInfo.Difficulty.CircleSize;
            var times = new Dictionary<string, long>();

            // Check if key count is supported (max 18 keys, even numbers only for K>10)
            if (keyCount > 18 || keyCount < 1 || (keyCount > 10 && keyCount % 2 == 1)) return (-1, times); // Return invalid SR

            try
            {
                var totalStopwatch = Stopwatch.StartNew(); // 总时间计时开始
                var noteSequence = new List<SRsNote>();

                foreach (T hitObject in beatmap.HitObjects)
                {
                    int col = hitObject is ManiaHitObject maniaHit ? maniaHit.Column : ManiaExtensions.GetColumnFromX(keyCount, hitObject.Position.X);
                    int time = (int)hitObject.StartTime;
                    int tail = hitObject.EndTime > hitObject.StartTime ? (int)hitObject.EndTime : -1;
                    noteSequence.Add(new SRsNote(col, time, tail));
                }

                // 优化：避免多次LINQ排序，使用Array.Sort，并使用Span优化
                SRsNote[] noteSeq = noteSequence.ToArray();

                // Handle empty note sequence
                if (noteSeq.Length == 0)
                {
                    totalStopwatch.Stop();
                    return (0, times);
                }

                Array.Sort(noteSeq, new NoteComparer());

                double x = 0.3 * Math.Sqrt((64.5 - Math.Ceiling(od * 3)) / 500);
                x = Math.Min(x, 0.6 * (x - 0.09) + 0.09);
                SRsNote[][] noteSeqByColumn = noteSeq.GroupBy(n => n.Index).OrderBy(g => g.Key).Select(g => g.ToArray()).ToArray();

                // 优化：预计算LN序列长度，避免Where().ToArray()
                int lnCount = 0;

                foreach (SRsNote note in noteSeq)
                {
                    if (note.EndTime >= 0)
                        lnCount++;
                }

                var LNSeq = new SRsNote[lnCount];
                int lnIndex = 0;

                foreach (SRsNote note in noteSeq)
                {
                    if (note.EndTime >= 0)
                        LNSeq[lnIndex++] = note;
                }

                // 优化：直接排序LNSeq而不是创建新数组
                Array.Sort(LNSeq, new NoteComparerByT());
                SRsNote[] tailSeq = LNSeq;

                var LNDict = new Dictionary<int, List<SRsNote>>();

                foreach (SRsNote note in LNSeq)
                {
                    if (!LNDict.ContainsKey(note.Index))
                        LNDict[note.Index] = new List<SRsNote>();
                    LNDict[note.Index].Add(note);
                }

                // Calculate T
                int totalTime = Math.Max(noteSeq.Max(n => n.StartTime), noteSeq.Max(n => n.EndTime)) + 1;

                var stopwatch = new Stopwatch();

                // Start all sections in parallel
                stopwatch.Start();

                // Define tasks for each section
                // 优化：使用并行任务加速Section 23/24/25的计算
                Task<(double[] jBar, double[][] deltaKsResult)> task23 = Task.Run(() =>
                {
                    var sectionStopwatch = Stopwatch.StartNew();
                    (double[] jBar, double[][] deltaKsResult) = CalculateSection23(keyCount, noteSeqByColumn, totalTime, x);
                    sectionStopwatch.Stop();
                    return (jBar, deltaKsResult);
                });

                Task<double[]> task24 = Task.Run(() =>
                {
                    var sectionStopwatch = Stopwatch.StartNew();
                    double[] XBar = CalculateSection24(keyCount, totalTime, noteSeqByColumn, x);
                    sectionStopwatch.Stop();
                    return XBar;
                });

                Task<double[]> task25 = Task.Run(() =>
                {
                    var sectionStopwatch = Stopwatch.StartNew();
                    double[] PBar = CalculateSection25(totalTime, LNSeq, noteSeq, x);
                    sectionStopwatch.Stop();
                    return PBar;
                });

                // Wait for all tasks to complete
                await Task.WhenAll(task23, task24, task25).ConfigureAwait(false);

                // Retrieve results
                (double[] JBar, double[][] deltaKs) = task23.Result;
                double[] XBar = task24.Result;
                double[] PBar = task25.Result;

                stopwatch.Stop();
                // Logger.WriteLine(LogLevel.Debug, $"[SRCalculator]Section 23/24/25 Time: {stopwatch.ElapsedMilliseconds}ms");
                times["Section232425"] = stopwatch.ElapsedMilliseconds;

                stopwatch.Restart();
                Task<(double[] ABar, int[] KS)> task26 = Task.Run(() => CalculateSection26(deltaKs, keyCount, totalTime, noteSeq));
                Task<(double[] RBar, double[] Is)> task27 = Task.Run(() => CalculateSection27(LNSeq, tailSeq, totalTime, noteSeqByColumn, x));

                // Wait for both tasks to complete
                await Task.WhenAll(task26, task27).ConfigureAwait(false);

                // Retrieve results
                (double[] ABar, int[] KS) = task26.Result;
                (double[] RBar, _) = task27.Result;

                stopwatch.Stop();
                // Logger.WriteLine(LogLevel.Debug, $"[SRCalculator]Section 26/27 Time: {stopwatch.ElapsedMilliseconds}ms");
                times["Section2627"] = stopwatch.ElapsedMilliseconds;

                // Final calculation
                stopwatch.Restart();
                double result = CalculateSection3(JBar, XBar, PBar, ABar, RBar, KS, totalTime, noteSeq, LNSeq, keyCount);
                stopwatch.Stop();
                // Logger.WriteLine(LogLevel.Debug, $"[SRCalculator]Section 3 Time: {stopwatch.ElapsedMilliseconds}ms");
                times["Section3"] = stopwatch.ElapsedMilliseconds;

                totalStopwatch.Stop(); // 总时间计时结束
                // Logger.WriteLine(LogLevel.Debug, $"[SRCalculator]Total Calculate Time: {totalStopwatch.ElapsedMilliseconds}ms");
                times["Total"] = totalStopwatch.ElapsedMilliseconds;

                // 强制GC以避免内存累积影响后续计算
                GC.Collect();
                GC.WaitForPendingFinalizers();

                return (result, times);
            }
            catch (Exception)
            {
                times["Error"] = -1;
                return (-1, times);
            }
        }

        private double[] Smooth(double[] lst, int T)
        {
            ReadOnlySpan<double> lstSpan = lst;
            double[] prefixSum = new double[T + 1];
            Span<double> prefixSpan = prefixSum;
            prefixSpan[0] = 0;
            for (int i = 1; i <= T; i++)
                prefixSpan[i] = prefixSpan[i - 1] + lstSpan[i - 1];

            double[] lstBar = new double[T];
            Span<double> lstBarSpan = lstBar;

            for (int s = 0; s < T; s += granularity)
            {
                int left = Math.Max(0, s - 500);
                int right = Math.Min(T, s + 500);
                double sum = prefixSpan[right] - prefixSpan[left];
                lstBarSpan[s] = 0.001 * sum; // 因为步长是1ms，不允许修改
            }

            return lstBar;
        }

        private (double[] JBar, double[][] deltaKs) CalculateSection23(int K, SRsNote[][] noteSeqByColumn, int T, double x)
        {
            double[][] JKs = new double[K][];
            double[][] deltaKs = new double[K][];

            if (K > 7)
            {
                // 局部变量，避免lambda捕获大对象
                SRsNote[][] localNoteSeqByColumn = noteSeqByColumn;
                int localT = T;
                double localX = x;
                Parallel.For(0, K, k =>
                {
                    JKs[k] = new double[localT];
                    deltaKs[k] = new double[localT];
                    Array.Fill(deltaKs[k], 1e9);

                    // 预计算复杂表达式，减少重复计算
                    double xPow025 = Math.Sqrt(Math.Sqrt(localX));
                    double lambda1X = lambda_1 * xPow025; // 只有当该列有音符时才处理

                    if (k < localNoteSeqByColumn.Length && localNoteSeqByColumn[k].Length > 1)
                    {
                        for (int i = 0; i < localNoteSeqByColumn[k].Length - 1; i++)
                        {
                            double delta = 0.001 * (localNoteSeqByColumn[k][i + 1].StartTime - localNoteSeqByColumn[k][i].StartTime);
                            if (delta < 1e-9) continue; // 避免除零错误
                            double absDelta = Math.Abs(delta - 0.08);
                            double temp = 0.15 + absDelta;
                            double temp4 = temp * temp * temp * temp;
                            double jack = 1 - 7e-5 * (1 / temp4);
                            double val = 1 / (delta * (delta + lambda1X)) * jack;

                            // Define start and end for filling the range in deltaKs and JKs
                            int start = (int)localNoteSeqByColumn[k][i].StartTime;
                            int end = (int)localNoteSeqByColumn[k][i + 1].StartTime;
                            int length = end - start;

                            // Use Span to fill subarrays
                            var deltaSpan = new Span<double>(deltaKs[k], start, length);
                            deltaSpan.Fill(delta);

                            var JKsSpan = new Span<double>(JKs[k], start, length);
                            JKsSpan.Fill(val);
                        }
                    }
                });
            }
            else
            {
                for (int k = 0; k < K; k++)
                {
                    JKs[k] = new double[T];
                    deltaKs[k] = new double[T];
                    Array.Fill(deltaKs[k], 1e9);

                    // 预计算复杂表达式，减少重复计算
                    double xPow025 = Math.Sqrt(Math.Sqrt(x));
                    double lambda1X = lambda_1 * xPow025; // 只有当该列有音符时才处理

                    if (k < noteSeqByColumn.Length && noteSeqByColumn[k].Length > 1)
                    {
                        for (int i = 0; i < noteSeqByColumn[k].Length - 1; i++)
                        {
                            double delta = 0.001 * (noteSeqByColumn[k][i + 1].StartTime - noteSeqByColumn[k][i].StartTime);
                            if (delta < 1e-9) continue; // 避免除零错误
                            double absDelta = Math.Abs(delta - 0.08);
                            double temp = 0.15 + absDelta;
                            double temp4 = temp * temp * temp * temp;
                            double jack = 1 - 7e-5 * (1 / temp4);
                            double val = 1 / (delta * (delta + lambda1X)) * jack;

                            // Define start and end for filling the range in deltaKs and JKs
                            int start = noteSeqByColumn[k][i].StartTime;
                            int end = noteSeqByColumn[k][i + 1].StartTime;
                            int length = end - start;

                            // Use Span to fill subarrays
                            var deltaSpan = new Span<double>(deltaKs[k], start, length);
                            deltaSpan.Fill(delta);

                            var JKsSpan = new Span<double>(JKs[k], start, length);
                            JKsSpan.Fill(val);
                        }
                    }
                }
            }

            // Smooth the JKs array，让系统自动调度
            double[][] JBarKs = new double[K][];
            Parallel.For(0, K, k => JBarKs[k] = Smooth(JKs[k], T));

            // Calculate JBar
            double[] JBar = new double[T];

            for (int s = 0; s < T; s += granularity)
            {
                double weightedSum = 0;
                double weightSum = 0;

                // Replace list allocation with direct accumulation
                for (int i = 0; i < K; i++)
                {
                    double val = JBarKs[i][s];
                    double weight = 1.0 / deltaKs[i][s];

                    weightSum += weight;
                    weightedSum += Math.Pow(Math.Max(val, 0), lambda_n) * weight;
                }

                weightSum = Math.Max(1e-9, weightSum);
                JBar[s] = Math.Pow(weightedSum / weightSum, 1.0 / lambda_n);
            }

            return (JBar, deltaKs);
        }

        private double[] CalculateSection24(int K, int T, SRsNote[][] noteSeqByColumn, double x)
        {
            double[][] XKs = new double[K + 1][];

            Parallel.For(0, K + 1, k =>
            {
                XKs[k] = new double[T];
                SRsNote[] notesInPair;

                if (k == 0)
                    notesInPair = noteSeqByColumn.Length > 0 ? noteSeqByColumn[0] : [];
                else if (k == K)
                    notesInPair = noteSeqByColumn.Length > 0 ? noteSeqByColumn[^1] : [];
                else
                {
                    int leftCol = k - 1;
                    int rightCol = k;
                    SRsNote[] leftNotes = leftCol < noteSeqByColumn.Length ? noteSeqByColumn[leftCol] : [];
                    SRsNote[] rightNotes = rightCol < noteSeqByColumn.Length ? noteSeqByColumn[rightCol] : [];
                    notesInPair = leftNotes.Concat(rightNotes).OrderBy(n => n.StartTime).ToArray();
                }

                Span<double> XKsSpan = XKs[k];

                for (int i = 1; i < notesInPair.Length; i++)
                {
                    double delta = 0.001 * (notesInPair[i].StartTime - notesInPair[i - 1].StartTime);
                    double maxXd = Math.Max(x, delta);
                    double val = 0.16 / (maxXd * maxXd);

                    int start = notesInPair[i - 1].StartTime;
                    int end = notesInPair[i].StartTime;
                    int length = end - start;
                    XKsSpan.Slice(start, length).Fill(val);
                }
            });

            double[] X = new double[T];

            for (int s = 0; s < T; s += granularity)
            {
                X[s] = 0;
                double[] matrix = CrossMatrixProvider.GetMatrix(K);
                for (int k = 0; k <= K; k++) X[s] += XKs[k][s] * matrix[k];
            }

            return Smooth(X, T);
        }

        private double[] CalculateSection25(int T, SRsNote[] LNSeq, SRsNote[] noteSeq, double x)
        {
            double[] P = new double[T];
            double[] LNBodies = new double[T];

            // 简化：使用系统默认的线程数，避免过度限制
            int numThreads = Environment.ProcessorCount;
            double[] partialLNBodies = new double[numThreads * T];

            Parallel.For(0, LNSeq.Length, i =>
            {
                int threadId = i % numThreads;
                int offset = threadId * T;
                SRsNote note = LNSeq[i];
                int t1 = Math.Min(note.StartTime + 80, note.EndTime);
                for (int t = note.StartTime; t < t1; t++)
                    partialLNBodies[offset + t] += 0.5;
                for (int t = t1; t < note.EndTime; t++)
                    partialLNBodies[offset + t] += 1;
            });

            // 合并结果
            for (int t = 0; t < T; t++)
            {
                for (int i = 0; i < numThreads; i++)
                    LNBodies[t] += partialLNBodies[i * T + t];
            }

            // 优化：计算 LNBodies 前缀和，用于快速求和
            double[] prefixSumLNBodies = new double[T + 1];
            for (int t = 1; t <= T; t++)
                prefixSumLNBodies[t] = prefixSumLNBodies[t - 1] + LNBodies[t - 1];

            double B(double delta)
            {
                double val = 7.5 / delta;

                if (val is > 160 and < 360)
                {
                    double diff = val - 160;
                    double diff2 = val - 360;
                    return 1 + 1.4e-7 * diff * (diff2 * diff2);
                }

                return 1;
            }

            // 预计算常量，减少重复计算
            double lambda2Scaled = lambda_2 * 0.001;

            for (int i = 0; i < noteSeq.Length - 1; i++)
            {
                double delta = 0.001 * (noteSeq[i + 1].StartTime - noteSeq[i].StartTime);

                if (delta < 1e-9)
                    P[noteSeq[i].StartTime] += 1000 * Math.Sqrt(Math.Sqrt(0.02 * (4 / x - lambda_3)));
                else
                {
                    int h_l = noteSeq[i].StartTime;
                    int h_r = noteSeq[i + 1].StartTime;
                    double v = 1 + lambda2Scaled * (prefixSumLNBodies[h_r] - prefixSumLNBodies[h_l]);

                    if (delta < 2 * x / 3)
                    {
                        double baseVal = Math.Sqrt(Math.Sqrt(0.08 / x *
                                                             (1 - lambda_3 / x * (delta - x / 2) * (delta - x / 2)))) *
                                         B(delta) * v / delta;

                        for (int s = h_l; s < h_r; s++)
                            P[s] += baseVal;
                    }
                    else
                    {
                        double baseVal = Math.Sqrt(Math.Sqrt(0.08 / x *
                                                             (1 - lambda_3 / x * (x / 6) * (x / 6)))) *
                                         B(delta) * v / delta;

                        for (int s = h_l; s < h_r; s++)
                            P[s] += baseVal;
                    }
                }
            }

            return Smooth(P, T);
        }

        private (double[] ABar, int[] KS) CalculateSection26(double[][] deltaKs, int K, int T, SRsNote[] noteSeq)
        {
            bool[][] KUKs = new bool[K][];
            for (int k = 0; k < K; k++) KUKs[k] = new bool[T];

            // 并行化：每个note独立填充KUKs，优化性能
            Parallel.ForEach(noteSeq, note =>
            {
                int startTime = Math.Max(0, note.StartTime - 500);
                int endTime = note.EndTime < 0 ? Math.Min(note.StartTime + 500, T - 1) : Math.Min(note.EndTime + 500, T - 1);

                for (int s = startTime; s < endTime; s++) KUKs[note.Index][s] = true;
            });

            int[] KS = new int[T];
            double[] A = new double[T];
            Array.Fill(A, 1);

            double[][] dks = new double[K - 1][];
            for (int k = 0; k < K - 1; k++) dks[k] = new double[T];

            // 并行化：每个s独立计算，优化性能
            for (int sIndex = 0; sIndex < T / granularity; sIndex++)
            {
                int s = sIndex * granularity;
                int[] cols = new int[K]; // 使用数组而不是List
                int colCount = 0;

                for (int k = 0; k < K; k++)
                {
                    if (KUKs[k][s])
                        cols[colCount++] = k;
                }

                KS[s] = Math.Max(colCount, 1);

                for (int i = 0; i < colCount - 1; i++)
                {
                    int col1 = cols[i];
                    int col2 = cols[i + 1];

                    dks[col1][s] = Math.Abs(deltaKs[col1][s] - deltaKs[col2][s]) +
                                   Math.Max(0, Math.Max(deltaKs[col1][s], deltaKs[col2][s]) - 0.3);

                    double maxDelta = Math.Max(deltaKs[col1][s], deltaKs[col2][s]);
                    if (dks[col1][s] < 0.02)
                        A[s] *= Math.Min(0.75 + 0.5 * maxDelta, 1);
                    else if (dks[col1][s] < 0.07) A[s] *= Math.Min(0.65 + 5 * dks[col1][s] + 0.5 * maxDelta, 1);
                }
            }

            return (Smooth(A, T), KS);
        }

        private SRsNote FindNextNoteInColumn(SRsNote sRsNote, SRsNote[] columnNotes)
        {
            int index = Array.BinarySearch(columnNotes, sRsNote, Comparer<SRsNote>.Create((a, b) => a.StartTime.CompareTo(b.StartTime)));

            // If the exact element is not found, BinarySearch returns a bitwise complement of the index.
            // Convert it to the nearest index of an element >= note.H
            if (index < 0) index = ~index;

            return index + 1 < columnNotes.Length
                       ? columnNotes[index + 1]
                       : new SRsNote(0, 1000000000, 1000000000);
        }

        private (double[] RBar, double[] Is) CalculateSection27(SRsNote[] LNSeq, SRsNote[] tailSeq, int T, SRsNote[][] noteSeqByColumn, double x)
        {
            double[] I = new double[LNSeq.Length];
            Parallel.For(0, tailSeq.Length, i =>
            {
                (int k, int h_i, int t_i) = (tailSeq[i].Index, tailSeq[i].StartTime, tailSeq[i].EndTime);
                SRsNote[] columnNotes = k < noteSeqByColumn.Length ? noteSeqByColumn[k] : [];
                SRsNote nextNote = FindNextNoteInColumn(tailSeq[i], columnNotes);
                (_, int h_j, _) = (nextNote.Index, nextNote.StartTime, nextNote.EndTime);

                double I_h = 0.001 * Math.Abs(t_i - h_i - 80) / x;
                double I_t = 0.001 * Math.Abs(h_j - t_i - 80) / x;
                I[i] = 2 / (2 + Math.Exp(-5 * (I_h - 0.75)) + Math.Exp(-5 * (I_t - 0.75)));
            });

            double[] Is = new double[T];
            double[] R = new double[T];

            Parallel.For(0, tailSeq.Length - 1, i =>
            {
                double delta_r = 0.001 * (tailSeq[i + 1].EndTime - tailSeq[i].EndTime);
                double isVal = 1 + I[i];
                double rVal = 0.08 * Math.Pow(delta_r, -1.0 / 2) * Math.Pow(x, -1) * (1 + 0.8 * (I[i] + I[i + 1]));

                for (int s = tailSeq[i].EndTime; s < tailSeq[i + 1].EndTime; s++)
                {
                    Is[s] = isVal;
                    R[s] = rVal;
                }
            });

            return (Smooth(R, T), Is);
        }

        private double RescaleHigh(double sr)
        {
            if (sr <= 9)
                return sr;
            return 9 + (sr - 9) * (1 / 1.2);
        }

        private void ForwardFill(double[] array)
        {
            double lastValidValue = 0; // Use initialValue for leading NaNs and 0s

            for (int i = 0; i < array.Length; i++)
            {
                if (!double.IsNaN(array[i]) && array[i] != 0) // Check if the current value is valid (not NaN or 0)
                    lastValidValue = array[i];
                else
                    array[i] = lastValidValue; // Replace NaN or 0 with last valid value or initial value
            }
        }

        private double CalculateSection3(
            double[] JBar,
            double[] XBar,
            double[] PBar,
            double[] ABar,
            double[] RBar,
            int[] KS,
            int T,
            SRsNote[] noteSeq,
            SRsNote[] LNSeq,
            int K)
        {
            double[] C = new double[T];
            int start = 0, end = 0;

            for (int t = 0; t < T; t++)
            {
                while (start < noteSeq.Length && noteSeq[start].StartTime < t - 500)
                    start++;
                while (end < noteSeq.Length && noteSeq[end].StartTime < t + 500)
                    end++;
                C[t] = end - start;
            }

            double[] S = new double[T];
            double[] D = new double[T];

            // 并行化计算S和D
            Parallel.For(0, T, t =>
            {
                // Ensure all values are non-negative
                JBar[t] = Math.Max(0, JBar[t]);
                XBar[t] = Math.Max(0, XBar[t]);
                PBar[t] = Math.Max(0, PBar[t]);
                ABar[t] = Math.Max(0, ABar[t]);
                RBar[t] = Math.Max(0, RBar[t]);

                double term1 = Math.Pow(0.4 * Math.Pow(Math.Pow(ABar[t], 3.0 / KS[t]) * Math.Min(JBar[t], 8 + 0.85 * JBar[t]), 1.5), 1);
                double term2 = Math.Pow((1 - 0.4) * Math.Pow(Math.Pow(ABar[t], 2.0 / 3) *
                                                             (0.8 * PBar[t] + RBar[t] * 35 / (C[t] + 8)), 1.5), 1);
                S[t] = Math.Pow(term1 + term2, 2.0 / 3);

                double T_t = Math.Pow(ABar[t], 3.0 / KS[t]) * XBar[t] / (XBar[t] + S[t] + 1);
                D[t] = 2.7 * Math.Pow(S[t], 0.5) * Math.Pow(T_t, 1.5) + S[t] * 0.27;
            });

            ForwardFill(D);
            ForwardFill(C);

            // New percentile-based calculation
            var dList = new List<double>();
            var weights = new List<double>();

            for (int t = 0; t < T; t++)
            {
                dList.Add(D[t]);
                weights.Add(C[t]);
            }

            // Sort D by value, keep weights
            List<(double d, double w)> sortedPairs = dList.Zip(weights, (d, w) => (d, w)).OrderBy(p => p.d).ToList();
            double[] sortedD = sortedPairs.Select(p => p.d).ToArray();
            double[] sortedWeights = sortedPairs.Select(p => p.w).ToArray();

            // Cumulative weights
            double totalWeight = sortedWeights.Sum();
            double[] cumWeights = new double[sortedWeights.Length];
            cumWeights[0] = sortedWeights[0];
            for (int i = 1; i < cumWeights.Length; i++)
                cumWeights[i] = cumWeights[i - 1] + sortedWeights[i];

            double[] normCumWeights = cumWeights.Select(cw => cw / totalWeight).ToArray();

            double[] targetPercentiles = [0.945, 0.935, 0.925, 0.915, 0.845, 0.835, 0.825, 0.815];

            double percentile93 = 0, percentile83 = 0;

            for (int i = 0; i < 4; i++)
            {
                int idx = Array.BinarySearch(normCumWeights, targetPercentiles[i]);
                if (idx < 0) idx = ~idx;
                if (idx >= sortedD.Length) idx = sortedD.Length - 1;
                percentile93 += sortedD[idx];
            }

            percentile93 /= 4;

            for (int i = 4; i < 8; i++)
            {
                int idx = Array.BinarySearch(normCumWeights, targetPercentiles[i]);
                if (idx < 0) idx = ~idx;
                if (idx >= sortedD.Length) idx = sortedD.Length - 1;
                percentile83 += sortedD[idx];
            }

            percentile83 /= 4;

            double weightedMean = Math.Pow(sortedD.Zip(sortedWeights, (d, w) => Math.Pow(d, 5) * w).Sum() / sortedWeights.Sum(), 1.0 / 5);

            double SR = 0.88 * percentile93 * 0.25 + 0.94 * percentile83 * 0.2 + weightedMean * 0.55;
            SR = Math.Pow(SR, 1.0) / Math.Pow(8, 1.0) * 8;

            double totalNotes = noteSeq.Length + 0.5 * LNSeq.Length;
            SR *= totalNotes / (totalNotes + 60);

            SR = RescaleHigh(SR);
            SR *= 0.975;

            return SR;
        }

        // Rust DLL import
        [DllImport("rust_sr_calculator.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern double calculate_sr_from_struct(IntPtr data);

        [DllImport("rust_sr_calculator.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr calculate_sr_from_osu_content(byte[] content, int len);

        [DllImport("rust_sr_calculator.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr calculate_sr_from_osu_file(byte[] path, int len);

        [StructLayout(LayoutKind.Sequential)]
        private struct CHitObject
        {
            public int col;
            public int start_time;
            public int end_time;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct CBeatmapData
        {
            public double overall_difficulty;
            public double circle_size;
            public IntPtr hit_objects_count; // usize is IntPtr
            public IntPtr hit_objects_ptr;
        }

        /// <summary>
        /// Calculate SR using Rust DLL
        /// </summary>
        public double CalculateSRRust<T>(IBeatmap<T> beatmap) where T : HitObject
        {
            double od = beatmap.BeatmapInfo.Difficulty.OverallDifficulty;
            int keyCount = (int)beatmap.BeatmapInfo.Difficulty.CircleSize;

            if (keyCount > 18 || keyCount < 1 || (keyCount > 10 && keyCount % 2 == 1)) return -1;

            var hitObjects = new CHitObject[beatmap.HitObjects.Count];

            for (int i = 0; i < beatmap.HitObjects.Count; i++)
            {
                T hitObject = beatmap.HitObjects[i];
                int col = hitObject is ManiaHitObject maniaHit ? maniaHit.Column : ManiaExtensions.GetColumnFromX(keyCount, hitObject.Position.X);
                int start = Math.Max(0, (int)hitObject.StartTime);
                int end = hitObject.EndTime > start ? Math.Min(1000000, (int)hitObject.EndTime) : -1;
                hitObjects[i] = new CHitObject
                {
                    col = col,
                    start_time = start,
                    end_time = end
                };
            }

            var data = new CBeatmapData
            {
                overall_difficulty = od,
                circle_size = keyCount,
                hit_objects_count = new IntPtr(hitObjects.Length),
                hit_objects_ptr = Marshal.AllocHGlobal(Marshal.SizeOf<CHitObject>() * hitObjects.Length)
            };

            // Copy array to unmanaged memory
            for (int i = 0; i < hitObjects.Length; i++) Marshal.StructureToPtr(hitObjects[i], data.hit_objects_ptr + i * Marshal.SizeOf<CHitObject>(), false);

            IntPtr dataPtr = Marshal.AllocHGlobal(Marshal.SizeOf<CBeatmapData>());
            Marshal.StructureToPtr(data, dataPtr, false);

            double sr = calculate_sr_from_struct(dataPtr);

            // Free memory
            Marshal.FreeHGlobal(data.hit_objects_ptr);
            Marshal.FreeHGlobal(dataPtr);

            return sr;
        }

        /// <summary>
        /// 使用Rust版本计算SR，直接从.osu文件路径
        /// </summary>
        /// <param name="filePath">.osu文件路径</param>
        /// <returns>SR值</returns>
        public double CalculateSRFromFile(string filePath)
        {
            byte[] pathBytes = System.Text.Encoding.UTF8.GetBytes(filePath);
            IntPtr resultPtr = calculate_sr_from_osu_file(pathBytes, pathBytes.Length);

            if (resultPtr == IntPtr.Zero)
                return -1.0;

            string jsonResult = Marshal.PtrToStringUTF8(resultPtr)!;
            // Note: In real implementation, we should free the string allocated by Rust
            // For simplicity, assuming Rust handles memory management

            // Parse JSON result
            JsonDocument json = JsonDocument.Parse(jsonResult);
            return json.RootElement.GetProperty("sr").GetDouble();
        }

        /// <summary>
        /// 使用Rust版本计算SR，直接从.osu文件内容
        /// </summary>
        /// <param name="content">.osu文件内容</param>
        /// <returns>SR值</returns>
        public double CalculateSRFromContent(string content)
        {
            byte[] contentBytes = System.Text.Encoding.UTF8.GetBytes(content);
            IntPtr resultPtr = calculate_sr_from_osu_content(contentBytes, contentBytes.Length);

            if (resultPtr == IntPtr.Zero)
                return -1.0;

            string jsonResult = Marshal.PtrToStringUTF8(resultPtr)!;
            // Parse JSON result
            JsonDocument json = JsonDocument.Parse(jsonResult);
            return json.RootElement.GetProperty("sr").GetDouble();
        }
    }
}
