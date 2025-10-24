# API 使用说明 / API Usage Guide

本指南提供了 LAsOsuBeatmapParser 库的详细 API 使用说明，包括代码示例和最佳实践。

This guide provides detailed API usage instructions for the LAsOsuBeatmapParser library, including code examples and best practices.

## 目录 / Table of Contents

- [快速开始 / Quick Start](#快速开始--quick-start)
- [解析谱面 / Parsing Beatmaps](#解析谱面--parsing-beatmaps)
- [访问谱面数据 / Accessing Beatmap Data](#访问谱面数据--accessing-beatmap-data)
- [Mania 模式特殊功能 / Mania Mode Special Features](#mania-模式特殊功能--mania-mode-special-features)
- [序列化和反序列化 / Serialization and Deserialization](#序列化和反序列化--serialization-and-deserialization)
- [验证和错误处理 / Validation and Error Handling](#验证和错误处理--validation-and-error-handling)
- [高级用法 / Advanced Usage](#高级用法--advanced-usage)
- [自定义游戏模式扩展 / Custom Game Mode Extension](#自定义游戏模式扩展--custom-game-mode-extension)

## 快速开始 / Quick Start

### 安装包 / Installing the Package

```bash
dotnet add package LAsOsuBeatmapParser
```

### 基本用法 / Basic Usage

```csharp
using LAsOsuBeatmapParser;

// 创建解码器 / Create decoder
var decoder = new BeatmapDecoder();

// 解析谱面文件 / Parse beatmap file
Beatmap beatmap = decoder.Decode("path/to/beatmap.osu");

// 访问基本信息 / Access basic information
Console.WriteLine($"标题: {beatmap.Metadata.Title} / Title: {beatmap.Metadata.Title}");
Console.WriteLine($"艺术家: {beatmap.Metadata.Artist} / Artist: {beatmap.Metadata.Artist}");
Console.WriteLine($"打击对象数量: {beatmap.HitObjects.Count} / Hit Objects Count: {beatmap.HitObjects.Count}");
```

## 解析谱面 / Parsing Beatmaps

### 同步解析 / Synchronous Parsing

```csharp
var decoder = new BeatmapDecoder();
Beatmap beatmap = decoder.Decode("beatmap.osu");
```

### 异步解析 / Asynchronous Parsing

```csharp
var decoder = new BeatmapDecoder();
Beatmap beatmap = await decoder.DecodeAsync("beatmap.osu");
```

### 从流解析 / Parsing from Stream

```csharp
using var stream = File.OpenRead("beatmap.osu");
var decoder = new BeatmapDecoder();
Beatmap beatmap = decoder.Decode(stream);
```

### 从字符串解析 / Parsing from String

```csharp
string osuFileContent = File.ReadAllText("beatmap.osu");
var decoder = new BeatmapDecoder();
Beatmap beatmap = decoder.DecodeFromString(osuFileContent);
```

## 访问谱面数据 / Accessing Beatmap Data

### 基本属性 / Basic Properties

```csharp
// 元数据 / Metadata
string title = beatmap.Metadata.Title;
string artist = beatmap.Metadata.Artist;
string creator = beatmap.Metadata.Creator;

// 难度设置 / Difficulty Settings
double hp = beatmap.Difficulty.HPDrainRate;
double cs = beatmap.Difficulty.CircleSize;
double od = beatmap.Difficulty.OverallDifficulty;
double ar = beatmap.Difficulty.ApproachRate;

// 游戏模式 / Game Mode
GameMode mode = beatmap.Metadata.GameMode;

// 打击对象 / Hit Objects
IReadOnlyList<HitObject> hitObjects = beatmap.HitObjects;
```

### 时间点和控制点 / Timing Points and Control Points

```csharp
// 访问时间点 / Access timing points
foreach (var timingPoint in beatmap.ControlPointInfo.TimingPoints)
{
    Console.WriteLine($"时间: {timingPoint.Time}, BPM: {timingPoint.BPM}");
}

// 访问控制点 / Access control points
var controlPoints = beatmap.ControlPointInfo;
```

### 统计信息 / Statistics

```csharp
// 获取谱面统计 / Get beatmap statistics
var statistics = beatmap.GetStatistics();
foreach (var stat in statistics)
{
    Console.WriteLine($"{stat.Name}: {stat.Content}");
}

// 最常见的节拍长度 / Most common beat length
double commonBeatLength = beatmap.GetMostCommonBeatLength();

// 休息时间总和 / Total break time
double totalBreakTime = beatmap.TotalBreakTime;
```

## Mania 模式特殊功能 / Mania Mode Special Features

### 获取 Mania 谱面 / Getting Mania Beatmap

```csharp
ManiaBeatmap maniaBeatmap = beatmap.GetManiaBeatmap();
```

### Mania 专用属性 / Mania-Specific Properties

```csharp
// 键数 / Key Count
int keyCount = maniaBeatmap.KeyCount;

// BPM / BPM
double bpm = maniaBeatmap.BPM;

// 时间-音符矩阵 / Time-Note Matrix
var matrix = maniaBeatmap.Matrix;

// 遍历矩阵 / Iterate through matrix
foreach (var (time, notes) in matrix)
{
    Console.WriteLine($"时间 {time}: 音符 {string.Join(",", notes)}");
}
```

### 谱面难度分析 / Beatmap Difficulty Analysis

```csharp
// 使用 SR 计算器 / Using SR Calculator
var calculator = SRCalculator.Instance;
double sr = calculator.CalculateSR(maniaBeatmap);
```

## 序列化和反序列化 / Serialization and Deserialization

### JSON 序列化 / JSON Serialization

```csharp
using System.Text.Json;

// 序列化为 JSON / Serialize to JSON
string json = JsonSerializer.Serialize(beatmap, new JsonSerializerOptions
{
    WriteIndented = true
});

// 从 JSON 反序列化 / Deserialize from JSON
Beatmap deserializedBeatmap = JsonSerializer.Deserialize<Beatmap>(json);
```

### 自定义序列化选项 / Custom Serialization Options

```csharp
var options = new JsonSerializerOptions
{
    WriteIndented = true,
    Converters = { new HitObjectListConverter() }
};

string json = JsonSerializer.Serialize(beatmap, options);
```

## 验证和错误处理 / Validation and Error Handling

### 验证谱面 / Validating Beatmap

```csharp
var errors = BeatmapValidator.Validate(beatmap);
if (errors.Any())
{
    foreach (var error in errors)
    {
        Console.WriteLine($"验证错误: {error}");
    }
}
else
{
    Console.WriteLine("谱面验证通过");
}
```

### 异常处理 / Exception Handling

```csharp
try
{
    Beatmap beatmap = decoder.Decode("beatmap.osu");
}
catch (BeatmapParseException ex)
{
    Console.WriteLine($"解析错误: {ex.Message}");
}
catch (BeatmapInvalidForRulesetException ex)
{
    Console.WriteLine($"规则集无效: {ex.Message}");
}
```

## 高级用法 / Advanced Usage

### 扩展方法 / Extension Methods

```csharp
using LAsOsuBeatmapParser.Extensions;

// 使用扩展方法 / Using extension methods
var filteredObjects = beatmap.HitObjects.WhereByTime(1000, 5000);
var circles = beatmap.HitObjects.OfType<HitCircle>();
```

### 自定义解析 / Custom Parsing

```csharp
// 实现自定义解析器 / Implement custom parser
public class CustomBeatmapDecoder : BeatmapDecoder
{
    protected override void ParseSection(string sectionName, string[] lines)
    {
        // 自定义解析逻辑 / Custom parsing logic
        base.ParseSection(sectionName, lines);
    }
}
```

### 性能优化 / Performance Optimization

```csharp
// 对于大文件，使用异步解析 / For large files, use async parsing
Beatmap beatmap = await decoder.DecodeAsync("large_beatmap.osu");

// 重用解码器实例 / Reuse decoder instances
var decoder = new BeatmapDecoder();
for (int i = 0; i < 100; i++)
{
    Beatmap beatmap = decoder.Decode($"beatmap_{i}.osu");
}
```

### 内存管理 / Memory Management

```csharp
// 使用 using 语句确保资源释放 / Use using statement to ensure resource disposal
using var stream = File.OpenRead("beatmap.osu");
Beatmap beatmap = decoder.Decode(stream);
```

## 常见问题 / Frequently Asked Questions

### Q: 如何处理不同游戏模式的谱面？ / How to handle beatmaps of different game modes?

A: 使用 `beatmap.Metadata.GameMode` 检查模式，然后转换为相应类型。
Use `beatmap.Metadata.GameMode` to check the mode, then cast to the appropriate type.

```csharp
switch (beatmap.Metadata.GameMode)
{
    case GameMode.Standard:
        // 处理标准模式 / Handle standard mode
        break;
    case GameMode.Mania:
        var mania = beatmap.GetManiaBeatmap();
        // 处理 Mania 模式 / Handle mania mode
        break;
}
```

### Q: 如何计算谱面的难度？ / How to calculate beatmap difficulty?

A: 使用 SRCalculator 类。
Use the SRCalculator class.

```csharp
double starRating = SRCalculator.Instance.CalculateSR(beatmap);
```

### Q: 支持哪些文件格式？ / What file formats are supported?

A: 目前只支持 .osu 文件格式。
Currently only .osu file format is supported.

## 更多资源 / More Resources

- [项目主页 / Project Homepage](https://github.com/SK-la/LAsOsuBeatmapParser)
- [问题跟踪 / Issue Tracker](https://github.com/SK-la/LAsOsuBeatmapParser/issues)
- [osu! 官方文档 / osu! Official Documentation](https://osu.ppy.sh/wiki)

## 自定义游戏模式扩展 / Custom Game Mode Extension

LAsOsuBeatmapParser 支持扩展自定义游戏模式，允许您创建自己的谱面格式和解析逻辑。

**重要：游戏模式ID规则 / Important: Game Mode ID Rules**

osu! 官方游戏模式已占用以下ID：
- `0`: Standard (STD)
- `1`: Taiko
- `2`: Catch (CTB)
- `3`: Mania

**自定义游戏模式必须使用大于3的ID / Custom game modes must use IDs greater than 3**

### 实现自定义游戏模式 / Implementing Custom Game Modes

要创建自定义游戏模式，实现 `IGameMode` 接口：

```csharp
using LAsOsuBeatmapParser.Beatmaps;

public class CustomGameMode : IGameMode
{
    public int Id { get; }
    public string Name { get; }

    public CustomGameMode(int id, string name)
    {
        // 确保ID大于3 / Ensure ID is greater than 3
        if (id <= 3)
            throw new ArgumentException("Custom game mode IDs must be greater than 3", nameof(id));

        Id = id;
        Name = name;
    }

    public override string ToString() => Name;
}
```

### 创建自定义谱面类型 / Creating Custom Beatmap Types

为您的自定义游戏模式创建谱面类：

```csharp
using LAsOsuBeatmapParser.Beatmaps;

public class CustomBeatmap : Beatmap<CustomHitObject>
{
    public CustomBeatmap()
    {
        Mode = new CustomGameMode(100, "Custom Mode");
        // 初始化其他属性
    }
}

public class CustomHitObject : HitObject
{
    // 实现您的自定义打击对象逻辑
}
```

### 自定义解析逻辑 / Custom Parsing Logic

扩展解析器以支持您的自定义模式：

```csharp
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Beatmaps.Formats;

public class CustomBeatmapDecoder : LegacyBeatmapDecoder
{
    protected override HitObject ParseHitObject(string[] parts)
    {
        // 检查是否是您的自定义模式
        if (CurrentBeatmap.Mode.Id == 100)
        {
            // 实现自定义解析逻辑
            return ParseCustomHitObject(parts);
        }

        // 回退到默认解析
        return base.ParseHitObject(parts);
    }

    private CustomHitObject ParseCustomHitObject(string[] parts)
    {
        // 您的自定义解析实现
        return new CustomHitObject();
    }
}
```

### 使用自定义解析器 / Using Custom Parsers

```csharp
var customDecoder = new CustomBeatmapDecoder();
Beatmap beatmap = customDecoder.Decode("custom_beatmap.osu");

// 访问自定义模式
if (beatmap.Mode.Id == 100)
{
    var customBeatmap = (CustomBeatmap)beatmap;
    // 使用您的自定义API
}
```

### 扩展优势 / Extension Benefits

- **完全可扩展**: 支持任何数量的自定义游戏模式
- **向后兼容**: 现有代码无需修改
- **类型安全**: 强类型支持自定义打击对象
- **API一致性**: 享受与标准模式相同的解析库功能

### 为自定义模式创建扩展方法 / Creating Extension Methods for Custom Modes

参考 `GetManiaBeatmap()` 的实现，为您的自定义游戏模式创建类似的扩展方法：

```csharp
using LAsOsuBeatmapParser.Beatmaps;

public static class CustomBeatmapExtensions
{
    /// <summary>
    /// 从 Beatmap 获取 PuzzleBeatmap，并验证模式。
    /// </summary>
    public static PuzzleBeatmap GetPuzzleBeatmap<T>(this Beatmap<T> beatmap) where T : HitObject
    {
        // 如果已经是PuzzleBeatmap，直接返回
        if (typeof(T) == typeof(PuzzleHitObject) && beatmap is PuzzleBeatmap puzzleBeatmap)
        {
            return puzzleBeatmap;
        }

        // 验证模式ID
        if (beatmap.Mode.Id != 100) // 您的自定义模式ID
        {
            throw new InvalidModeException("Beatmap mode must be Puzzle Mode.");
        }

        // 创建并返回自定义谱面
        return new PuzzleBeatmap
        {
            BeatmapInfo = beatmap.BeatmapInfo,
            ControlPointInfo = beatmap.ControlPointInfo,
            Breaks = beatmap.Breaks,
            MetadataLegacy = beatmap.MetadataLegacy,
            DifficultyLegacy = beatmap.DifficultyLegacy,
            TimingPoints = beatmap.TimingPoints,
            HitObjects = beatmap.HitObjects.Select(h => ConvertToPuzzleHitObject(h)).ToList(),
            Events = beatmap.Events,
            Mode = beatmap.Mode,
            Version = beatmap.Version
            // 设置其他PuzzleBeatmap特定的属性
        };
    }

    private static PuzzleHitObject ConvertToPuzzleHitObject(HitObject hitObject)
    {
        // 实现从通用HitObject到PuzzleHitObject的转换逻辑
        return new PuzzleHitObject(hitObject.StartTime, "default_shape", 4);
    }
}
```

### 使用自定义扩展方法 / Using Custom Extension Methods

```csharp
// 使用您的自定义扩展方法
var decoder = new BeatmapDecoder();
Beatmap beatmap = decoder.Decode("puzzle.osu");

PuzzleBeatmap puzzleBeatmap = beatmap.GetPuzzleBeatmap();
// 现在可以使用PuzzleBeatmap的所有功能
```
