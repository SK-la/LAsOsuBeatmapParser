# 自定义游戏模式示例 / Custom Game Mode Example# 自定义游戏模式示例 / Custom Game Mode Example



这个示例展示了如何使用 LAsOsuBeatmapParser 创建和使用自定义游戏模式。这个示例展示了如何使用 LAsOsuBeatmapParser 创建和使用自定义游戏模式。



This example shows how to create and use custom game modes with LAsOsuBeatmapParser.This example shows how to create and use custom game modes with LAsOsuBeatmapParser.



```csharp```csharp

using LAsOsuBeatmapParser.Beatmaps;using LAsOsuBeatmapParser.Beatmaps;

using LAsOsuBeatmapParser.Beatmaps.Formats;using LAsOsuBeatmapParser.Beatmaps.Formats;



// 1. 定义自定义游戏模式 / Define custom game mode// 1. 定义自定义游戏模式 / Define custom game mode

public class PuzzleMode : IGameModepublic class PuzzleMode : IGameMode

{{

    public int Id => 100; // 自定义ID必须大于3    public int Id => 100; // 自定义ID必须大于3

    public string Name => "Puzzle Mode";    public string Name => "Puzzle Mode";



    public override string ToString() => Name;    public override string ToString() => Name;

}}



// 2. 定义自定义打击对象 / Define custom hit object// 2. 定义自定义打击对象 / Define custom hit object

public class PuzzlePiece : HitObjectpublic class PuzzlePiece : HitObject

{{

    public string Shape { get; set; }    public string Shape { get; set; }

    public int Connections { get; set; }    public int Connections { get; set; }



    public PuzzlePiece(double startTime, string shape, int connections)    public PuzzlePiece(double startTime, string shape, int connections)

    {    {

        StartTime = startTime;        StartTime = startTime;

        Shape = shape;        Shape = shape;

        Connections = connections;        Connections = connections;

    }    }

}}



// 3. 创建自定义谱面类 / Create custom beatmap class// 3. 创建自定义谱面类 / Create custom beatmap class

public class PuzzleBeatmap : Beatmap<PuzzlePiece>public class PuzzleBeatmap : Beatmap<PuzzlePiece>

{{

    public PuzzleBeatmap()    public PuzzleBeatmap()

    {    {

        Mode = new PuzzleMode();        Mode = new PuzzleMode();

        Metadata = new BeatmapMetadata        Metadata = new BeatmapMetadata

        {        {

            Title = "Puzzle Challenge",            Title = "Puzzle Challenge",

            Artist = "Custom Creator",            Artist = "Custom Creator",

            Creator = "Puzzle Maker"            Creator = "Puzzle Maker"

        };        };

    }    }

}}



// 4. 创建扩展方法 / Create extension methods// 5. 创建扩展方法 / Create extension methods

public static class PuzzleBeatmapExtensionspublic static class PuzzleBeatmapExtensions

{{

    /// <summary>    /// <summary>

    /// 从 Beatmap 获取 PuzzleBeatmap，并验证模式。    /// 从 Beatmap 获取 PuzzleBeatmap，并验证模式。

    /// </summary>    /// </summary>

    public static PuzzleBeatmap GetPuzzleBeatmap<T>(this Beatmap<T> beatmap) where T : HitObject    public static PuzzleBeatmap GetPuzzleBeatmap<T>(this Beatmap<T> beatmap) where T : HitObject

    {    {

        // 如果已经是PuzzleBeatmap，直接返回        // 如果已经是PuzzleBeatmap，直接返回

        if (typeof(T) == typeof(PuzzlePiece) && beatmap is PuzzleBeatmap puzzleBeatmap)        if (typeof(T) == typeof(PuzzleHitObject) && beatmap is PuzzleBeatmap puzzleBeatmap)

        {        {

            return puzzleBeatmap;            return puzzleBeatmap;

        }        }



        // 验证模式ID        // 验证模式ID

        if (beatmap.Mode.Id != 100)        if (beatmap.Mode.Id != 100)

        {        {

            throw new InvalidModeException("Beatmap mode must be Puzzle Mode.");            throw new InvalidModeException("Beatmap mode must be Puzzle Mode.");

        }        }



        // 创建并返回PuzzleBeatmap        // 创建并返回PuzzleBeatmap

        return new PuzzleBeatmap        return new PuzzleBeatmap

        {        {

            BeatmapInfo = beatmap.BeatmapInfo,            BeatmapInfo = beatmap.BeatmapInfo,

            ControlPointInfo = beatmap.ControlPointInfo,            ControlPointInfo = beatmap.ControlPointInfo,

            Breaks = beatmap.Breaks,            Breaks = beatmap.Breaks,

            MetadataLegacy = beatmap.MetadataLegacy,            MetadataLegacy = beatmap.MetadataLegacy,

            DifficultyLegacy = beatmap.DifficultyLegacy,            DifficultyLegacy = beatmap.DifficultyLegacy,

            TimingPoints = beatmap.TimingPoints,            TimingPoints = beatmap.TimingPoints,

            HitObjects = beatmap.HitObjects.Select(h => new PuzzlePiece(h.StartTime, "converted", 3)).ToList(),            HitObjects = beatmap.HitObjects.Select(h => new PuzzleHitObject(h.StartTime, "converted", 3)).ToList(),

            Events = beatmap.Events,            Events = beatmap.Events,

            Mode = beatmap.Mode,            Mode = beatmap.Mode,

            Version = beatmap.Version            Version = beatmap.Version

        };        };

    }    }

}}

{

// 5. 使用示例 / Usage example    protected override HitObject ParseHitObject(string[] parts)

public class Program    {

{        // 检查是否是谜题模式 / Check if it's puzzle mode

    public static void Main()        if (CurrentBeatmap.Mode.Id == 999)

    {        {

        // 创建自定义谱面 / Create custom beatmap            return ParsePuzzlePiece(parts);

        var puzzleBeatmap = new PuzzleBeatmap();        }

        puzzleBeatmap.HitObjects.Add(new PuzzlePiece(1000, "triangle", 3));

        puzzleBeatmap.HitObjects.Add(new PuzzlePiece(2000, "square", 4));        return base.ParseHitObject(parts);

    }

        // 使用解析器 / Use parser

        var decoder = new BeatmapDecoder();    private PuzzlePiece ParsePuzzlePiece(string[] parts)

        // 可以解析自定义格式的文件 / Can parse custom format files    {

        // 自定义解析逻辑 / Custom parsing logic

        Console.WriteLine($"游戏模式: {puzzleBeatmap.Mode.Name}");        // 假设格式: time,shape,connections

        Console.WriteLine($"打击对象数量: {puzzleBeatmap.HitObjects.Count}");        double time = double.Parse(parts[0]);

        string shape = parts[1];

        // 使用扩展方法 / Use extension method        int connections = int.Parse(parts[2]);

        PuzzleBeatmap converted = puzzleBeatmap.GetPuzzleBeatmap();

        Console.WriteLine("成功转换为PuzzleBeatmap!");        return new PuzzlePiece(time, shape, connections);

    }

        // 享受标准API / Enjoy standard API}

        var statistics = puzzleBeatmap.GetStatistics();

        foreach (var stat in statistics)// 5. 使用示例 / Usage example

        {public class Program

            Console.WriteLine($"{stat.Name}: {stat.Content}");{

        }    public static void Main()

    }    {

}        // 创建自定义谱面 / Create custom beatmap

```        var puzzleBeatmap = new PuzzleBeatmap();
        puzzleBeatmap.HitObjects.Add(new PuzzlePiece(1000, "triangle", 3));
        puzzleBeatmap.HitObjects.Add(new PuzzlePiece(2000, "square", 4));

        // 使用解析器 / Use parser
        var decoder = new PuzzleBeatmapDecoder();
        // 可以解析自定义格式的文件 / Can parse custom format files

        Console.WriteLine($"游戏模式: {puzzleBeatmap.Mode.Name}");
        Console.WriteLine($"打击对象数量: {puzzleBeatmap.HitObjects.Count}");

        // 享受标准API / Enjoy standard API
        var statistics = puzzleBeatmap.GetStatistics();
        foreach (var stat in statistics)
        {
            Console.WriteLine($"{stat.Name}: {stat.Content}");
        }
    }
}
```
