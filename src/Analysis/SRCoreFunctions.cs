using System;
using System.Collections.Generic;
using System.Linq;

namespace LAsOsuBeatmapParser.Analysis
{
    /// <summary>
    /// Core SR calculation functions, rewritten from Python algorithm.py
    /// </summary>
    public static class SRCoreFunctions
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

        // Helper methods

        private static double[] CumulativeSum(double[] x, double[] f)
        {
            double[] F = new double[x.Length];

            for (int i = 1; i < x.Length; i++)
            {
                F[i] = F[i - 1] + f[i - 1] * (x[i] - x[i - 1]);
            }

            return F;
        }

        private static double QueryCumsum(double q, double[] x, double[] F, double[] f)
        {
            if (q <= x[0]) return 0.0;
            if (q >= x[x.Length - 1]) return F[F.Length - 1];
            int i = Array.BinarySearch(x, q);
            if (i < 0) i = ~i - 1;
            return F[i] + f[i] * (q - x[i]);
        }

        private static double[] SmoothOnCorners(double[] x, double[] f, double window, double scale = 1.0, string mode = "sum")
        {
            double[] F = CumulativeSum(x, f);
            double[] g = new double[f.Length];

            for (int i = 0; i < x.Length; i++)
            {
                double s = x[i];
                double a = Math.Max(s - window, x[0]);
                double b = Math.Min(s + window, x[x.Length - 1]);
                double val = QueryCumsum(b, x, F, f) - QueryCumsum(a, x, F, f);

                if (mode == "avg")
                {
                    g[i] = val / (b - a) > 0 ? val / (b - a) : 0.0;
                }
                else
                {
                    g[i] = scale * val;
                }
            }

            return g;
        }

        public static double[] InterpValues(double[] newX, double[] oldX, double[] oldVals)
        {
            double[] result = new double[newX.Length];

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
                        result[i] = y0 + (y1 - y0) * (x - x0) / (x1 - x0);
                    }
                }
            }

            return result;
        }

        public static double[] StepInterp(double[] newX, double[] oldX, double[] oldVals)
        {
            double[] result = new double[newX.Length];

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
                    idx = ~idx - 1;

                    if (idx < 0)
                    {
                        result[i] = 0;
                    }
                    else
                    {
                        result[i] = oldVals[idx];
                    }
                }
            }

            return result;
        }

        private static double JackNerfer(double delta)
        {
            return 1 - 7e-5 * Math.Pow(0.15 + Math.Abs(delta - 0.08), -4);
        }

        private static double StreamBooster(double delta)
        {
            double val = 7.5 / delta;

            if (160 < val && val < 360)
            {
                return 1 + 1.7e-7 * (val - 160) * Math.Pow(val - 360, 2);
            }

            return 1;
        }

        // Core functions

        public static (double[][][], double[]) ComputeJbar(int K, int T, double x, SRsNote[][] noteSeqByColumn, double[] baseCorners)
        {
            double[][][] deltaKs = new double[K][][]; // k -> time -> k (but in Python it's k -> time)

            for (int k = 0; k < K; k++)
            {
                deltaKs[k] = new double[baseCorners.Length][];

                for (int j = 0; j < baseCorners.Length; j++)
                {
                    deltaKs[k][j] = new double[K];
                    Array.Fill(deltaKs[k][j], 1e9);
                }
            }

            double[][] JKs = new double[K][];

            for (int k = 0; k < K; k++)
            {
                JKs[k] = new double[baseCorners.Length];
            }

            for (int k = 0; k < K; k++)
            {
                var notes = noteSeqByColumn[k];

                for (int i = 0; i < notes.Length - 1; i++)
                {
                    int start = notes[i].StartTime;
                    int end = notes[i + 1].StartTime;
                    int leftIdx = Array.BinarySearch(baseCorners, start);
                    if (leftIdx < 0) leftIdx = ~leftIdx;
                    int rightIdx = Array.BinarySearch(baseCorners, end);
                    if (rightIdx < 0) rightIdx = ~rightIdx;

                    for (int idx = leftIdx; idx < rightIdx; idx++)
                    {
                        double delta = 0.001 * (end - start);
                        double val = 1.0 / delta * 1.0 / (delta + 0.11 * Math.Pow(x, 0.25));
                        double JVal = val * JackNerfer(delta);
                        JKs[k][idx] = JVal;
                        deltaKs[k][idx][k] = delta; // Wait, in Python delta_ks[k][idx] = delta, but it's per k
                        // Actually, delta_ks is {k: array}, so deltaKs[k][idx] = delta
                        // But in Python it's delta_ks[k][idx] = delta, and used as delta_ks[k0][i]
                        // So deltaKs[k][idx] = delta
                    }
                }
            }

            // Smooth
            double[][] JbarKs = new double[K][];

            for (int k = 0; k < K; k++)
            {
                JbarKs[k] = SmoothOnCorners(baseCorners, JKs[k], 500, 0.001, "sum");
            }

            // Aggregate
            double[] Jbar = new double[baseCorners.Length];

            for (int i = 0; i < baseCorners.Length; i++)
            {
                double num = 0;
                double den = 0;

                for (int k = 0; k < K; k++)
                {
                    double v = Math.Max(JbarKs[k][i], 0);
                    double w = 1.0 / deltaKs[k][i][k]; // Assuming deltaKs[k][i][k] is set
                    num += Math.Pow(v, 5) * w;
                    den += w;
                }

                Jbar[i] = Math.Pow(num / Math.Max(1e-9, den), 1.0 / 5);
            }

            return (deltaKs, Jbar);
        }

        public static double[] ComputeXbar(int K, int T, double x, SRsNote[][] noteSeqByColumn, List<int>[] activeColumns, double[] baseCorners)
        {
            double[] crossMatrix = CrossMatrixProvider.GetMatrix(K);
            double[][] XKs = new double[K + 1][];

            for (int k = 0; k <= K; k++)
            {
                XKs[k] = new double[baseCorners.Length];
            }

            double[][] fastCross = new double[K + 1][];

            for (int k = 0; k <= K; k++)
            {
                fastCross[k] = new double[baseCorners.Length];
            }

            for (int k = 0; k <= K; k++)
            {
                int kk = k > 11 ? 11 : k;
                SRsNote[] notesInPair;

                if (k == 0)
                {
                    notesInPair = noteSeqByColumn[0];
                }
                else if (k == K)
                {
                    notesInPair = noteSeqByColumn[K - 1];
                }
                else
                {
                    // Merge two columns
                    var list = new List<SRsNote>();
                    list.AddRange(noteSeqByColumn[k - 1]);
                    list.AddRange(noteSeqByColumn[k]);
                    list.Sort((a, b) => a.StartTime.CompareTo(b.StartTime));
                    notesInPair = list.ToArray();
                }

                for (int i = 1; i < notesInPair.Length; i++)
                {
                    int start = notesInPair[i - 1].StartTime;
                    int end = notesInPair[i].StartTime;
                    int idxStart = Array.BinarySearch(baseCorners, start);
                    if (idxStart < 0) idxStart = ~idxStart;
                    int idxEnd = Array.BinarySearch(baseCorners, end);
                    if (idxEnd < 0) idxEnd = ~idxEnd;

                    for (int idx = idxStart; idx < idxEnd; idx++)
                    {
                        double delta = 0.001 * (end - start);
                        double val = 0.16 * Math.Pow(Math.Max(x, delta), -2);

                        if ((k - 1 < 0 || !activeColumns[idx].Contains(k - 1)) && (k >= K || !activeColumns[idx].Contains(k)))
                        {
                            val *= (1 - crossMatrix[kk]);
                        }

                        XKs[k][idx] = val;
                        fastCross[k][idx] = Math.Max(0, 0.4 * Math.Pow(Math.Max(Math.Max(delta, 0.06), 0.75 * x), -2) - 80);
                    }
                }
            }

            double[] XBase = new double[baseCorners.Length];

            for (int i = 0; i < baseCorners.Length; i++)
            {
                double sum1 = 0;

                for (int k = 0; k <= K; k++)
                {
                    sum1 += XKs[k][i] * crossMatrix[k];
                }

                double sum2 = 0;

                for (int k = 0; k < K; k++)
                {
                    sum2 += Math.Sqrt(fastCross[k][i] * crossMatrix[k] * fastCross[k + 1][i] * crossMatrix[k + 1]);
                }

                XBase[i] = sum1 + sum2;
            }

            double[] Xbar = SmoothOnCorners(baseCorners, XBase, 500, 0.001, "sum");
            return Xbar;
        }

        public static double[] ComputePbar(int K, int T, double x, SRsNote[] noteSeq, (double[], double[], double[]) LNRep, double[] anchor, double[] baseCorners)
        {
            var crossMatrix = CrossMatrixProvider.GetMatrix(K);
            double[] PStep = new double[baseCorners.Length];

            for (int i = 0; i < noteSeq.Length - 1; i++)
            {
                double hL = noteSeq[i].StartTime;
                double hR = noteSeq[i + 1].StartTime;
                double deltaTime = hR - hL;

                if (deltaTime < 1e-9)
                {
                    // Dirac delta
                    double spike = 1000 * Math.Pow(0.02 * (4 / x - 24), 0.25);
                    int idx = Array.BinarySearch(baseCorners, hL);

                    if (idx >= 0)
                    {
                        PStep[idx] += spike;
                    }

                    continue;
                }

                int leftIdx2 = Array.BinarySearch(baseCorners, hL);
                if (leftIdx2 < 0) leftIdx2 = ~leftIdx2;
                int rightIdx2 = Array.BinarySearch(baseCorners, hR);
                if (rightIdx2 < 0) rightIdx2 = ~rightIdx2;

                for (int idx = leftIdx2; idx < rightIdx2; idx++)
                {
                    double delta = 0.001 * deltaTime;
                    double v = 1 + 6 * 0.001 * LN_sum(hL, hR, LNRep);
                    double bVal = StreamBooster(delta);
                    double inc;

                    if (delta < 2 * x / 3)
                    {
                        inc = 1.0 / delta * Math.Pow(0.08 / x * (1 - 24 / x * Math.Pow(delta - x / 2, 2)), 0.25) * Math.Max(bVal, v);
                    }
                    else
                    {
                        inc = 1.0 / delta * Math.Pow(0.08 / x * (1 - 24 / x * Math.Pow(x / 6, 2)), 0.25) * Math.Max(bVal, v);
                    }

                    PStep[idx] += Math.Min(inc * anchor[idx], Math.Max(inc, inc * 2 - 10));
                }
            }

            double[] Pbar = SmoothOnCorners(baseCorners, PStep, 500, 0.001, "sum");
            return Pbar;
        }

        public static double[] ComputeAbar(int K, int T, double x, SRsNote[][] noteSeqByColumn, List<int>[] activeColumns, double[][][] deltaKs, double[] A_corners, double[] baseCorners)
        {
            double[][] dks = new double[K][];

            for (int i = 0; i < K; i++)
            {
                dks[i] = new double[baseCorners.Length];
            }

            for (int i = 0; i < baseCorners.Length; i++)
            {
                var cols = activeColumns[i];

                for (int j = 0; j < cols.Count - 1; j++)
                {
                    int k0 = cols[j];
                    int k1 = cols[j + 1];
                    dks[k0][i] = Math.Abs(deltaKs[k0][i][k0] - deltaKs[k1][i][k1]) + 0.4 * Math.Max(0, Math.Max(deltaKs[k0][i][k0], deltaKs[k1][i][k1]) - 0.11);
                }
            }

            double[] AStep = new double[A_corners.Length];
            Array.Fill(AStep, 1.0);

            double[][] AKs = new double[K][];

            for (int k = 0; k < K; k++)
            {
                AKs[k] = new double[baseCorners.Length];
            }

            for (int k = 0; k < K; k++)
            {
                for (int i = 0; i < baseCorners.Length; i++)
                {
                    double delta = deltaKs[k][i][k];
                    if (delta > 1e8) continue;
                    double aVal = 1.0 / (delta + 0.5);
                    AKs[k][i] = aVal;
                }
            }

            for (int i = 0; i < A_corners.Length; i++)
            {
                double s = A_corners[i];
                int idx = Array.BinarySearch(baseCorners, s);
                if (idx < 0) idx = ~idx;
                if (idx >= baseCorners.Length) idx = baseCorners.Length - 1;
                var cols = activeColumns[idx];

                for (int j = 0; j < cols.Count - 1; j++)
                {
                    int k0 = cols[j];
                    int k1 = cols[j + 1];
                    double dVal = dks[k0][idx];

                    if (dVal < 0.02)
                    {
                        AStep[i] *= Math.Min(0.75 + 0.5 * Math.Max(deltaKs[k0][idx][k0], deltaKs[k1][idx][k1]), 1);
                    }
                    else if (dVal < 0.07)
                    {
                        AStep[i] *= Math.Min(0.65 + 5 * dVal + 0.5 * Math.Max(deltaKs[k0][idx][k0], deltaKs[k1][idx][k1]), 1);
                    }
                }
            }

            double[] ABase = new double[A_corners.Length];
            double[][] AKsInterp = new double[K][];

            for (int k = 0; k < K; k++)
            {
                AKsInterp[k] = InterpValues(A_corners, baseCorners, AKs[k]);
            }

            for (int i = 0; i < A_corners.Length; i++)
            {
                double sum = 0;

                for (int k = 0; k < K; k++)
                {
                    sum += AKsInterp[k][i];
                }

                ABase[i] = sum;
            }

            double[] Abar = SmoothOnCorners(A_corners, AStep, 250, 1.0, "avg");
            return Abar;
        }

        public static double[] ComputeRbar(int K, int T, double x, SRsNote[][] noteSeqByColumn, SRsNote[] tailSeq, double[] baseCorners)
        {
            double[] RStep = new double[baseCorners.Length];

            double[] IArr = new double[baseCorners.Length];
            double[] IList = new double[tailSeq.Length];
            int[][] timesByColumn = new int[K][];

            for (int i = 0; i < K; i++)
            {
                timesByColumn[i] = noteSeqByColumn[i].Select(n => n.StartTime).ToArray();
            }

            for (int i = 0; i < tailSeq.Length; i++)
            {
                int k = tailSeq[i].Index;
                int hI = tailSeq[i].StartTime;
                int tI = tailSeq[i].EndTime;
                int idx = Array.BinarySearch(timesByColumn[k], hI);
                if (idx < 0) idx = ~idx;
                int hJ = idx + 1 < timesByColumn[k].Length ? timesByColumn[k][idx + 1] : int.MaxValue;
                double I_h = 0.001 * Math.Abs(tI - hI - 80) / x;
                double I_t = 0.001 * Math.Abs(hJ - tI - 80) / x;
                IList[i] = 2 / (2 + Math.Exp(-5 * (I_h - 0.75)) + Math.Exp(-5 * (I_t - 0.75)));
            }

            for (int i = 0; i < tailSeq.Length - 1; i++)
            {
                int tStart = tailSeq[i].EndTime;
                int tEnd = tailSeq[i + 1].EndTime;
                int leftIdx = Array.BinarySearch(baseCorners, tStart);
                if (leftIdx < 0) leftIdx = ~leftIdx;
                int rightIdx = Array.BinarySearch(baseCorners, tEnd);
                if (rightIdx < 0) rightIdx = ~rightIdx;

                for (int idx = leftIdx; idx < rightIdx; idx++)
                {
                    IArr[idx] = 1 + IList[i];
                    double deltaR = 0.001 * (tEnd - tStart);
                    RStep[idx] = 0.08 * Math.Pow(deltaR, -0.5) / x * (1 + 0.8 * (IList[i] + IList[i + 1]));
                }
            }

            double[] Rbar = SmoothOnCorners(baseCorners, RStep, 500, 0.001, "sum");
            return Rbar;
        }

        public static double CalculateFinalSR(double[] dAll, double[] effectiveWeights, SRsNote[] noteSeq, SRsNote[] lnSeq)
        {
            // Sort D by value, keep weights
            var pairs = new List<(double d, double w)>();

            for (int i = 0; i < dAll.Length; i++)
            {
                pairs.Add((dAll[i], effectiveWeights[i]));
            }

            pairs.Sort((a, b) => a.d.CompareTo(b.d));

            var sortedD = pairs.Select(p => p.d).ToArray();
            var sortedWeights = pairs.Select(p => p.w).ToArray();

            // Cumulative weights
            var cumWeights = new double[sortedWeights.Length];
            cumWeights[0] = sortedWeights[0];

            for (int i = 1; i < cumWeights.Length; i++)
            {
                cumWeights[i] = cumWeights[i - 1] + sortedWeights[i];
            }

            double totalWeight = cumWeights[cumWeights.Length - 1];
            var normCumWeights = cumWeights.Select(cw => cw / totalWeight).ToArray();

            var targetPercentiles = new double[] { 0.945, 0.935, 0.925, 0.915, 0.845, 0.835, 0.825, 0.815 };

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

            double sr = (0.88 * percentile93 * 0.25) + (0.94 * percentile83 * 0.2) + (weightedMean * 0.55);
            sr = Math.Pow(sr, 1.0) / Math.Pow(8, 1.0) * 8;

            // Match Python implementation: total_notes = len(note_seq) + 0.5*sum(min(t-h, 1000)/200 for LN)
            double totalNotes = noteSeq.Length;

            foreach (var ln in lnSeq)
            {
                double lnLength = Math.Min(ln.EndTime - ln.StartTime, 1000);
                totalNotes += 0.5 * (lnLength / 200.0);
            }

            sr *= totalNotes / (totalNotes + 60);

            sr = RescaleHigh(sr);
            sr *= 0.975;

            return sr;
        }

        private static double RescaleHigh(double sr)
        {
            if (sr <= 9)
                return sr;
            return 9 + (sr - 9) * (1 / 1.2);
        }

        private static int BisectLeft(double[] arr, double x)
        {
            int low = 0;
            int high = arr.Length;

            while (low < high)
            {
                int mid = (low + high) / 2;
                if (arr[mid] < x)
                    low = mid + 1;
                else
                    high = mid;
            }

            return low;
        }

        public static (double[], double[], double[]) LN_bodies_count_sparse_representation(SRsNote[] LN_seq, int T)
        {
            var diff = new Dictionary<int, double>();

            foreach (var ln in LN_seq)
            {
                int k = ln.Index;
                int h = ln.StartTime;
                int t = ln.EndTime;
                int t0 = Math.Min(h + 60, t);
                int t1 = Math.Min(h + 120, t);
                diff[t0] = diff.GetValueOrDefault(t0, 0) + 1.3;
                diff[t1] = diff.GetValueOrDefault(t1, 0) + (-1.3 + 1);
                diff[t] = diff.GetValueOrDefault(t, 0) - 1;
            }

            var points = new HashSet<int> { 0, T };

            foreach (var key in diff.Keys)
            {
                points.Add(key);
            }

            var pointsList = points.OrderBy(p => p).ToList();

            var values = new List<double>();
            var cumsum = new List<double> { 0 };
            double curr = 0.0;

            for (int i = 0; i < pointsList.Count - 1; i++)
            {
                int t = pointsList[i];

                if (diff.ContainsKey(t))
                {
                    curr += diff[t];
                }

                double v = Math.Min(curr, 2.5 + 0.5 * curr);
                values.Add(v);
                double seg_length = pointsList[i + 1] - pointsList[i];
                cumsum.Add(cumsum.Last() + seg_length * v);
            }

            return (pointsList.Select(p => (double)p).ToArray(), cumsum.ToArray(), values.ToArray());
        }

        private static double LN_sum(double a, double b, (double[], double[], double[]) LN_rep)
        {
            var (points, cumsum, values) = LN_rep;
            int i = Array.BinarySearch(points, a);
            if (i < 0) i = ~i - 1;
            int j = Array.BinarySearch(points, b);
            if (j < 0) j = ~j - 1;

            double total = 0.0;

            if (i == j)
            {
                if (i >= 0 && i < values.Length)
                {
                    total = (b - a) * values[i];
                }
                // else total = 0
            }
            else
            {
                if (i >= 0 && i < values.Length && i + 1 < points.Length)
                {
                    total += (points[i + 1] - a) * values[i];
                }

                if (i + 1 <= j && j < cumsum.Length && i + 1 < cumsum.Length)
                {
                    total += cumsum[j] - cumsum[i + 1];
                }

                if (j >= 0 && j < values.Length && j < points.Length)
                {
                    total += (b - points[j]) * values[j];
                }
            }

            return total;
        }
    }
}
