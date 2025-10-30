# 自定义游戏模式示例 / Custom Game Mode Example

这个示例展示了如何使用 LAsOsuBeatmapParser 创建和使用自定义游戏模式。

This example shows how to create and use custom game modes with LAsOsuBeatmapParser.

## 1. 定义自定义游戏模式 / Define Custom Game Mode

```csharp
using LAsOsuBeatmapParser.Beatmaps;

public class PuzzleMode : IGameMode
{
    public int Id => 100; // 自定义ID必须大于3 / Custom ID must be greater than 3
    public string Name => "Puzzle Mode";

    public override string ToString() => Name;
}
```

## 2. 定义自定义打击对象 / Define Custom Hit Object

```csharp
public class PuzzlePiece : HitObject
{
    public string Shape { get; set; }
    public int Connections { get; set; }

    public PuzzlePiece(double startTime, string shape, int connections)
    {
        StartTime = startTime;
        Shape = shape;
        Connections = connections;
    }
}
```

## 3. 创建自定义谱面类 / Create Custom Beatmap Class

```csharp
public class PuzzleBeatmap : Beatmap<PuzzlePiece>
{
    public PuzzleBeatmap()
    {
        Mode = new PuzzleMode();
        Metadata = new BeatmapMetadata
        {
            Title = "Puzzle Challenge",
            Artist = "Custom Creator",
            Creator = "Puzzle Maker"
        };
    }
}
```

## 4. 创建扩展方法 / Create Extension Methods

```csharp
public static class PuzzleBeatmapExtensions
{
    /// <summary>
    /// 从 Beatmap 获取 PuzzleBeatmap，并验证模式。
    /// </summary>
    public static PuzzleBeatmap GetPuzzleBeatmap<T>(this Beatmap<T> beatmap) where T : HitObject
    {
        // 如果已经是PuzzleBeatmap，直接返回
        if (typeof(T) == typeof(PuzzlePiece) && beatmap is PuzzleBeatmap puzzleBeatmap)
        {
            return puzzleBeatmap;
        }

        // 验证模式ID
        if (beatmap.Mode.Id != 100)
        {
            throw new InvalidModeException("Beatmap mode must be Puzzle Mode.");
        }

        // 创建并返回PuzzleBeatmap
        return new PuzzleBeatmap
        {
            BeatmapInfo = beatmap.BeatmapInfo,
            ControlPointInfo = beatmap.ControlPointInfo,
            Breaks = beatmap.Breaks,
            MetadataLegacy = beatmap.MetadataLegacy,
            DifficultyLegacy = beatmap.DifficultyLegacy,
            TimingPoints = beatmap.TimingPoints,
            HitObjects = beatmap.HitObjects.Select(h => new PuzzlePiece(h.StartTime, "converted", 3)).ToList(),
            Events = beatmap.Events,
            Mode = beatmap.Mode,
            Version = beatmap.Version
        };
    }
}
```

## 5. 自定义解析逻辑 / Custom Parsing Logic

```csharp
using LAsOsuBeatmapParser.Beatmaps.Formats;

public class PuzzleBeatmapDecoder : LegacyBeatmapDecoder
{
    protected override HitObject ParseHitObject(string[] parts)
    {
        // 检查是否是谜题模式 / Check if it's puzzle mode
        if (CurrentBeatmap.Mode.Id == 100)
        {
            return ParsePuzzlePiece(parts);
        }

        return base.ParseHitObject(parts);
    }

    private PuzzlePiece ParsePuzzlePiece(string[] parts)
    {
        // 自定义解析逻辑 / Custom parsing logic
        // 假设格式: time,shape,connections
        double time = double.Parse(parts[0]);
        string shape = parts[1];
        int connections = int.Parse(parts[2]);

        return new PuzzlePiece(time, shape, connections);
    }
}
```

## 6. 使用示例 / Usage Example

```csharp
public class Program
{
    public static void Main()
    {
        // 创建自定义谱面 / Create custom beatmap
        var puzzleBeatmap = new PuzzleBeatmap();
        puzzleBeatmap.HitObjects.Add(new PuzzlePiece(1000, "triangle", 3));
        puzzleBeatmap.HitObjects.Add(new PuzzlePiece(2000, "square", 4));

        // 使用解析器 / Use parser
        var decoder = new PuzzleBeatmapDecoder();
        // 可以解析自定义格式的文件 / Can parse custom format files

        Console.WriteLine($"游戏模式: {puzzleBeatmap.Mode.Name}");
        Console.WriteLine($"打击对象数量: {puzzleBeatmap.HitObjects.Count}");

        // 使用扩展方法 / Use extension method
        PuzzleBeatmap converted = puzzleBeatmap.GetPuzzleBeatmap();
        Console.WriteLine("成功转换为PuzzleBeatmap!");

        // 享受标准API / Enjoy standard API
        var statistics = puzzleBeatmap.GetStatistics();
        foreach (var stat in statistics)
        {
            Console.WriteLine($"{stat.Name}: {stat.Content}");
        }
    }
}
```
