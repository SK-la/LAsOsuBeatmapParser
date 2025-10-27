using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Beatmaps.Formats;
using LAsOsuBeatmapParser.Extensions;

namespace LAsOsuBeatmapParser.Analysis
{
    /// <summary>
    ///     SR计算器，用于计算谱面的xxy SR值
    ///     <para></para>
    ///     From: https://github.com/sunnyxxy/Star-Rating-Rebirth
    /// </summary>
    public class SRCalculator
    {
        private const double lambda_n = 5;
        private const double lambda_1 = 0.11;
        private const double lambda_3 = 24;
        private const double lambda_2 = 7.0;
        private const double lambda_4 = 0.1;
        private const double w_0      = 0.4;
        private const double w_1      = 2.7;
        private const double p_1      = 1.5;
        private const double w_2      = 0.27;
        private const double p_0      = 1.0;

        private const int granularity = 1; // 只能保持为1，确保精度不变，不可修改

        private SRCalculator() { } // 私有构造函数

        /// <summary>
        ///     单例模式：无状态类，线程安全
        /// </summary>
        public static SRCalculator Instance { get; } = new SRCalculator();

        /// <summary>
        ///     同步计算，等待返回值，不要用在高并发场景！
        /// </summary>
        /// <param name="beatmap"></param>
        /// <param name="times"></param>
        /// <typeparam name="T"></typeparam>
        /// <returns></returns>
        public double CalculateSR<T>(IBeatmap<T> beatmap, out Dictionary<string, long> times) where T : HitObject
        {
            Task<(double sr, Dictionary<string, long> times)> task = CalculateSRAsync(beatmap);
            (double sr, Dictionary<string, long> t) = task.Result; // 同步等待（用于兼容旧接口）
            times                                   = t;
            return sr;
        }

        /// <summary>
        ///     异步SR计算核心，已并行化所有section，后台计算
        /// </summary>
        /// <param name="beatmap"></param>
        /// <typeparam name="T"></typeparam>
        /// <returns></returns>
        public async Task<(double sr, Dictionary<string, long> times)> CalculateSRAsync<T>(IBeatmap<T> beatmap) where T : HitObject
        {
            double od       = beatmap.BeatmapInfo.Difficulty.OverallDifficulty;
            int    keyCount = (int)beatmap.BeatmapInfo.Difficulty.CircleSize;
            var    times    = new Dictionary<string, long>();

            // Check if key count is supported (max 18 keys, even numbers only for K>10)
            if (keyCount > 18 || keyCount < 1 || (keyCount > 10 && keyCount % 2 == 1)) return (-1, times); // Return invalid SR

            try
            {
                var totalStopwatch = Stopwatch.StartNew(); // 总时间计时开始
                var noteSequence   = new List<SRsNote>();

                foreach (T hitObject in beatmap.HitObjects)
                {
                    int col  = hitObject is ManiaHitObject maniaHit ? maniaHit.Column : ManiaExtensions.GetColumnFromX(keyCount, hitObject.Position.X);
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
                x = Math.Min(x, (0.6 * (x - 0.09)) + 0.09);
                SRsNote[][] noteSeqByColumn = noteSeq.GroupBy(n => n.Index).OrderBy(g => g.Key).Select(g => g.ToArray()).ToArray();

                // 优化：预计算LN序列长度，避免Where().ToArray()
                int lnCount = 0;

                foreach (SRsNote note in noteSeq)
                {
                    if (note.EndTime >= 0)
                        lnCount++;
                }

                var LNSeq   = new SRsNote[lnCount];
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

                var (allCorners, baseCorners, _) = GetCorners(totalTime, noteSeq);
                var keyUsage400 = GetKeyUsage400(keyCount, totalTime, noteSeq, baseCorners);
                var anchorBase = ComputeAnchor(keyCount, keyUsage400, baseCorners);
                double[] anchor = new double[totalTime];

                for (int t = 0; t < totalTime; t++)
                {
                    anchor[t] = InterpValues(new double[] { t }, baseCorners, anchorBase)[0];
                }

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
                    var      sectionStopwatch = Stopwatch.StartNew();
                    double[] XBar             = CalculateSection24(keyCount, totalTime, noteSeqByColumn, x, noteSeq);
                    sectionStopwatch.Stop();
                    return XBar;
                });

                Task<double[]> task25 = Task.Run(() =>
                {
                    var      sectionStopwatch = Stopwatch.StartNew();
                    double[] PBar             = CalculateSection25(totalTime, LNSeq, noteSeq, x, anchor);
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
                Task<(double[] ABar, int[] KS)>    task26 = Task.Run(() => CalculateSection26(deltaKs, keyCount, totalTime, noteSeq));
                Task<(double[] RBar, double[] Is)> task27 = Task.Run(() => CalculateSection27(LNSeq, tailSeq, totalTime, noteSeqByColumn, x));

                // Wait for both tasks to complete
                await Task.WhenAll(task26, task27).ConfigureAwait(false);

                // Retrieve results
                (double[] ABar, int[] KS) = task26.Result;
                (double[] RBar, _)        = task27.Result;

                stopwatch.Stop();
                // Logger.WriteLine(LogLevel.Debug, $"[SRCalculator]Section 26/27 Time: {stopwatch.ElapsedMilliseconds}ms");
                times["Section2627"] = stopwatch.ElapsedMilliseconds;

                double[] C_step = new double[baseCorners.Length];
                for (int i = 0; i < baseCorners.Length; i++)
                {
                    double s = baseCorners[i];
                    int low = (int)Math.Max(0, s - 500);
                    int high = (int)Math.Min(totalTime, s + 500);
                    int cnt = 0;
                    for (int j = 0; j < noteSeq.Length; j++)
                    {
                        if (noteSeq[j].StartTime >= low && noteSeq[j].StartTime < high) cnt++;
                    }
                    C_step[i] = cnt;
                }
                double[] C_arr = new double[allCorners.Length];
                for (int i = 0; i < allCorners.Length; i++)
                {
                    double q = allCorners[i];
                    int idx = Array.BinarySearch(baseCorners, q);
                    if (idx >= 0) C_arr[i] = C_step[idx];
                    else
                    {
                        idx = ~idx - 1;
                        if (idx < 0) C_arr[i] = 0;
                        else C_arr[i] = C_step[idx];
                    }
                }
                double[] gaps = new double[allCorners.Length];
                if (allCorners.Length > 0)
                {
                    gaps[0] = allCorners.Length > 1 ? (allCorners[1] - allCorners[0]) / 2.0 : allCorners[0] / 2.0;
                    if (allCorners.Length > 1)
                        gaps[allCorners.Length - 1] = (allCorners[allCorners.Length - 1] - allCorners[allCorners.Length - 2]) / 2.0;
                    for (int i = 1; i < allCorners.Length - 1; i++)
                    {
                        gaps[i] = (allCorners[i + 1] - allCorners[i - 1]) / 2.0;
                    }
                }
                double[] effective_weights = new double[allCorners.Length];
                for (int i = 0; i < allCorners.Length; i++)
                {
                    effective_weights[i] = C_arr[i] * gaps[i];
                }
                double[] T_array = new double[totalTime];
                for (int i = 0; i < totalTime; i++) T_array[i] = i;
                double[] JBar_all = InterpValues(allCorners, T_array, JBar);
                double[] XBar_all = InterpValues(allCorners, T_array, XBar);
                double[] PBar_all = InterpValues(allCorners, T_array, PBar);
                double[] ABar_all = InterpValues(allCorners, T_array, ABar);
                double[] RBar_all = InterpValues(allCorners, T_array, RBar);
                int[] KS_all = new int[allCorners.Length];
                for (int i = 0; i < allCorners.Length; i++)
                {
                    double q = allCorners[i];
                    int idx = Array.BinarySearch(T_array, q);
                    if (idx >= 0) KS_all[i] = KS[idx];
                    else
                    {
                        idx = ~idx - 1;
                        if (idx < 0) KS_all[i] = 1;
                        else KS_all[i] = KS[idx];
                    }
                }
                double[] D_all = new double[allCorners.Length];

                // Final calculation
                stopwatch.Restart();
                double result = CalculateSection3(JBar_all, XBar_all, PBar_all, ABar_all, RBar_all, KS_all, allCorners.Length, noteSeq, LNSeq, keyCount, C_arr, effective_weights, D_all);
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
            ReadOnlySpan<double> lstSpan    = lst;
            double[]             prefixSum  = new double[T + 1];
            Span<double>         prefixSpan = prefixSum;
            prefixSpan[0] = 0;
            for (int i = 1; i <= T; i++)
                prefixSpan[i] = prefixSpan[i - 1] + lstSpan[i - 1];

            double[]     lstBar     = new double[T];
            Span<double> lstBarSpan = lstBar;

            for (int s = 0; s < T; s += granularity)
            {
                int    left  = Math.Max(0, s - 500);
                int    right = Math.Min(T, s + 500);
                double sum   = prefixSpan[right] - prefixSpan[left];
                lstBarSpan[s] = 0.001 * sum; // 因为步长是1ms，不允许修改
            }

            return lstBar;
        }

        private (double[] JBar, double[][] deltaKs) CalculateSection23(int K, SRsNote[][] noteSeqByColumn, int T, double x)
        {
            double[][] JKs     = new double[K][];
            double[][] deltaKs = new double[K][];

            if (K > 7)
            {
                // 局部变量，避免lambda捕获大对象
                SRsNote[][] localNoteSeqByColumn = noteSeqByColumn;
                int         localT               = T;
                double      localX               = x;
                Parallel.For(0, K, k =>
                {
                    JKs[k]     = new double[localT];
                    deltaKs[k] = new double[localT];
                    Array.Fill(deltaKs[k], 1e9);

                    // 预计算复杂表达式，减少重复计算
                    double xPow025  = Math.Sqrt(Math.Sqrt(localX));
                    double lambda1X = lambda_1 * xPow025; // 只有当该列有音符时才处理

                    if (k < localNoteSeqByColumn.Length && localNoteSeqByColumn[k].Length > 1)
                    {
                        for (int i = 0; i < localNoteSeqByColumn[k].Length - 1; i++)
                        {
                            double delta = 0.001 * (localNoteSeqByColumn[k][i + 1].StartTime - localNoteSeqByColumn[k][i].StartTime);
                            if (delta < 1e-9) continue; // 避免除零错误

                            double absDelta = Math.Abs(delta - 0.08);
                            double temp     = 0.15 + absDelta;
                            double temp4    = temp * temp * temp * temp;
                            double jack     = 1 - (7e-5 * (1 / temp4));
                            double val      = 1 / (delta * (delta + lambda1X)) * jack;

                            // Define start and end for filling the range in deltaKs and JKs
                            int start  = localNoteSeqByColumn[k][i].StartTime;
                            int end    = localNoteSeqByColumn[k][i + 1].StartTime;
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
                    JKs[k]     = new double[T];
                    deltaKs[k] = new double[T];
                    Array.Fill(deltaKs[k], 1e9);

                    // 预计算复杂表达式，减少重复计算
                    double xPow025  = Math.Sqrt(Math.Sqrt(x));
                    double lambda1X = lambda_1 * xPow025; // 只有当该列有音符时才处理

                    if (k < noteSeqByColumn.Length && noteSeqByColumn[k].Length > 1)
                    {
                        for (int i = 0; i < noteSeqByColumn[k].Length - 1; i++)
                        {
                            double delta = 0.001 * (noteSeqByColumn[k][i + 1].StartTime - noteSeqByColumn[k][i].StartTime);
                            if (delta < 1e-9) continue; // 避免除零错误

                            double absDelta = Math.Abs(delta - 0.08);
                            double temp     = 0.15 + absDelta;
                            double temp4    = temp * temp * temp * temp;
                            double jack     = 1 - (7e-5 * (1 / temp4));
                            double val      = 1 / (delta * (delta + lambda1X)) * jack;

                            // Define start and end for filling the range in deltaKs and JKs
                            int start  = noteSeqByColumn[k][i].StartTime;
                            int end    = noteSeqByColumn[k][i + 1].StartTime;
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
                double weightSum   = 0;

                // Replace list allocation with direct accumulation
                for (int i = 0; i < K; i++)
                {
                    double val    = JBarKs[i][s];
                    double weight = 1.0 / deltaKs[i][s];

                    weightSum   += weight;
                    weightedSum += Math.Pow(Math.Max(val, 0), lambda_n) * weight;
                }

                weightSum = Math.Max(1e-9, weightSum);
                JBar[s]   = Math.Pow(weightedSum / weightSum, 1.0 / lambda_n);
            }

            return (JBar, deltaKs);
        }

        private double[] CalculateSection24(int K, int T, SRsNote[][] noteSeqByColumn, double x, SRsNote[] noteSeq)
        {
            bool[][] activeColumnsT = new bool[T][];
            for (int t = 0; t < T; t++) activeColumnsT[t] = new bool[K];
            Parallel.ForEach(noteSeq, note =>
            {
                int startTime = Math.Max(0, note.StartTime - 150);
                int endTime = note.EndTime < 0 ? Math.Min(note.StartTime + 150, T - 1) : Math.Min(note.EndTime + 150, T - 1);
                for (int s = startTime; s <= endTime; s++) activeColumnsT[s][note.Index] = true;
            });
            List<int>[] activeListT = new List<int>[T];
            for (int t = 0; t < T; t++)
            {
                activeListT[t] = new List<int>();
                for (int kk = 0; kk < K; kk++) if (activeColumnsT[t][kk]) activeListT[t].Add(kk);
            }
            double[] cross_coeff = CrossMatrixProvider.GetMatrix(K);

            double[][] XKs = new double[K + 1][];
            double[][] fastCross = new double[K + 1][];

            Parallel.For(0, K + 1, k =>
            {
                XKs[k] = new double[T];
                fastCross[k] = new double[T];
                SRsNote[] notesInPair;

                if (k == 0)
                    notesInPair = noteSeqByColumn.Length > 0 ? noteSeqByColumn[0] : [];
                else if (k == K)
                    notesInPair = noteSeqByColumn.Length > 0 ? noteSeqByColumn[^1] : [];
                else
                {
                    int       leftCol    = k - 1;
                    int       rightCol   = k;
                    SRsNote[] leftNotes  = leftCol < noteSeqByColumn.Length ? noteSeqByColumn[leftCol] : [];
                    SRsNote[] rightNotes = rightCol < noteSeqByColumn.Length ? noteSeqByColumn[rightCol] : [];
                    notesInPair = leftNotes.Concat(rightNotes).OrderBy(n => n.StartTime).ToArray();
                }

                Span<double> XKsSpan = XKs[k];
                Span<double> fastCrossSpan = fastCross[k];

                for (int i = 1; i < notesInPair.Length; i++)
                {
                    double delta = 0.001 * (notesInPair[i].StartTime - notesInPair[i - 1].StartTime);
                    double maxXd = Math.Max(x, delta);
                    double val   = 0.16 / (maxXd * maxXd);

                    int start  = notesInPair[i - 1].StartTime;
                    int end    = notesInPair[i].StartTime;
                    int length = end - start;
                    if ((!activeListT[start].Contains(k - 1) && !activeListT[end].Contains(k - 1)) || (!activeListT[start].Contains(k) && !activeListT[end].Contains(k)))
                    {
                        val *= (1 - cross_coeff[k]);
                    }
                    XKsSpan.Slice(start, length).Fill(val);

                    double fastVal = Math.Max(0, 0.4 * Math.Pow((Math.Max(delta, Math.Max(0.06, 0.75 * x))), -2) - 80);
                    fastCrossSpan.Slice(start, length).Fill(fastVal);
                }
            });

            double[] X = new double[T];

            for (int s = 0; s < T; s += granularity)
            {
                X[s] = 0;
                double[] matrix                   = CrossMatrixProvider.GetMatrix(K);
                for (int k = 0; k <= K; k++) X[s] += XKs[k][s] * matrix[k];

                // Add fast cross terms
                for (int k = 0; k < K; k++)
                {
                    double sqrtTerm = Math.Sqrt(fastCross[k][s] * matrix[k] * fastCross[k + 1][s] * matrix[k + 1]);
                    X[s] += sqrtTerm;
                }
            }

            return Smooth(X, T);
        }

        private double[] CalculateSection25(int T, SRsNote[] LNSeq, SRsNote[] noteSeq, double x, double[] anchor)
        {
            double[] P        = new double[T];
            double[] LNBodies = new double[T];

            // 简化：使用系统默认的线程数，避免过度限制
            int      numThreads      = Environment.ProcessorCount;
            double[] partialLNBodies = new double[numThreads * T];

            Parallel.For(0, LNSeq.Length, i =>
            {
                int     threadId = i % numThreads;
                int     offset   = threadId * T;
                SRsNote note     = LNSeq[i];
                int     t1       = Math.Min(note.StartTime + 80, note.EndTime);
                for (int t = note.StartTime; t < t1; t++)
                    partialLNBodies[offset + t] += 0.5;
                for (int t = t1; t < note.EndTime; t++)
                    partialLNBodies[offset + t] += 1;
            });

            // 合并结果
            for (int t = 0; t < T; t++)
            {
                for (int i = 0; i < numThreads; i++)
                    LNBodies[t] += partialLNBodies[(i * T) + t];
            }

            // 优化：计算 LNBodies 前缀和，用于快速求和
            double[] prefixSumLNBodies = new double[T + 1];
            for (int t = 1; t <= T; t++)
                prefixSumLNBodies[t] = prefixSumLNBodies[t - 1] + LNBodies[t - 1];

            // 计算P
            for (int i = 0; i < noteSeq.Length - 1; i++)
            {
                int    h_l       = noteSeq[i].StartTime;
                int    h_r       = noteSeq[i + 1].StartTime;
                double deltaTime = h_r - h_l;

                if (deltaTime < 1e-9)
                {
                    // Dirac delta case
                    double spike = 1000 * Math.Pow(0.02 * ((4 / x) - 24), 1.0 / 4);
                    int    spikeStart = Math.Max(0, h_l);
                    int    spikeEnd   = Math.Min(T, h_l + 1);
                    for (int t = spikeStart; t < spikeEnd; t++)
                        P[t] += spike;
                    continue;
                }

                int pStart = Math.Max(0, h_l);
                int pEnd   = Math.Min(T, h_r);

                double delta = 0.001 * deltaTime;

                // LN_sum approximation
                double lnSum = prefixSumLNBodies[Math.Min(T, h_r)] - prefixSumLNBodies[Math.Max(0, h_l)];
                double v     = 1 + (6 * 0.001 * lnSum);

                double b_val = 1;
                if (160 < 7.5 / delta && 7.5 / delta < 360)
                {
                    double temp = 7.5 / delta;
                    b_val = 1 + 1.7e-7 * (temp - 160) * Math.Pow((temp - 360), 2);
                }

                double inc;
                if (delta < 2 * x / 3)
                {
                    double temp = (1 - (24 * ((delta - x / 2) * (delta - x / 2)) / x));
                    inc = 1 / delta * Math.Pow(0.08 / x * temp, 1.0 / 4) * Math.Max(b_val, v);
                }
                else
                {
                    double temp = (1 - (24 * ((x / 6) * (x / 6)) / x));
                    inc = 1 / delta * Math.Pow(0.08 / x * temp, 1.0 / 4) * Math.Max(b_val, v);
                }

                for (int t = pStart; t < pEnd; t++)
                {
                    P[t] += Math.Min(inc * anchor[t], Math.Max(inc, (inc * 2 - 10)));
                }
            }

            return Smooth(P, T);
        }

        private (double[] ABar, int[] KS) CalculateSection26(double[][] deltaKs, int K, int T, SRsNote[] noteSeq)
        {
            bool[][] KUKs                       = new bool[K][];
            for (int k = 0; k < K; k++) KUKs[k] = new bool[T];

            // 并行化：每个note独立填充KUKs，优化性能
            Parallel.ForEach(noteSeq, note =>
            {
                int startTime = Math.Max(0, note.StartTime - 500);
                int endTime   = note.EndTime < 0 ? Math.Min(note.StartTime + 500, T - 1) : Math.Min(note.EndTime + 500, T - 1);

                for (int s = startTime; s < endTime; s++) KUKs[note.Index][s] = true;
            });

            int[]    KS = new int[T];
            double[] A  = new double[T];
            Array.Fill(A, 1);

            double[][] dks                         = new double[K - 1][];
            for (int k = 0; k < K - 1; k++) dks[k] = new double[T];

            // 并行化：每个s独立计算，优化性能
            for (int sIndex = 0; sIndex < T / granularity; sIndex++)
            {
                int   s        = sIndex * granularity;
                int[] cols     = new int[K]; // 使用数组而不是List
                int   colCount = 0;

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
                                   (0.4 * Math.Max(0, Math.Max(deltaKs[col1][s], deltaKs[col2][s]) - 0.11));

                    double maxDelta = Math.Max(deltaKs[col1][s], deltaKs[col2][s]);
                    if (dks[col1][s] < 0.02)
                        A[s]                           *= Math.Min(0.75 + (0.5 * maxDelta), 1);
                    else if (dks[col1][s] < 0.07) A[s] *= Math.Min(0.65 + (5 * dks[col1][s]) + (0.5 * maxDelta), 1);
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
                SRsNote   nextNote    = FindNextNoteInColumn(tailSeq[i], columnNotes);
                (_, int h_j, _) = (nextNote.Index, nextNote.StartTime, nextNote.EndTime);

                double I_h = 0.001 * Math.Abs(t_i - h_i - 80) / x;
                double I_t = 0.001 * Math.Abs(h_j - t_i - 80) / x;
                I[i] = 2 / (2 + Math.Exp(-5 * (I_h - 0.75)) + Math.Exp(-5 * (I_t - 0.75)));
            });

            double[] Is = new double[T];
            double[] R  = new double[T];

            Parallel.For(0, tailSeq.Length - 1, i =>
            {
                double delta_r = 0.001 * (tailSeq[i + 1].EndTime - tailSeq[i].EndTime);
                double isVal   = 1 + I[i];
                double rVal    = 0.08 * Math.Pow(delta_r, -1.0 / 2) * Math.Pow(x, -1) * (1 + (0.8 * (I[i] + I[i + 1])));

                for (int s = tailSeq[i].EndTime; s < tailSeq[i + 1].EndTime; s++)
                {
                    Is[s] = isVal;
                    R[s]  = rVal;
                }
            });

            return (Smooth(R, T), Is);
        }

        private double RescaleHigh(double sr)
        {
            if (sr <= 9)
                return sr;

            return 9 + ((sr - 9) * (1 / 1.2));
        }

        private static int BisectLeft(double[] array, double value)
        {
            int low = 0, high = array.Length;
            while (low < high)
            {
                int mid = (low + high) / 2;
                if (array[mid] < value)
                    low = mid + 1;
                else
                    high = mid;
            }
            return low;
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

        private double CalculateSection3(double[]  JBar,
                                         double[]  XBar,
                                         double[]  PBar,
                                         double[]  ABar,
                                         double[]  RBar,
                                         int[]     KS,
                                         int       len,
                                         SRsNote[] noteSeq,
                                         SRsNote[] LNSeq,
                                         int       K,
                                         double[]  C_arr,
                                         double[]  effectiveWeights,
                                         double[]  D_all)
        {
            for (int i = 0; i < len; i++)
            {
                double term1 = 0.4 * Math.Pow(Math.Pow(ABar[i], 3.0 / KS[i]) * Math.Min(JBar[i], 8 + (0.85 * JBar[i])), 1.5);
                double term2 = (1 - 0.4) * Math.Pow(Math.Pow(ABar[i], 2.0 / 3) * ((0.8 * PBar[i]) + (RBar[i] * 35 / (C_arr[i] + 8))), 1.5);
                double S = Math.Pow(term1 + term2, 2.0 / 3);
                double T_t = Math.Pow(ABar[i], 3.0 / KS[i]) * XBar[i] / (XBar[i] + S + 1);
                D_all[i] = (2.7 * Math.Pow(S, 0.5) * Math.Pow(T_t, 1.5)) + (S * 0.27);
            }

            List<double> dList = D_all.ToList();
            List<double> wList = effectiveWeights.ToList();

            // Sort D by value, keep weights
            List<(double d, double w)> sortedPairs   = dList.Zip(wList, (d, w) => (d, w)).OrderBy(p => p.d).ToList();
            double[]                   sortedD       = sortedPairs.Select(p => p.d).ToArray();
            double[]                   sortedWeights = sortedPairs.Select(p => p.w).ToArray();

            // Cumulative weights
            double   totalWeight = sortedWeights.Sum();
            double[] cumWeights  = new double[sortedWeights.Length];
            cumWeights[0] = sortedWeights[0];
            for (int i = 1; i < cumWeights.Length; i++)
                cumWeights[i] = cumWeights[i - 1] + sortedWeights[i];

            double[] normCumWeights = cumWeights.Select(cw => cw / totalWeight).ToArray();

            double[] targetPercentiles = [0.99, 0.98, 0.97, 0.96, 0.89, 0.88, 0.87, 0.86];

            double percentile93 = 0, percentile83 = 0;

            for (int i = 0; i < 4; i++)
            {
                int idx = BisectLeft(normCumWeights, targetPercentiles[i]);
                if (idx >= sortedD.Length) idx = sortedD.Length - 1;
                percentile93 += sortedD[idx];
            }

            percentile93 /= 4;

            for (int i = 4; i < 8; i++)
            {
                int idx = BisectLeft(normCumWeights, targetPercentiles[i]);
                if (idx >= sortedD.Length) idx = sortedD.Length - 1;
                percentile83 += sortedD[idx];
            }

            percentile83 /= 4;

            double weightedMean = Math.Pow(sortedD.Zip(sortedWeights, (d, w) => Math.Pow(d, 5) * w).Sum() / sortedWeights.Sum(), 1.0 / 5);

            double SR = (0.88 * percentile93 * 0.25) + (0.94 * percentile83 * 0.2) + (weightedMean * 0.55);
            Console.WriteLine($"Before scaling: percentile_93={percentile93}, percentile_83={percentile83}, weighted_mean={weightedMean}, sr={SR}");
            SR = Math.Pow(SR, 1.0) / Math.Pow(8, 1.0) * 8;
            Console.WriteLine($"After power scaling: sr={SR}");

            // Match Python implementation: total_notes = len(note_seq) + 0.5*sum(min(t-h, 1000)/200 for LN)
            double totalNotes = noteSeq.Length;
            foreach (SRsNote ln in LNSeq)
            {
                double lnLength = Math.Min(ln.EndTime - ln.StartTime, 1000);
                totalNotes += 0.5 * (lnLength / 200.0);
            }
            SR *= totalNotes / (totalNotes + 60);

            SR =  RescaleHigh(SR);
            SR *= 0.975;
            Console.WriteLine($"Final sr: {SR}");

            return SR;
        }

        /// <summary>
        ///     从.osu文件路径计算SR（C#版本）
        /// </summary>
        /// <param name="filePath">.osu文件路径</param>
        /// <returns>SR值</returns>
        public double CalculateSRFromFileCS(string filePath)
        {
            var     decoder = new LegacyBeatmapDecoder();
            Beatmap beatmap = decoder.Decode(filePath);
            Console.WriteLine($"C# parsed {beatmap.HitObjects.Count} hit objects");
            return CalculateSR(beatmap, out _);
        }

        /// <summary>
        ///     从.osu文件内容计算SR（C#版本）
        /// </summary>
        /// <param name="content">.osu文件内容</param>
        /// <returns>SR值</returns>
        public double CalculateSRFromContentCS(string content)
        {
            var       decoder = new LegacyBeatmapDecoder();
            using var stream  = new MemoryStream(Encoding.UTF8.GetBytes(content));
            Beatmap   beatmap = decoder.Decode(stream);
            Console.WriteLine($"C# parsed {beatmap.HitObjects.Count} hit objects");
            return CalculateSR(beatmap, out _);
        }

        private double[] ComputeAnchor(int K, double[][] keyUsage400, double[] baseCorners)
        {
            double[] anchor = new double[baseCorners.Length];
            for (int idx = 0; idx < baseCorners.Length; idx++)
            {
                double[] counts = new double[K];
                for (int k = 0; k < K; k++)
                    counts[k] = keyUsage400[k][idx];
                Array.Sort(counts);
                Array.Reverse(counts);
                double[] nonzero = counts.Where(c => c != 0).ToArray();
                if (nonzero.Length > 1)
                {
                    double walk = 0;
                    for (int i = 0; i < nonzero.Length - 1; i++)
                    {
                        double ratio = nonzero[i + 1] / nonzero[i];
                        walk += nonzero[i] * (1 - 4 * Math.Pow((0.5 - ratio), 2));
                    }
                    double maxWalk = nonzero.Sum();
                    anchor[idx] = walk / maxWalk;
                }
                else
                {
                    anchor[idx] = 0;
                }
            }
            for (int idx = 0; idx < anchor.Length; idx++)
                anchor[idx] = 1 + Math.Min(anchor[idx] - 0.18, 5 * Math.Pow(anchor[idx] - 0.22, 3));
            return anchor;
        }

        private (double[], double[], double[]) GetCorners(int T, SRsNote[] noteSeq)
        {
            var cornersBase = new HashSet<double>();
            foreach (var note in noteSeq)
            {
                cornersBase.Add(note.StartTime);
                if (note.EndTime >= 0)
                    cornersBase.Add(note.EndTime);
            }
            var temp = cornersBase.ToList();
            foreach (double s in temp)
            {
                cornersBase.Add(s + 501);
                cornersBase.Add(s - 499);
                cornersBase.Add(s + 1);
            }
            cornersBase.Add(0);
            cornersBase.Add(T);
            var baseCorners = cornersBase.Where(s => s >= 0 && s <= T).OrderBy(s => s).ToArray();
            var cornersA = new HashSet<double>();
            foreach (var note in noteSeq)
            {
                cornersA.Add(note.StartTime);
                if (note.EndTime >= 0)
                    cornersA.Add(note.EndTime);
            }
            temp = cornersA.ToList();
            foreach (double s in temp)
            {
                cornersA.Add(s + 1000);
                cornersA.Add(s - 1000);
            }
            cornersA.Add(0);
            cornersA.Add(T);
            var A_corners = cornersA.Where(s => s >= 0 && s <= T).OrderBy(s => s).ToArray();
            var allCorners = baseCorners.Union(A_corners).OrderBy(s => s).ToArray();
            return (allCorners, baseCorners, A_corners);
        }

        private double[][] GetKeyUsage400(int K, int T, SRsNote[] noteSeq, double[] baseCorners)
        {
            var keyUsage400 = new double[K][];
            for (int k = 0; k < K; k++)
                keyUsage400[k] = new double[baseCorners.Length];
            foreach (var note in noteSeq)
            {
                int k = note.Index;
                double startTime = Math.Max(note.StartTime, 0);
                double endTime = note.EndTime >= 0 ? Math.Min(note.EndTime, T - 1) : note.StartTime + 1500;
                int left400_idx = Array.BinarySearch(baseCorners, startTime - 400);
                if (left400_idx < 0) left400_idx = ~left400_idx;
                int left_idx = Array.BinarySearch(baseCorners, startTime);
                if (left_idx < 0) left_idx = ~left_idx;
                int right_idx = Array.BinarySearch(baseCorners, endTime);
                if (right_idx < 0) right_idx = ~right_idx;
                int right400_idx = Array.BinarySearch(baseCorners, endTime + 400);
                if (right400_idx < 0) right400_idx = ~right400_idx;
                for (int idx = left_idx; idx < right_idx; idx++)
                {
                    double lnLength = Math.Min(endTime - startTime, 1500);
                    keyUsage400[k][idx] += (3.75 + (lnLength / 150));
                }
                for (int idx = left400_idx; idx < left_idx; idx++)
                {
                    double dist = startTime - baseCorners[idx];
                    keyUsage400[k][idx] += 3.75 - ((3.75 / (400 * 400)) * dist * dist);
                }
                for (int idx = right_idx; idx < right400_idx; idx++)
                {
                    double dist = baseCorners[idx] - endTime;
                    keyUsage400[k][idx] += 3.75 - ((3.75 / (400 * 400)) * dist * dist);
                }
            }
            return keyUsage400;
        }

        private double[] InterpValues(double[] newX, double[] oldX, double[] oldVals)
        {
            double[] result = new double[newX.Length];
            for (int i = 0; i < newX.Length; i++)
            {
                double x = newX[i];
                int idx = Array.BinarySearch(oldX, x);
                if (idx >= 0)
                    result[i] = oldVals[idx];
                else
                {
                    idx = ~idx;
                    if (idx == 0)
                        result[i] = oldVals[0];
                    else if (idx == oldX.Length)
                        result[i] = oldVals[oldX.Length - 1];
                    else
                    {
                        double x0 = oldX[idx - 1];
                        double x1 = oldX[idx];
                        double y0 = oldVals[idx - 1];
                        double y1 = oldVals[idx];
                        result[i] = y0 + ((y1 - y0) * (x - x0)) / (x1 - x0);
                    }
                }
            }
            return result;
        }
    }
}
