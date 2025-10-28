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
    ///     Star Rating calculator – C# port of the official Python implementation.
    /// </summary>
    public sealed class SRCalculator
    {
        private SRCalculator() { }

        /// <summary>
        ///     Singleton entry point for SR calculations.
        /// </summary>
        public static SRCalculator Instance { get; } = new SRCalculator();

        /// <summary>
        ///     Computes the star rating for the supplied beatmap synchronously.
        /// </summary>
        /// <typeparam name="T">Hit object type contained in the beatmap.</typeparam>
        /// <param name="beatmap">Beatmap instance.</param>
        /// <param name="times">Timing breakdown produced by the calculation.</param>
        /// <returns>Calculated SR value.</returns>
        public double CalculateSR<T>(IBeatmap<T> beatmap, out Dictionary<string, long> times) where T : HitObject
        {
            (double sr, Dictionary<string, long> collectedTimes) = ComputeInternal(beatmap);
            times                                                = collectedTimes;
            return sr;
        }

        /// <summary>
        ///     Computes the star rating for the supplied beatmap asynchronously.
        /// </summary>
        /// <typeparam name="T">Hit object type contained in the beatmap.</typeparam>
        /// <param name="beatmap">Beatmap instance.</param>
        /// <returns>Tuple containing the SR value and timing breakdown.</returns>
        public Task<(double sr, Dictionary<string, long> times)> CalculateSRAsync<T>(IBeatmap<T> beatmap) where T : HitObject
        {
            return Task.FromResult(ComputeInternal(beatmap));
        }

        /// <summary>
        ///     Calculates SR directly from an osu! beatmap file path.
        /// </summary>
        /// <param name="filePath">Absolute path to an .osu file.</param>
        /// <returns>Calculated SR value.</returns>
        public double CalculateSRFromFileCS(string filePath)
        {
            var     decoder = new LegacyBeatmapDecoder();
            Beatmap beatmap = decoder.Decode(filePath);
            return CalculateSR(beatmap, out _);
        }

        /// <summary>
        ///     Calculates SR from raw beatmap content.
        /// </summary>
        /// <param name="content">String containing .osu file contents.</param>
        /// <returns>Calculated SR value.</returns>
        public double CalculateSRFromContentCS(string content)
        {
            var       decoder = new LegacyBeatmapDecoder();
            using var stream  = new MemoryStream(Encoding.UTF8.GetBytes(content));
            Beatmap   beatmap = decoder.Decode(stream);
            return CalculateSR(beatmap, out _);
        }

        private static (double sr, Dictionary<string, long> times) ComputeInternal<T>(IBeatmap<T> beatmap) where T : HitObject
        {
            var    stopwatch = Stopwatch.StartNew();
            double sr        = PythonPortAlgorithm.Calculate(beatmap);
            stopwatch.Stop();

            var timings = new Dictionary<string, long>
            {
                ["Total"] = stopwatch.ElapsedMilliseconds
            };

            return (sr, timings);
        }

#region 结构体

        private readonly struct NoteData : IEquatable<NoteData>
        {
            public NoteData(int column, int headTime, int tailTime)
            {
                Column   = column;
                HeadTime = headTime;
                TailTime = tailTime;
            }

            public int Column   { get; }
            public int HeadTime { get; }
            public int TailTime { get; }

            public bool IsLongNote
            {
                get => TailTime >= 0 && TailTime > HeadTime;
            }

            public bool Equals(NoteData other)
            {
                return Column == other.Column && HeadTime == other.HeadTime && TailTime == other.TailTime;
            }

            public override bool Equals(object? obj)
            {
                return obj is NoteData other && Equals(other);
            }

            public override int GetHashCode()
            {
                return HashCode.Combine(Column, HeadTime, TailTime);
            }
        }

        private readonly struct LnRepresentation
        {
            public LnRepresentation(int[] points, double[] cumulative, double[] values)
            {
                Points     = points;
                Cumulative = cumulative;
                Values     = values;
            }

            public int[]    Points     { get; }
            public double[] Cumulative { get; }
            public double[] Values     { get; }
        }

#endregion

        private static class PythonPortAlgorithm
        {
            private static readonly double[][] CrossMatrix =
            {
                new[] { -1d },
                new[] { 0.075, 0.075 },
                new[] { 0.125, 0.05, 0.125 },
                new[] { 0.125, 0.125, 0.125, 0.125 },
                new[] { 0.175, 0.25, 0.05, 0.25, 0.175 },
                new[] { 0.175, 0.25, 0.175, 0.175, 0.25, 0.175 },
                new[] { 0.225, 0.35, 0.25, 0.05, 0.25, 0.35, 0.225 },
                new[] { 0.225, 0.35, 0.25, 0.225, 0.225, 0.25, 0.35, 0.225 },
                new[] { 0.275, 0.45, 0.35, 0.25, 0.05, 0.25, 0.35, 0.45, 0.275 },
                new[] { 0.275, 0.45, 0.35, 0.25, 0.275, 0.275, 0.25, 0.35, 0.45, 0.275 },
                new[] { 0.325, 0.55, 0.45, 0.35, 0.25, 0.05, 0.25, 0.35, 0.45, 0.55, 0.325 }
            };

            public static double Calculate<T>(IBeatmap<T> beatmap) where T : HitObject
            {
                int keyCount = Math.Max(1, (int)Math.Round(beatmap.BeatmapInfo.Difficulty.CircleSize));
                if (keyCount >= CrossMatrix.Length)
                    throw new NotSupportedException($"Key mode {keyCount}k is not supported by the SR algorithm.");

                int estimatedNotes = beatmap.HitObjects.Count;
                var notes         = new List<NoteData>(estimatedNotes);
                var notesByColumn = new List<NoteData>[keyCount];
                for (int i = 0; i < keyCount; i++)
                    notesByColumn[i] = new List<NoteData>(estimatedNotes / keyCount + 1);

                foreach (T hitObject in beatmap.HitObjects)
                {
                    int column = ResolveColumn(hitObject, keyCount);
                    int head   = (int)Math.Round(hitObject.StartTime);
                    int tail   = (int)Math.Round(hitObject.EndTime);

                    if (tail <= head)
                        tail = -1;

                    var note = new NoteData(column, head, tail);
                    notes.Add(note);
                    notesByColumn[column].Add(note);
                }

                if (notes.Count == 0)
                    return 0;

                notes.Sort(NoteComparer);
                foreach (List<NoteData> columnNotes in notesByColumn)
                    columnNotes.Sort(NoteComparer);

                List<NoteData> longNotes        = notes.Where(n => n.IsLongNote).ToList();
                List<NoteData> longNotesByTails = longNotes.OrderBy(n => n.TailTime).ToList();

                // 添加调试日志
                //Console.WriteLine($"C# notes count: {notes.Count}");
                //Console.WriteLine($"C# longNotes count: {longNotes.Count}");
                //Console.WriteLine($"C# keyCount: {keyCount}");

                double od = beatmap.BeatmapInfo.Difficulty.OverallDifficulty;
                double x  = ComputeHitLeniency(od);

                //Console.WriteLine($"C# od: {od}, x: {x}");

                int maxHead   = notes.Max(n => n.HeadTime);
                int maxTail   = longNotes.Count > 0 ? longNotes.Max(n => n.TailTime) : maxHead;
                int totalTime = Math.Max(maxHead, maxTail) + 1;

                //Console.WriteLine($"C# maxHead: {maxHead}, maxTail: {maxTail}, totalTime: {totalTime}");

                (double[] allCorners, double[] baseCorners, double[] aCorners) = BuildCorners(totalTime, notes);

                //Console.WriteLine($"C# allCorners length: {allCorners.Length}, baseCorners length: {baseCorners.Length}");

                bool[][] keyUsage      = BuildKeyUsage(keyCount, totalTime, notes, baseCorners);
                int[][]  activeColumns = DeriveActiveColumns(keyUsage);

                double[][] keyUsage400 = BuildKeyUsage400(keyCount, totalTime, notes, baseCorners);
                double[]   anchorBase  = ComputeAnchor(keyCount, keyUsage400, baseCorners);

                LnRepresentation? lnRep = longNotes.Count > 0 ? BuildLnRepresentation(longNotes, totalTime) : null;

                (double[][] deltaKs, double[] jBarBase) = ComputeJBar(keyCount, totalTime, x, notesByColumn, baseCorners);
                double[] jBar = InterpValues(allCorners, baseCorners, jBarBase);

                double[] xBarBase = ComputeXBar(keyCount, totalTime, x, notesByColumn, activeColumns, baseCorners);
                double[] xBar     = InterpValues(allCorners, baseCorners, xBarBase);

                //Console.WriteLine($"C# xBarBase sample: {string.Join(", ", xBarBase.Take(10))}");

                double[] pBarBase = ComputePBar(keyCount, totalTime, x, notes, lnRep, anchorBase, baseCorners);
                double[] pBar     = InterpValues(allCorners, baseCorners, pBarBase);

                //Console.WriteLine($"C# pBarBase sample: {string.Join(", ", pBarBase.Take(10))}");

                double[] aBarBase = ComputeABar(keyCount, totalTime, deltaKs, activeColumns, aCorners, baseCorners);
                double[] aBar     = InterpValues(allCorners, aCorners, aBarBase);

                double[] rBarBase = ComputeRBar(keyCount, totalTime, x, notesByColumn, longNotesByTails, baseCorners);
                double[] rBar     = InterpValues(allCorners, baseCorners, rBarBase);

                (double[] cStep, double[] ksStep) = ComputeCAndKs(keyCount, notes, keyUsage, baseCorners);
                double[] cArr  = StepInterp(allCorners, baseCorners, cStep);
                double[] ksArr = StepInterp(allCorners, baseCorners, ksStep);

                double[] gaps             = ComputeGaps(allCorners);
                double[] effectiveWeights = new double[allCorners.Length];
                for (int i = 0; i < allCorners.Length; i++)
                    effectiveWeights[i] = cArr[i] * gaps[i];

                double[] dAll = new double[allCorners.Length];

                // Original sequential loop
                // for (int i = 0; i < allCorners.Length; i++)
                // {
                //     ...
                // }

                // Parallel version for better performance
                Parallel.For(0, allCorners.Length, i =>
                {
                    double abarExponent             = 3.0 / Math.Max(ksArr[i], 1e-6);
                    double abarPow                  = aBar[i] <= 0 ? 0 : Math.Pow(aBar[i], abarExponent);
                    double minCandidateContribution = 0.85 * jBar[i];
                    double minCandidate             = 8 + minCandidateContribution;
                    double minJ                     = Math.Min(jBar[i], minCandidate);
                    double jackComponent            = abarPow * minJ;
                    double term1                    = 0.4 * (jackComponent <= 0 ? 0 : Math.Pow(jackComponent, 1.5));

                    double scaledP     = 0.8 * pBar[i];
                    double jackPenalty = rBar[i] * 35.0;
                    double ratio       = jackPenalty / (cArr[i] + 8);
                    double pComponent  = scaledP + ratio;
                    double powerBase   = (aBar[i] <= 0 ? 0 : Math.Pow(aBar[i], 2.0 / 3.0)) * pComponent;
                    double term2       = 0.6 * (powerBase <= 0 ? 0 : Math.Pow(powerBase, 1.5));

                    double sumTerms        = term1 + term2;
                    double s               = sumTerms <= 0 ? 0 : Math.Pow(sumTerms, 2.0 / 3.0);
                    double numerator       = abarPow * xBar[i];
                    double denominator     = xBar[i] + s + 1;
                    double tValue          = denominator <= 0 ? 0 : numerator / denominator;
                    double sqrtComponent   = Math.Sqrt(Math.Max(s, 0));
                    double primaryImpact   = 2.7 * sqrtComponent * (tValue <= 0 ? 0 : Math.Pow(tValue, 1.5));
                    double secondaryImpact = s * 0.27;

                    dAll[i] = primaryImpact + secondaryImpact;
                });

                double sr = FinaliseDifficulty(dAll, effectiveWeights, notes, longNotes);

                //Console.WriteLine($"C# final SR: {sr}");

                return sr;
            }

            private static double ComputeHitLeniency(double overallDifficulty)
            {
                double leniency       = 0.3 * Math.Sqrt((64.5 - Math.Ceiling(overallDifficulty * 3.0)) / 500.0);
                double offset         = leniency - 0.09;
                double scaledOffset   = 0.6 * offset;
                double adjustedWindow = scaledOffset + 0.09;
                return Math.Min(leniency, adjustedWindow);
            }

            private static (double[] allCorners, double[] baseCorners, double[] aCorners) BuildCorners(int totalTime, List<NoteData> notes)
            {
                var baseSet = new HashSet<int>();

                foreach (NoteData note in notes)
                {
                    baseSet.Add(note.HeadTime);
                    if (note.IsLongNote)
                        baseSet.Add(note.TailTime);
                }

                foreach (int value in baseSet.ToArray())
                {
                    baseSet.Add(value + 501);
                    baseSet.Add(value - 499);
                    baseSet.Add(value + 1);
                }

                baseSet.Add(0);
                baseSet.Add(totalTime);

                double[] baseCorners = baseSet.Where(v => v >= 0 && v <= totalTime).Select(v => (double)v).Distinct().OrderBy(v => v).ToArray();

                var aSet = new HashSet<int>();

                foreach (NoteData note in notes)
                {
                    aSet.Add(note.HeadTime);
                    if (note.IsLongNote)
                        aSet.Add(note.TailTime);
                }

                foreach (int value in aSet.ToArray())
                {
                    aSet.Add(value + 1000);
                    aSet.Add(value - 1000);
                }

                aSet.Add(0);
                aSet.Add(totalTime);

                double[] aCorners = aSet.Where(v => v >= 0 && v <= totalTime).Select(v => (double)v).Distinct().OrderBy(v => v).ToArray();

                double[] allCorners = baseCorners.Concat(aCorners).Distinct().OrderBy(v => v).ToArray();
                return (allCorners, baseCorners, aCorners);
            }

            private static bool[][] BuildKeyUsage(int keyCount, int totalTime, List<NoteData> notes, double[] baseCorners)
            {
                bool[][] keyUsage = new bool[keyCount][];
                for (int i = 0; i < keyCount; i++)
                    keyUsage[i] = new bool[baseCorners.Length];

                foreach (NoteData note in notes)
                {
                    int start = Math.Max(note.HeadTime - 150, 0);
                    int end   = note.IsLongNote ? Math.Min(note.TailTime + 150, totalTime - 1) : Math.Min(note.HeadTime + 150, totalTime - 1);

                    int left  = LowerBound(baseCorners, start);
                    int right = LowerBound(baseCorners, end);
                    for (int idx = left; idx < right; idx++)
                        keyUsage[note.Column][idx] = true;
                }

                return keyUsage;
            }

            private static int[][] DeriveActiveColumns(bool[][] keyUsage)
            {
                int     length = keyUsage[0].Length;
                int[][] active = new int[length][];

                for (int i = 0; i < length; i++)
                {
                    var list = new List<int>();

                    for (int col = 0; col < keyUsage.Length; col++)
                    {
                        if (keyUsage[col][i])
                            list.Add(col);
                    }

                    active[i] = list.ToArray();
                }

                return active;
            }

            private static double[][] BuildKeyUsage400(int keyCount, int totalTime, List<NoteData> notes, double[] baseCorners)
            {
                double[][] usage = new double[keyCount][];
                for (int k = 0; k < keyCount; k++)
                    usage[k] = new double[baseCorners.Length];

                const double baseContribution = 3.75;
                const double falloff          = 3.75 / (400.0 * 400.0);

                foreach (NoteData note in notes)
                {
                    int startTime = Math.Max(note.HeadTime, 0);
                    int endTime   = note.IsLongNote ? Math.Min(note.TailTime, totalTime - 1) : note.HeadTime;

                    int left400  = LowerBound(baseCorners, startTime - 400);
                    int left     = LowerBound(baseCorners, startTime);
                    int right    = LowerBound(baseCorners, endTime);
                    int right400 = LowerBound(baseCorners, endTime + 400);

                    int    duration        = endTime - startTime;
                    double clampedDuration = Math.Min(duration, 1500);
                    double extension       = clampedDuration / 150.0;
                    double contribution    = baseContribution + extension;

                    for (int idx = left; idx < right; idx++) usage[note.Column][idx] += contribution;

                    for (int idx = left400; idx < left; idx++)
                    {
                        double offset              = baseCorners[idx] - startTime;
                        double falloffContribution = falloff * Math.Pow(offset, 2);
                        double value               = baseContribution - falloffContribution;
                        double clamped             = Math.Max(value, 0);
                        usage[note.Column][idx] += clamped;
                    }

                    for (int idx = right; idx < right400; idx++)
                    {
                        double offset              = baseCorners[idx] - endTime;
                        double falloffContribution = falloff * Math.Pow(offset, 2);
                        double value               = baseContribution - falloffContribution;
                        double clamped             = Math.Max(value, 0);
                        usage[note.Column][idx] += clamped;
                    }
                }

                return usage;
            }

            private static double[] ComputeAnchor(int keyCount, double[][] keyUsage400, double[] baseCorners)
            {
                double[] anchor = new double[baseCorners.Length];

                for (int i = 0; i < baseCorners.Length; i++)
                {
                    double[] counts = new double[keyCount];
                    for (int k = 0; k < keyCount; k++)
                        counts[k] = keyUsage400[k][i];

                    Array.Sort(counts);
                    Array.Reverse(counts);

                    double[] nonZero = counts.Where(c => c > 0).ToArray();

                    if (nonZero.Length <= 1)
                    {
                        anchor[i] = 0;
                        continue;
                    }

                    double walk    = 0;
                    double maxWalk = 0;

                    for (int idx = 0; idx < nonZero.Length - 1; idx++)
                    {
                        double current       = nonZero[idx];
                        double next          = nonZero[idx + 1];
                        double ratio         = next / current;
                        double offset        = 0.5 - ratio;
                        double offsetPenalty = 4 * Math.Pow(offset, 2);
                        double damping       = 1 - offsetPenalty;
                        walk    += current * damping;
                        maxWalk += current;
                    }

                    double value = maxWalk <= 0 ? 0 : walk / maxWalk;
                    anchor[i] = 1 + Math.Min(value - 0.18, 5 * Math.Pow(value - 0.22, 3));
                }

                return anchor;
            }

            private static LnRepresentation BuildLnRepresentation(List<NoteData> longNotes, int totalTime)
            {
                var diff = new Dictionary<int, double>();

                foreach (NoteData note in longNotes)
                {
                    int t0 = Math.Min(note.HeadTime + 60, note.TailTime);
                    int t1 = Math.Min(note.HeadTime + 120, note.TailTime);

                    AddToMap(diff, t0, 1.3);
                    AddToMap(diff, t1, -0.3);
                    AddToMap(diff, note.TailTime, -1);
                }

                var pointsSet = new SortedSet<int> { 0, totalTime };
                foreach (int key in diff.Keys)
                    pointsSet.Add(key);

                int[]    points     = pointsSet.ToArray();
                double[] cumulative = new double[points.Length];
                double[] values     = new double[points.Length - 1];

                double current = 0;

                for (int i = 0; i < points.Length - 1; i++)
                {
                    if (diff.TryGetValue(points[i], out double delta))
                        current += delta;

                    double fallbackOffset = 0.5 * current;
                    double fallback       = 2.5 + fallbackOffset;
                    double transformed    = Math.Min(current, fallback);
                    values[i] = transformed;

                    int    length  = points[i + 1] - points[i];
                    double segment = length * transformed;
                    cumulative[i + 1] = cumulative[i] + segment;
                }

                return new LnRepresentation(points, cumulative, values);
            }

            private static (double[][] deltaKs, double[] jBar) ComputeJBar(int keyCount, int totalTime, double x, List<NoteData>[] notesByColumn, double[] baseCorners)
            {
                const double defaultDelta = 1e9;

                double[][] deltaKs = new double[keyCount][];
                double[][] jKs     = new double[keyCount][];

                Parallel.For(0, keyCount, k =>
                {
                    deltaKs[k] = Enumerable.Repeat(defaultDelta, baseCorners.Length).ToArray();
                    jKs[k]     = new double[baseCorners.Length];

                    List<NoteData> columnNotes = notesByColumn[k];

                    for (int i = 0; i < columnNotes.Count - 1; i++)
                    {
                        NoteData current = columnNotes[i];
                        NoteData next    = columnNotes[i + 1];

                        int left  = LowerBound(baseCorners, current.HeadTime);
                        int right = LowerBound(baseCorners, next.HeadTime);

                        if (right <= left)
                            continue;

                        double headGap     = Math.Max(next.HeadTime - current.HeadTime, 1e-6);
                        double delta       = 0.001 * headGap;
                        double deltaShift  = Math.Abs(delta - 0.08);
                        double penalty     = 0.15 + deltaShift;
                        double attenuation = Math.Pow(penalty, -4);
                        double nerfFactor  = 7e-5 * attenuation;
                        double jackNerfer  = 1 - nerfFactor;

                        double xRoot        = Math.Pow(x, 0.25);
                        double rootScale    = 0.11 * xRoot;
                        double jackBase     = delta + rootScale;
                        double inverseJack  = Math.Pow(jackBase, -1);
                        double inverseDelta = 1.0 / delta;
                        double value        = inverseDelta * inverseJack * jackNerfer;

                        for (int idx = left; idx < right; idx++)
                        {
                            deltaKs[k][idx] = Math.Min(deltaKs[k][idx], delta);
                            jKs[k][idx]     = value;
                        }
                    }

                    jKs[k] = SmoothOnCorners(baseCorners, jKs[k], 500, 0.001, SmoothMode.Sum);
                });
                // for (int k = 0; k < keyCount; k++)

                double[] jBar = new double[baseCorners.Length];

                for (int idx = 0; idx < baseCorners.Length; idx++)
                {
                    double numerator   = 0;
                    double denominator = 0;

                    for (int k = 0; k < keyCount; k++)
                    {
                        double v      = Math.Max(jKs[k][idx], 0);
                        double weight = 1.0 / Math.Max(deltaKs[k][idx], 1e-9);
                        numerator   += Math.Pow(v, 5) * weight;
                        denominator += weight;
                    }

                    double combined = denominator <= 0 ? 0 : numerator / denominator;
                    jBar[idx] = Math.Pow(Math.Max(combined, 0), 0.2);
                }

                return (deltaKs, jBar);
            }

            private static double[] ComputeXBar(int keyCount, int totalTime, double x, List<NoteData>[] notesByColumn, int[][] activeColumns, double[] baseCorners)
            {
                double[]   cross     = CrossMatrix[keyCount];
                double[][] xKs       = new double[keyCount + 1][];
                double[][] fastCross = new double[keyCount + 1][];

                for (int i = 0; i < xKs.Length; i++)
                {
                    xKs[i]       = new double[baseCorners.Length];
                    fastCross[i] = new double[baseCorners.Length];
                }

                // Parallel.For(0, keyCount + 1, k =>
                Parallel.For(0, keyCount + 1, k =>
                {
                    var pair = new List<NoteData>();

                    if (k == 0)
                        pair.AddRange(notesByColumn[0]);
                    else if (k == keyCount)
                        pair.AddRange(notesByColumn[keyCount - 1]);
                    else
                    {
                        pair.AddRange(notesByColumn[k - 1]);
                        pair.AddRange(notesByColumn[k]);
                    }

                    pair.Sort(NoteComparer);
                    if (pair.Count < 2) return;

                    for (int i = 1; i < pair.Count; i++)
                    {
                        NoteData prev    = pair[i - 1];
                        NoteData current = pair[i];
                        int      left    = LowerBound(baseCorners, prev.HeadTime);
                        int      right   = LowerBound(baseCorners, current.HeadTime);
                        if (right <= left) continue;

                        double delta = 0.001 * Math.Max(current.HeadTime - prev.HeadTime, 1e-6);
                        double val   = 0.16 * Math.Pow(Math.Max(x, delta), -2);

                        int idxStart = Math.Min(left, baseCorners.Length - 1);
                        int idxEnd   = Math.Min(Math.Max(right, 0), baseCorners.Length - 1);

                        bool condition1 = !Contains(activeColumns[idxStart], k - 1) && !Contains(activeColumns[idxEnd], k - 1);
                        bool condition2 = !Contains(activeColumns[idxStart], k) && !Contains(activeColumns[idxEnd], k);
                        if (condition1 || condition2)
                            val *= 1 - cross[Math.Min(k, cross.Length - 1)];

                        for (int idx = left; idx < right; idx++)
                        {
                            xKs[k][idx]       = val;
                            fastCross[k][idx] = Math.Max(0, 0.4 * Math.Pow(Math.Max(Math.Max(delta, 0.06), 0.75 * x), -2) - 80);
                        }
                    }
                });
                // for (int k = 0; k <= keyCount; k++)

                double[] xBase = new double[baseCorners.Length];

                for (int idx = 0; idx < baseCorners.Length; idx++)
                {
                    double sum = 0;
                    for (int k = 0; k <= keyCount; k++)
                        sum += cross[Math.Min(k, cross.Length - 1)] * xKs[k][idx];

                    for (int k = 0; k < keyCount; k++)
                    {
                        double leftVal  = fastCross[k][idx] * cross[Math.Min(k, cross.Length - 1)];
                        double rightVal = fastCross[k + 1][idx] * cross[Math.Min(k + 1, cross.Length - 1)];
                        sum += Math.Sqrt(Math.Max(leftVal * rightVal, 0));
                    }

                    xBase[idx] = sum;
                }

                return SmoothOnCorners(baseCorners, xBase, 500, 0.001, SmoothMode.Sum);
            }

            private static double[] ComputePBar(int keyCount, int totalTime, double x, List<NoteData> notes, LnRepresentation? lnRep, double[] anchor, double[] baseCorners)
            {
                double[] pStep = new double[baseCorners.Length];

                for (int i = 0; i < notes.Count - 1; i++)
                {
                    NoteData leftNote  = notes[i];
                    NoteData rightNote = notes[i + 1];

                    int deltaTime = rightNote.HeadTime - leftNote.HeadTime;

                    if (deltaTime <= 0)
                    {
                        double invX           = 1.0 / Math.Max(x, 1e-6);
                        double spikeInnerBase = 4 * invX;
                        double spikeInner     = spikeInnerBase - 24;
                        double spikeBase      = 0.02 * spikeInner;
                        if (spikeBase <= 0)
                            continue;

                        double spikeMagnitude = Math.Pow(spikeBase, 0.25);
                        double spike          = 1000 * spikeMagnitude;
                        int    leftIdx        = LowerBound(baseCorners, leftNote.HeadTime);
                        int    rightIdx       = UpperBound(baseCorners, leftNote.HeadTime);
                        for (int idx = leftIdx; idx < rightIdx; idx++)
                            pStep[idx] += spike;

                        continue;
                    }

                    int left  = LowerBound(baseCorners, leftNote.HeadTime);
                    int right = LowerBound(baseCorners, rightNote.HeadTime);
                    if (right <= left) continue;

                    double delta = 0.001 * deltaTime;
                    double v     = 1;
                    if (lnRep.HasValue)
                        v += 6 * 0.001 * LnIntegral(lnRep.Value, leftNote.HeadTime, rightNote.HeadTime);

                    double booster   = StreamBooster(delta);
                    double effective = Math.Max(booster, v);

                    double inc;

                    if (delta < 2 * x / 3)
                    {
                        double invX        = 1.0 / Math.Max(x, 1e-6);
                        double halfX       = x / 2.0;
                        double deltaCentre = delta - halfX;
                        double deltaTerm   = 24 * invX * Math.Pow(deltaCentre, 2);
                        double inner       = 0.08 * invX * (1 - deltaTerm);
                        double innerClamp  = Math.Max(inner, 0);
                        double magnitude   = Math.Pow(innerClamp, 0.25);
                        inc = magnitude / Math.Max(delta, 1e-6) * effective;
                    }
                    else
                    {
                        double invX       = 1.0 / Math.Max(x, 1e-6);
                        double centreTerm = Math.Pow(x / 6.0, 2);
                        double deltaTerm  = 24 * invX * centreTerm;
                        double inner      = 0.08 * invX * (1 - deltaTerm);
                        double innerClamp = Math.Max(inner, 0);
                        double magnitude  = Math.Pow(innerClamp, 0.25);
                        inc = magnitude / Math.Max(delta, 1e-6) * effective;
                    }

                    for (int idx = left; idx < right; idx++)
                    {
                        double doubled      = inc * 2;
                        double limit        = Math.Max(inc, doubled - 10);
                        double anchored     = inc * anchor[idx];
                        double contribution = Math.Min(anchored, limit);

                        pStep[idx] += contribution;
                    }
                }

                return SmoothOnCorners(baseCorners, pStep, 500, 0.001, SmoothMode.Sum);
            }

            private static double[] ComputeABar(int keyCount, int totalTime, double[][] deltaKs, int[][] activeColumns, double[] aCorners, double[] baseCorners)
            {
                double[] aStep = Enumerable.Repeat(1.0, aCorners.Length).ToArray();

                for (int i = 0; i < aCorners.Length; i++)
                {
                    int idx = LowerBound(baseCorners, aCorners[i]);
                    idx = Math.Min(idx, baseCorners.Length - 1);
                    int[] cols = activeColumns[idx];
                    if (cols.Length < 2) continue;

                    for (int j = 0; j < cols.Length - 1; j++)
                    {
                        int c0 = cols[j];
                        int c1 = cols[j + 1];

                        double deltaGap           = Math.Abs(deltaKs[c0][idx] - deltaKs[c1][idx]);
                        double maxDelta           = Math.Max(deltaKs[c0][idx], deltaKs[c1][idx]);
                        double offset             = Math.Max(maxDelta - 0.11, 0);
                        double offsetContribution = 0.4 * offset;
                        double diff               = deltaGap + offsetContribution;

                        if (diff < 0.02)
                        {
                            double factorBase         = Math.Max(deltaKs[c0][idx], deltaKs[c1][idx]);
                            double factorContribution = 0.5 * factorBase;
                            double factor             = 0.75 + factorContribution;
                            aStep[i] *= Math.Min(factor, 1);
                        }
                        else if (diff < 0.07)
                        {
                            double factorBase         = Math.Max(deltaKs[c0][idx], deltaKs[c1][idx]);
                            double growth             = 5 * diff;
                            double factorContribution = 0.5 * factorBase;
                            double factor             = 0.65 + growth + factorContribution;
                            aStep[i] *= Math.Min(factor, 1);
                        }
                    }
                }

                return SmoothOnCorners(aCorners, aStep, 250, 0, SmoothMode.Average);
            }

            private static double[] ComputeRBar(int keyCount, int totalTime, double x, List<NoteData>[] notesByColumn, List<NoteData> tailNotes, double[] baseCorners)
            {
                if (tailNotes.Count < 2) return new double[baseCorners.Length];

                double[] iList = new double[tailNotes.Count];

                for (int idx = 0; idx < tailNotes.Count; idx++)
                {
                    NoteData  note     = tailNotes[idx];
                    NoteData? next     = FindNextColumnNote(note, notesByColumn);
                    double    nextHead = next?.HeadTime ?? 1_000_000_000;

                    double ih = 0.001 * Math.Abs(note.TailTime - note.HeadTime - 80) / Math.Max(x, 1e-6);
                    double it = 0.001 * Math.Abs(nextHead - note.TailTime - 80) / Math.Max(x, 1e-6);

                    iList[idx] = 2 / (2 + Math.Exp(-5 * (ih - 0.75)) + Math.Exp(-5 * (it - 0.75)));
                }

                double[] rStep = new double[baseCorners.Length];

                for (int idx = 0; idx < tailNotes.Count - 1; idx++)
                {
                    NoteData current = tailNotes[idx];
                    NoteData next    = tailNotes[idx + 1];

                    int left  = LowerBound(baseCorners, current.TailTime);
                    int right = LowerBound(baseCorners, next.TailTime);
                    if (right <= left) continue;

                    double delta             = 0.001 * Math.Max(next.TailTime - current.TailTime, 1e-6);
                    double invSqrtDelta      = Math.Pow(delta, -0.5);
                    double invX              = 1.0 / Math.Max(x, 1e-6);
                    double blend             = iList[idx] + iList[idx + 1];
                    double blendContribution = 0.8 * blend;
                    double modulation        = 1 + blendContribution;
                    double strength          = 0.08 * invSqrtDelta * invX * modulation;

                    for (int baseIdx = left; baseIdx < right; baseIdx++)
                        rStep[baseIdx] = Math.Max(rStep[baseIdx], strength);
                }

                return SmoothOnCorners(baseCorners, rStep, 500, 0.001, SmoothMode.Sum);
            }

            private static (double[] cStep, double[] ksStep) ComputeCAndKs(int keyCount, List<NoteData> notes, bool[][] keyUsage, double[] baseCorners)
            {
                double[] cStep  = new double[baseCorners.Length];
                double[] ksStep = new double[baseCorners.Length];

                var noteTimesList = new List<double>(notes.Count);
                foreach (NoteData note in notes)
                    noteTimesList.Add(note.HeadTime);
                noteTimesList.Sort();
                double[] noteTimes = noteTimesList.ToArray();

                for (int idx = 0; idx < baseCorners.Length; idx++)
                {
                    double left  = baseCorners[idx] - 500;
                    double right = baseCorners[idx] + 500;

                    int leftIndex  = LowerBound(noteTimes, left);
                    int rightIndex = LowerBound(noteTimes, right);
                    cStep[idx] = Math.Max(rightIndex - leftIndex, 0);

                    int activeCount = 0;

                    for (int col = 0; col < keyCount; col++)
                    {
                        if (keyUsage[col][idx])
                            activeCount++;
                    }

                    ksStep[idx] = Math.Max(activeCount, 1);
                }

                return (cStep, ksStep);
            }

            private static double[] ComputeGaps(double[] corners)
            {
                if (corners.Length == 0)
                    return Array.Empty<double>();

                double[] gaps = new double[corners.Length];

                if (corners.Length == 1)
                {
                    gaps[0] = 0;
                    return gaps;
                }

                gaps[0]  = (corners[1] - corners[0]) / 2.0;
                gaps[^1] = (corners[^1] - corners[^2]) / 2.0;

                for (int i = 1; i < corners.Length - 1; i++) gaps[i] = (corners[i + 1] - corners[i - 1]) / 2.0;

                return gaps;
            }

            private static double FinaliseDifficulty(List<double> difficulties, List<double> weights, List<NoteData> notes, List<NoteData> longNotes)
            {
                List<(double d, double w)> combined = difficulties.Zip(weights, (d, w) => (d, w)).OrderBy(pair => pair.d).ToList();
                if (combined.Count == 0)
                    return 0;

                double[] sortedD       = combined.Select(p => p.d).ToArray();
                double[] sortedWeights = combined.Select(p => Math.Max(p.w, 0)).ToArray();

                double[] cumulative = new double[sortedWeights.Length];
                cumulative[0] = sortedWeights[0];
                for (int i = 1; i < sortedWeights.Length; i++)
                    cumulative[i] = cumulative[i - 1] + sortedWeights[i];

                double   totalWeight = Math.Max(cumulative[^1], 1e-9);
                double[] norm        = cumulative.Select(v => v / totalWeight).ToArray();

                double[] targets      = { 0.945, 0.935, 0.925, 0.915, 0.845, 0.835, 0.825, 0.815 };
                double   percentile93 = 0;
                double   percentile83 = 0;

                for (int i = 0; i < 4; i++)
                {
                    int index = Math.Min(BisectLeft(norm, targets[i]), sortedD.Length - 1);
                    percentile93 += sortedD[index];
                }

                percentile93 /= 4.0;

                for (int i = 4; i < 8; i++)
                {
                    int index = Math.Min(BisectLeft(norm, targets[i]), sortedD.Length - 1);
                    percentile83 += sortedD[index];
                }

                percentile83 /= 4.0;

                //Console.WriteLine($"C# percentile93: {percentile93}, percentile83: {percentile83}");

                double weightedMeanNumerator = 0;
                for (int i = 0; i < sortedD.Length; i++)
                    weightedMeanNumerator += Math.Pow(sortedD[i], 5) * sortedWeights[i];

                double weightedMean = Math.Pow(Math.Max(weightedMeanNumerator / totalWeight, 0), 0.2);

                //Console.WriteLine($"C# weightedMean: {weightedMean}");

                double topComponent    = 0.25 * 0.88 * percentile93;
                double middleComponent = 0.2 * 0.94 * percentile83;
                double meanComponent   = 0.55 * weightedMean;
                double sr              = topComponent + middleComponent + meanComponent;
                sr = Math.Pow(sr, 1.0) / Math.Pow(8, 1.0) * 8;

                //Console.WriteLine($"C# sr before notes adjustment: {sr}");

                double totalNotes = notes.Count;

                foreach (NoteData ln in longNotes)
                {
                    double len = Math.Min(ln.TailTime - ln.HeadTime, 1000);
                    totalNotes += 0.5 * (len / 200.0);
                }

                //Console.WriteLine($"C# totalNotes: {totalNotes}");

                sr *= totalNotes / (totalNotes + 60);
                sr =  RescaleHigh(sr);
                sr *= 0.975;

                //Console.WriteLine($"C# final SR: {sr}");

                return sr;
            }

            private static double FinaliseDifficulty(double[] difficulties, double[] weights, List<NoteData> notes, List<NoteData> longNotes)
            {
                return FinaliseDifficulty(difficulties.ToList(), weights.ToList(), notes, longNotes);
            }

            private static NoteData? FindNextColumnNote(NoteData note, List<NoteData>[] notesByColumn)
            {
                List<NoteData> columnNotes = notesByColumn[note.Column];
                int            index       = columnNotes.IndexOf(note);
                if (index >= 0 && index + 1 < columnNotes.Count)
                    return columnNotes[index + 1];

                return null;
            }

            private static int ResolveColumn(HitObject hitObject, int keyCount)
            {
                switch (hitObject)
                {
                    case ManiaHoldNote hold:
                        if (hold.KeyCount <= 0)
                            hold.KeyCount = keyCount;
                        return Clamp(hold.Column, 0, keyCount - 1);

                    case ManiaHitObject mania:
                        if (mania.KeyCount <= 0)
                            mania.KeyCount = keyCount;
                        return Clamp(mania.Column, 0, keyCount - 1);

                    default:
                        return Clamp(ManiaExtensions.GetColumnFromX(keyCount, hitObject.Position.X), 0, keyCount - 1);
                }
            }

            private static double[] InterpValues(double[] newX, double[] oldX, double[] oldVals)
            {
                double[] result = new double[newX.Length];

                for (int i = 0; i < newX.Length; i++)
                {
                    double x = newX[i];

                    if (x <= oldX[0])
                    {
                        result[i] = oldVals[0];
                        continue;
                    }

                    if (x >= oldX[^1])
                    {
                        result[i] = oldVals[^1];
                        continue;
                    }

                    int idx = LowerBound(oldX, x);

                    if (idx < oldX.Length && NearlyEquals(oldX[idx], x))
                    {
                        result[i] = oldVals[idx];
                        continue;
                    }

                    int    prev      = Math.Max(idx - 1, 0);
                    double x0        = oldX[prev];
                    double x1        = oldX[idx];
                    double y0        = oldVals[prev];
                    double y1        = oldVals[idx];
                    double deltaY    = y1 - y0;
                    double deltaX    = x - x0;
                    double numerator = deltaY * deltaX;
                    double fraction  = numerator / (x1 - x0);
                    result[i] = y0 + fraction;
                }

                return result;
            }

            private static double[] StepInterp(double[] newX, double[] oldX, double[] oldVals)
            {
                double[] result = new double[newX.Length];

                for (int i = 0; i < newX.Length; i++)
                {
                    int idx = UpperBound(oldX, newX[i]) - 1;
                    if (idx < 0)
                        idx = 0;
                    result[i] = oldVals[Math.Min(idx, oldVals.Length - 1)];
                }

                return result;
            }

            private enum SmoothMode
            {
                Sum,
                Average
            }

            private static double[] SmoothOnCorners(double[] positions, double[] values, double window, double scale, SmoothMode mode)
            {
                if (positions.Length == 0)
                    return Array.Empty<double>();

                double[] cumulative = BuildCumulative(positions, values);
                double[] output     = new double[positions.Length];

                for (int i = 0; i < positions.Length; i++)
                {
                    double s = positions[i];
                    double a = Math.Max(s - window, positions[0]);
                    double b = Math.Min(s + window, positions[^1]);

                    if (b <= a)
                    {
                        output[i] = 0;
                        continue;
                    }

                    double integral = QueryIntegral(positions, cumulative, values, b) - QueryIntegral(positions, cumulative, values, a);

                    if (mode == SmoothMode.Average)
                        output[i] = integral / Math.Max(b - a, 1e-9);
                    else
                        output[i] = integral * scale;
                }

                return output;
            }

            private static double[] BuildCumulative(double[] positions, double[] values)
            {
                double[] cumulative = new double[positions.Length];

                for (int i = 1; i < positions.Length; i++)
                {
                    double width     = positions[i] - positions[i - 1];
                    double increment = values[i - 1] * width;
                    cumulative[i] = cumulative[i - 1] + increment;
                }

                return cumulative;
            }

            private static double QueryIntegral(double[] positions, double[] cumulative, double[] values, double point)
            {
                if (point <= positions[0])
                    return 0;
                if (point >= positions[^1])
                    return cumulative[^1];

                int idx = LowerBound(positions, point);
                if (idx < positions.Length && NearlyEquals(positions[idx], point))
                    return cumulative[idx];

                int    prev         = Math.Max(idx - 1, 0);
                double delta        = point - positions[prev];
                double contribution = values[prev] * delta;

                return cumulative[prev] + contribution;
            }

            private static double LnIntegral(LnRepresentation representation, int a, int b)
            {
                int[]    points     = representation.Points;
                double[] cumulative = representation.Cumulative;
                double[] values     = representation.Values;

                int startIndex = UpperBound(points, a) - 1;
                int endIndex   = UpperBound(points, b) - 1;

                if (startIndex < 0) startIndex      = 0;
                if (endIndex < startIndex) endIndex = startIndex;

                double total = 0;

                if (startIndex == endIndex)
                    total = (b - a) * values[startIndex];
                else
                {
                    total += (points[startIndex + 1] - a) * values[startIndex];
                    total += cumulative[endIndex] - cumulative[startIndex + 1];
                    total += (b - points[endIndex]) * values[endIndex];
                }

                return total;
            }

            private static double StreamBooster(double delta)
            {
                double inv = 7.5 / Math.Max(delta, 1e-6);
                if (inv <= 160 || inv >= 360)
                    return 1;

                double shifted    = inv - 160;
                double distance   = inv - 360;
                double adjustment = 1.7e-7 * shifted * Math.Pow(distance, 2);

                return 1 + adjustment;
            }

            private static bool Contains(int[] array, int target)
            {
                if (target < 0)
                    return false;

                for (int i = 0; i < array.Length; i++)
                {
                    if (array[i] == target)
                        return true;
                }

                return false;
            }

            private static int LowerBound(double[] array, double value)
            {
                int left  = 0;
                int right = array.Length;

                while (left < right)
                {
                    int span = right - left;
                    int mid  = left + (span >> 1);
                    if (array[mid] < value)
                        left = mid + 1;
                    else
                        right = mid;
                }

                return left;
            }

            private static int LowerBound(double[] array, int value)
            {
                return LowerBound(array, (double)value);
            }

            private static int LowerBound(int[] array, double value)
            {
                int left  = 0;
                int right = array.Length;

                while (left < right)
                {
                    int span = right - left;
                    int mid  = left + (span >> 1);
                    if (array[mid] < value)
                        left = mid + 1;
                    else
                        right = mid;
                }

                return left;
            }

            private static int UpperBound(int[] array, int value)
            {
                int left  = 0;
                int right = array.Length;

                while (left < right)
                {
                    int span = right - left;
                    int mid  = left + (span >> 1);
                    if (array[mid] <= value)
                        left = mid + 1;
                    else
                        right = mid;
                }

                return left;
            }

            private static int UpperBound(double[] array, double value)
            {
                int left  = 0;
                int right = array.Length;

                while (left < right)
                {
                    int span = right - left;
                    int mid  = left + (span >> 1);
                    if (array[mid] <= value)
                        left = mid + 1;
                    else
                        right = mid;
                }

                return left;
            }

            private static int BisectLeft(double[] array, double value)
            {
                int left  = 0;
                int right = array.Length;

                while (left < right)
                {
                    int span = right - left;
                    int mid  = left + (span >> 1);
                    if (array[mid] < value)
                        left = mid + 1;
                    else
                        right = mid;
                }

                return left;
            }

            private static double SafePow(double value, double exponent)
            {
                if (value <= 0)
                    return 0;

                double result = Math.Pow(value, exponent);

                return result;
            }

            private static double RescaleHigh(double sr)
            {
                double excess     = sr - 9;
                double normalized = excess / 1.2;
                double softened   = 9 + normalized;

                return sr <= 9 ? sr : softened;
            }

            private static int Clamp(int value, int min, int max)
            {
                return Math.Min(Math.Max(value, min), max);
            }

            private static bool NearlyEquals(double a, double b, double epsilon = 1e-9)
            {
                return Math.Abs(a - b) <= epsilon;
            }

            private static int CompareNotes(NoteData a, NoteData b)
            {
                int headCompare = a.HeadTime.CompareTo(b.HeadTime);
                return headCompare != 0 ? headCompare : a.Column.CompareTo(b.Column);
            }

            private static readonly Comparison<NoteData> NoteComparer = CompareNotes;

            private static void AddToMap(Dictionary<int, double> map, int key, double value)
            {
                if (!map.TryAdd(key, value))
                    map[key] += value;
            }
        }
    }
}
