# LAsOsuBeatmapParser

一个轻量级、高性能（真的吗）的C#库，用于解析osu! .osu文件，构建类型安全的Beatmap对象模型。

A lightweight, high-performance(Really?) C# library for parsing osu! .osu files, building type-safe Beatmap object models.

注意：大部分api接口代码参考peppy的osu! 项目，本项目始于学习官方api。如果你喜欢本项目，请支持官方项目：https://github.com/ppy/osu

Note: Most API interfaces are referenced from peppy's osu! project. This project started as a way to learn the official API. If you like this project, please support the official project: https://github.com/ppy/osu

xxySR算法，参考：https://github.com/sunnyxxy/Star-Rating-Rebirth ，我做了并行、Span等优化，单谱解析大约100ms左右。

xxySR algorithm, reference: https://github.com/sunnyxxy/Star-Rating-Rebirth. I made optimizations with parallel processing, Span, etc., single beatmap parsing takes about 100ms.


## 功能特性 / Features

- **类型安全 / Type Safety**: 使用枚举的强类型对象，用于游戏模式和打击对象类型
- **模块化设计 / Modular Design**: 分离的解析器、模型和扩展
- **高性能 / High Performance**: 大文件的异步解析和流处理
- **可扩展 / Extensible**: 支持自定义解析逻辑和游戏模式扩展
- **全面 / Comprehensive**: 支持所有osu!游戏模式，特别关注Mania
- **自定义游戏模式 / Custom Game Modes**: 通过IGameMode接口支持无限扩展（ID必须大于3，避免与官方模式冲突）

## 安装 / Installation

```bash
dotnet add package LAsOsuBeatmapParser
```

## 使用方法 / Usage

有关详细的 API 使用说明，请参阅 [API 使用说明](API.md)。

For detailed API usage instructions, see [API Usage Guide](API.md).

### 基本解析 / Basic Parsing

```csharp
using LAsOsuBeatmapParser;

// 同步解析 / Synchronous parsing
var decoder = new BeatmapDecoder();
Beatmap beatmap = decoder.Decode("path/to/beatmap.osu");

// 异步解析 / Asynchronous parsing
Beatmap beatmap = await decoder.DecodeAsync("path/to/beatmap.osu");
```

### 处理Mania谱面 / Working with Mania Beatmaps

```csharp
// 获取Mania专用谱面 / Get Mania-specific beatmap
ManiaBeatmap maniaBeatmap = beatmap.GetManiaBeatmap();

// 访问Mania专用属性 / Access Mania-specific properties
int keyCount = maniaBeatmap.KeyCount;
double bpm = maniaBeatmap.BPM; // 现在是一个属性 / Now a property
var matrix = maniaBeatmap.Matrix; // 现在是一个属性 / Now a property
```

### 流畅API / Fluent API

```csharp
var bpm = new BeatmapDecoder()
    .Decode("beatmap.osu")
    .GetManiaBeatmap()
    .BPM;
```

### 自定义游戏模式扩展 / Custom Game Mode Extensions

参考 `GetManiaBeatmap()` 方法，为您的自定义游戏模式创建扩展方法：

```csharp
// 为您的自定义模式创建扩展方法
public static class MyExtensions
{
    public static MyBeatmap GetMyBeatmap<T>(this Beatmap<T> beatmap) where T : HitObject
    {
        if (beatmap.Mode.Id != 100) // 您的自定义ID
            throw new InvalidModeException("Beatmap mode must be My Custom Mode.");

        return new MyBeatmap { /* 转换逻辑 */ };
    }
}
```

### 序列化 / Serialization

```csharp
// 转换为JSON / To JSON
string json = JsonSerializer.Serialize(beatmap);

// 从JSON转换 / From JSON
Beatmap beatmap = JsonSerializer.Deserialize<Beatmap>(json);
```

### 验证 / Validation

```csharp
var errors = BeatmapValidator.Validate(beatmap);
if (errors.Any())
{
    // 处理验证错误 / Handle validation errors
}
```

## API参考 / API Reference

### 核心类 / Core Classes

- `Beatmap`: 核心谱面模型 / Core beatmap model
  - `BPM`: 计算的BPM属性 / Calculated BPM property
  - `Matrix`: 用于分析的时间-音符矩阵 / Time-note matrix for analysis
- `ManiaBeatmap`: Mania专用谱面模型 / Mania-specific beatmap model
- `BeatmapDecoder`: 主要解析器类 / Main parser class
- `BeatmapExtensions`: 扩展方法 / Extension methods

### 模型 / Models

- `BeatmapMetadata`: 标题、艺术家、创作者等 / Title, artist, creator, etc.
- `BeatmapDifficulty`: HP, CS, OD, AR等 / HP, CS, OD, AR, etc.
- `TimingPoint`: BPM和时间信息 / BPM and timing information
- `HitObject`: 打击对象的基类 / Base class for hit objects
  - `HitCircle`: 标准打击圆圈 / Standard hit circle
  - `Slider`: 滑条对象 / Slider object
  - `Spinner`: 转盘对象 / Spinner object
  - `ManiaHold`: Mania长按音符 / Mania hold note

### 枚举 / Enums

- `GameMode`: Standard, Taiko, Catch, Mania
- `HitObjectType`: Circle, Slider, Spinner, ManiaHold

## 要求 / Requirements

- .NET 6.0 或更高版本 / .NET 6.0 or later
- System.Text.Json (已包含) / System.Text.Json (included)

## 代码质量 / Code Quality

此库设计为可重用组件，因此一些公共API在库内部可能显示为"未使用"，但它们旨在供外部使用。

This library is designed as a reusable component, so some public APIs may appear "unused" within the library itself but are intended for external consumption.

### 警告抑制 / Warning Suppressions

库项目中故意抑制了以下编译器警告：

The following compiler warnings are intentionally suppressed in library projects:

- **CS0219**: 变量已赋值但其值从未使用 / Variable is assigned but its value is never used
- **CS0169**: 字段从未使用 / Field is never used
- **CS0414**: 字段已赋值但其值从未使用 / Field is assigned but its value is never used
- **CS0649**: 字段从未赋值，将始终具有其默认值 / Field is never assigned to, and will always have its default value

这些抑制在以下文件中配置：
- 项目文件: `src/LAsOsuBeatmapParser.csproj`
- EditorConfig: `.editorconfig`

### 处理未使用代码 / Handling Unused Code

对于库开发，请考虑以下方法：

For library development, consider these approaches:

1. **公共API**: 保留它们 - 它们供外部使用 / Keep them - they're for external use
2. **私有成员**: 对故意情况使用 `#pragma warning disable/restore` / Use `#pragma warning disable/restore` for intentional cases
3. **参数**: 为未使用的参数添加前缀 `_`: `void Method(int _unusedParam)` / Prefix with `_` for unused parameters: `void Method(int _unusedParam)`

## 贡献 / Contributing

欢迎贡献！请随时提交拉取请求或打开问题。

Contributions are welcome! Please feel free to submit pull requests or open issues.

## 许可证 / License

MIT License
