using System;
using System.Collections.Generic;
using System.Linq;
using BenchmarkDotNet.Attributes;
using LAsOsuBeatmapParser.Analysis;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Tests;

namespace LAsOsuBeatmapParser.Tests
{
    [MemoryDiagnoser]
    public class SRCalculatorBenchmarks
    {
        private Beatmap<ManiaHitObject> _testBeatmap;
        private SRCalculator _calculator;

        [GlobalSetup]
        public void Setup()
        {
            // 创建测试数据：一个中等复杂度的谱面
            var random = new Random(42); // 固定种子确保一致性
            int totalNotes = 2000;
            int maxTime = 180000; // 3分钟
            int keyCount = 10;

            var hitObjects = new List<ManiaHitObject>();

            for (int i = 0; i < totalNotes; i++)
            {
                int time = (int)(i * (maxTime / (double)totalNotes));
                int column = random.Next(0, keyCount);
                bool isLn = random.Next(0, 10) < 2; // 20% LN
                int tail = isLn ? time + random.Next(500, 2000) : time;

                var hitObject = new ManiaHitObject(time, column, keyCount);
                if (isLn) hitObject.EndTime = tail;
                hitObjects.Add(hitObject);
            }

            _testBeatmap = new Beatmap<ManiaHitObject>();
            _testBeatmap.Difficulty.CircleSize = keyCount;
            _testBeatmap.Difficulty.OverallDifficulty = 8.0f;
            _testBeatmap.HitObjects = hitObjects;

            _calculator = SRCalculator.Instance;
        }

        [Benchmark]
        public double CalculateSR()
        {
            return _calculator.CalculateSR(_testBeatmap, out _);
        }

        [Benchmark]
        public double CalculateSRWithTimeTracking()
        {
            return _calculator.CalculateSR(_testBeatmap, out Dictionary<string, long> times);
        }
    }
}
