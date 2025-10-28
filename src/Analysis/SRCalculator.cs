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
        private const double w_0 = 0.4;
        private const double w_1 = 2.7;
        private const double p_1 = 1.5;
        private const double w_2 = 0.27;
        private const double p_0 = 1.0;

        private const int granularity = 1; // 只能保持为1，确保精度不变，不可修改

        private SRCalculator() { } // 私有构造函数

        /// <summary>
        ///     单例模式：无状态类，线程安全，高并发不建议使用
        /// </summary>
        public static SRCalculator Instance { get; } = new SRCalculator();

        /// <summary>
        ///     从.osu文件路径计算SR（C#版本）
        /// </summary>
        /// <param name="filePath">.osu文件路径</param>
        /// <returns>SR值</returns>
        public double CalculateSRFromFileCS(string filePath)
        {
            var decoder = new LegacyBeatmapDecoder();
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
            var decoder = new LegacyBeatmapDecoder();
            using var stream = new MemoryStream(Encoding.UTF8.GetBytes(content));
            Beatmap beatmap = decoder.Decode(stream);
            Console.WriteLine($"C# parsed {beatmap.HitObjects.Count} hit objects");
            return CalculateSR(beatmap, out _);
        }

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
            times = t;
            return sr;
        }

        /// <summary>
        ///     异步SR计算核心，使用全局算法
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
                x = Math.Min(x, (0.6 * (x - 0.09)) + 0.09);
                SRsNote[][] noteSeqByColumn = new SRsNote[keyCount][];

                for (int k = 0; k < keyCount; k++)
                {
                    noteSeqByColumn[k] = new SRsNote[0];
                }

                foreach (var group in noteSeq.GroupBy(n => n.Index))
                {
                    noteSeqByColumn[group.Key] = group.ToArray();
                }

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

                // Calculate T
                int totalTime = Math.Max(noteSeq.Max(n => n.StartTime), noteSeq.Max(n => n.EndTime)) + 1;

                // Global algorithm starts here
                var (allCorners, baseCorners, A_corners) = GetCorners(totalTime, noteSeq);

                var keyUsage = GetKeyUsage(keyCount, totalTime, noteSeq, baseCorners);
                var activeColumns = GetActiveColumns(keyCount, keyUsage);

                var keyUsage400 = GetKeyUsage400Global(keyCount, totalTime, noteSeq, baseCorners);
                var anchorBase = ComputeAnchorGlobal(keyCount, keyUsage400, baseCorners);
                var anchor = InterpValuesGlobal(allCorners, baseCorners, anchorBase);

                var lnRep = SRCoreFunctions.LN_bodies_count_sparse_representation(LNSeq, totalTime);

                var (deltaKs, jbarBase) = SRCoreFunctions.ComputeJbar(keyCount, totalTime, x, noteSeqByColumn, baseCorners);
                var jbar = SRCoreFunctions.InterpValues(allCorners, baseCorners, jbarBase);

                var xbarBase = SRCoreFunctions.ComputeXbar(keyCount, totalTime, x, noteSeqByColumn, activeColumns, baseCorners);
                var xbar = SRCoreFunctions.InterpValues(allCorners, baseCorners, xbarBase);

                var pbarBase = SRCoreFunctions.ComputePbar(keyCount, totalTime, x, noteSeq, lnRep, anchorBase, baseCorners);
                var pbar = SRCoreFunctions.InterpValues(allCorners, baseCorners, pbarBase);

                var abarBase = SRCoreFunctions.ComputeAbar(keyCount, totalTime, x, noteSeqByColumn, activeColumns, deltaKs, A_corners, baseCorners);
                var abar = SRCoreFunctions.InterpValues(allCorners, A_corners, abarBase);

                var rbarBase = SRCoreFunctions.ComputeRbar(keyCount, totalTime, x, noteSeqByColumn, tailSeq, baseCorners);
                var rbar = SRCoreFunctions.InterpValues(allCorners, baseCorners, rbarBase);

                var (cArr, ksArr) = ComputeCAndKs(keyCount, totalTime, noteSeq, keyUsage, baseCorners);
                var cArrInterp = SRCoreFunctions.StepInterp(allCorners, baseCorners, cArr);
                var ksArrInterp = SRCoreFunctions.StepInterp(allCorners, baseCorners, ksArr);

                // Compute gaps
                var gaps = ComputeGaps(allCorners);

                // Effective weights
                var effectiveWeights = new double[allCorners.Length];

                for (int i = 0; i < allCorners.Length; i++)
                {
                    effectiveWeights[i] = cArrInterp[i] * gaps[i];
                }

                // Compute D_all
                var dAll = new double[allCorners.Length];

                for (int i = 0; i < allCorners.Length; i++)
                {
                    double term1 = 0.4 * Math.Pow(Math.Pow(abar[i], 3.0 / ksArrInterp[i]) * Math.Min(jbar[i], 8 + (0.85 * jbar[i])), 1.5);
                    double term2 = (1 - 0.4) * Math.Pow(Math.Pow(abar[i], 2.0 / 3) * ((0.8 * pbar[i]) + (rbar[i] * 35 / (cArrInterp[i] + 8))), 1.5);
                    double s = Math.Pow(term1 + term2, 2.0 / 3);
                    double t_val = Math.Pow(abar[i], 3.0 / ksArrInterp[i]) * xbar[i] / (xbar[i] + s + 1);
                    dAll[i] = (2.7 * Math.Pow(s, 0.5) * Math.Pow(t_val, 1.5)) + (s * 0.27);
                }

                double sr = SRCoreFunctions.CalculateFinalSR(dAll, effectiveWeights, noteSeq, LNSeq);

                totalStopwatch.Stop();
                times["TotalTime"] = totalStopwatch.ElapsedMilliseconds;

                return (sr, times);
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Error in SR calculation: {ex.Message}");
                Console.WriteLine(ex.StackTrace);
                throw; // Throw the exception instead of returning -1
            }
        }

        // Global algorithm helper methods

        private static double[] GetAllCorners(int totalTime)
        {
            var corners = new List<double>();

            for (int i = 0; i < totalTime; i += granularity)
            {
                corners.Add(i);
            }

            return corners.ToArray();
        }

        private static double[] GetBaseCorners(int totalTime)
        {
            var corners = new List<double>();

            for (int i = 0; i < totalTime; i++)
            {
                corners.Add(i);
            }

            return corners.ToArray();
        }

        private static bool[][] GetKeyUsage(int k, int t, SRsNote[] noteSeq, double[] baseCorners)
        {
            var keyUsage = new bool[k][];

            for (int i = 0; i < k; i++)
            {
                keyUsage[i] = new bool[baseCorners.Length];
            }

            foreach (var note in noteSeq)
            {
                int col = note.Index;
                int startTime = note.StartTime;
                int endTime = note.EndTime >= 0 ? Math.Min(note.EndTime + 150, t - 1) : startTime + 150;
                int startIdx = Array.BinarySearch(baseCorners, Math.Max(startTime - 150, 0));
                if (startIdx < 0) startIdx = ~startIdx;
                int endIdx = Array.BinarySearch(baseCorners, endTime);
                if (endIdx < 0) endIdx = ~endIdx;

                for (int idx = startIdx; idx < endIdx; idx++)
                {
                    if (idx >= 0 && idx < baseCorners.Length)
                        keyUsage[col][idx] = true;
                }
            }

            return keyUsage;
        }

        private static List<int>[] GetActiveColumns(int k, bool[][] keyUsage)
        {
            var activeColumns = new List<int>[keyUsage[0].Length];

            for (int i = 0; i < activeColumns.Length; i++)
            {
                activeColumns[i] = new List<int>();

                for (int j = 0; j < k; j++)
                {
                    if (keyUsage[j][i])
                    {
                        activeColumns[i].Add(j);
                    }
                }
            }

            return activeColumns;
        }

        private static double[][] GetKeyUsage400(int k, int t, SRsNote[] noteSeq, double[] baseCorners)
        {
            var keyUsage400 = new double[k][];

            for (int i = 0; i < k; i++)
            {
                keyUsage400[i] = new double[baseCorners.Length];
            }

            foreach (var note in noteSeq)
            {
                int col = note.Index;
                int startTime = note.StartTime;
                int endTime = note.EndTime >= 0 ? note.EndTime : startTime;

                int startIdx = Math.Max(0, startTime - 400);
                int endIdx = Math.Min(t - 1, endTime + 400);

                for (int time = startIdx; time <= endIdx; time++)
                {
                    keyUsage400[col][time] = 1.0;
                }
            }

            return keyUsage400;
        }

        private static double[] ComputeAnchor(int k, double[][] keyUsage400, double[] baseCorners)
        {
            var anchor = new double[baseCorners.Length];
            var crossMatrix = CrossMatrixProvider.GetMatrix(k);

            for (int i = 0; i < baseCorners.Length; i++)
            {
                double sum = 0;

                for (int j = 0; j < k; j++)
                {
                    sum += keyUsage400[j][i] * crossMatrix[j];
                }

                anchor[i] = sum;
            }

            return anchor;
        }

        private static (double[], double[]) ComputeCAndKs(int k, int t, SRsNote[] noteSeq, bool[][] keyUsage, double[] baseCorners)
        {
            var cArr = new double[baseCorners.Length];
            var ksArr = new double[baseCorners.Length];

            for (int i = 0; i < baseCorners.Length; i++)
            {
                int count = 0;

                for (int j = 0; j < k; j++)
                {
                    if (keyUsage[j][i])
                    {
                        count++;
                    }
                }

                cArr[i] = count;
                ksArr[i] = count;
            }

            return (cArr, ksArr);
        }

        private static double[] ComputeGaps(double[] allCorners)
        {
            var gaps = new double[allCorners.Length];
            if (allCorners.Length == 0) return gaps;

            gaps[0] = allCorners.Length > 1 ? (allCorners[1] - allCorners[0]) / 2.0 : allCorners[0] / 2.0;

            if (allCorners.Length > 1)
            {
                gaps[allCorners.Length - 1] = (allCorners[allCorners.Length - 1] - allCorners[allCorners.Length - 2]) / 2.0;

                for (int i = 1; i < allCorners.Length - 1; i++)
                {
                    gaps[i] = (allCorners[i + 1] - allCorners[i - 1]) / 2.0;
                }
            }

            return gaps;
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

        private static double[][] GetKeyUsage400Global(int k, int t, SRsNote[] noteSeq, double[] baseCorners)
        {
            var keyUsage400 = new double[k][];

            for (int i = 0; i < k; i++)
            {
                keyUsage400[i] = new double[baseCorners.Length];
            }

            foreach (var note in noteSeq)
            {
                int col = note.Index;
                int startTime = note.StartTime;
                int endTime = note.EndTime >= 0 ? Math.Min(note.EndTime, t - 1) : startTime;
                int left400Idx = Array.BinarySearch(baseCorners, startTime - 400);
                if (left400Idx < 0) left400Idx = ~left400Idx;
                int leftIdx = Array.BinarySearch(baseCorners, startTime);
                if (leftIdx < 0) leftIdx = ~leftIdx;
                int rightIdx = Array.BinarySearch(baseCorners, endTime);
                if (rightIdx < 0) rightIdx = ~rightIdx;
                int right400Idx = Array.BinarySearch(baseCorners, endTime + 400);
                if (right400Idx < 0) right400Idx = ~right400Idx;

                // idx = np.arange(left_idx, right_idx)
                for (int idx = leftIdx; idx < rightIdx; idx++)
                {
                    if (idx >= 0 && idx < baseCorners.Length)
                    {
                        double lnLength = note.EndTime >= 0 ? note.EndTime - note.StartTime : 0;
                        keyUsage400[col][idx] += 3.75 + Math.Min(lnLength, 1500) / 150;
                    }
                }

                // idx = np.arange(left400_idx, left_idx)
                for (int idx = left400Idx; idx < leftIdx; idx++)
                {
                    if (idx >= 0 && idx < baseCorners.Length)
                    {
                        double dist = baseCorners[idx] - startTime;
                        keyUsage400[col][idx] += 3.75 - 3.75 / (400 * 400) * (dist * dist);
                    }
                }

                // idx = np.arange(right_idx, right400_idx)
                for (int idx = rightIdx; idx < right400Idx; idx++)
                {
                    if (idx >= 0 && idx < baseCorners.Length)
                    {
                        double dist = Math.Abs(baseCorners[idx] - endTime);
                        keyUsage400[col][idx] += 3.75 - 3.75 / (400 * 400) * (dist * dist);
                    }
                }
            }

            return keyUsage400;
        }

        private static double[] ComputeAnchorGlobal(int k, double[][] keyUsage400, double[] baseCorners)
        {
            var anchor = new double[baseCorners.Length];
            var crossMatrix = CrossMatrixProvider.GetMatrix(k);

            for (int i = 0; i < baseCorners.Length; i++)
            {
                double sum = 0;

                for (int j = 0; j < k; j++)
                {
                    sum += keyUsage400[j][i] * crossMatrix[j];
                }

                anchor[i] = sum;
            }

            return anchor;
        }

        private static double[] InterpValuesGlobal(double[] newX, double[] oldX, double[] oldVals)
        {
            var result = new double[newX.Length];

            for (int i = 0; i < newX.Length; i++)
            {
                double x = newX[i];
                int idx = Array.BinarySearch(oldX, x);

                if (idx >= 0)
                {
                    result[i] = oldVals[idx];
                }
                else
                {
                    idx = ~idx;

                    if (idx == 0)
                    {
                        result[i] = oldVals[0];
                    }
                    else if (idx >= oldX.Length)
                    {
                        result[i] = oldVals[oldX.Length - 1];
                    }
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
