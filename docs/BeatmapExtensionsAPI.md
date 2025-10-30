# BeatmapExtensions API 文档

本文档描述了 `BeatmapExtensions.cs` 中定义的所有扩展方法。这些方法扩展了 `Beatmap` 类，提供谱面分析、转换和处理功能。

## 接口和类

### IApplyBeatmap 接口

**描述**：  
应用于 Beatmap 的转换接口，类似于官方的 IApplyBeatmap。实现此接口的类可以修改 Beatmap 的属性，如 HitObject、TimingPoints 等。

**方法**：  

- `void ApplyToBeatmap(Beatmap beatmap)`: 将转换应用到指定的 Beatmap。

### IApplyToHitObject 接口

**描述**：  
应用于单个 HitObject 的转换接口。实现此接口的类可以修改 HitObject 的属性，如位置、时间等。

**方法**：  

- `void ApplyToHitObject(HitObject hitObject)`: 将转换应用到指定的 HitObject。

### IApplyToMetadata 接口

**描述**：  
应用于 Beatmap Metadata 的转换接口。实现此接口的类可以修改 Metadata 的属性，如标题、艺术家等。

**方法**：  

- `void ApplyToMetadata(BeatmapMetadata metadata)`: 将转换应用到指定的 Metadata。

### IApplyToDifficulty 接口

**描述**：  
应用于 Beatmap Difficulty 的转换接口。实现此接口的类可以修改 Difficulty 的属性，如 OverallDifficulty、ApproachRate 等。

**方法**：  

- `void ApplyToDifficulty(BeatmapDifficulty difficulty)`: 将转换应用到指定的 Difficulty。

### ApplyBeatmapBase 抽象类

**描述**：  
Beatmap 转换的基类，提供基础功能如复制属性。子类可以重写 ApplyToBeatmap，并在开头调用 base.ApplyToBeatmap() 以利用基础功能。

**方法**：  

- `virtual void ApplyToBeatmap(Beatmap beatmap)`: 基础实现，可以在这里复制或初始化属性。

### ApplyBeatGridTransformation 抽象类

**描述**：  
基于节拍网格的 Beatmap 转换抽象类。继承此类并重写 ApplyTransformation 方法来实现具体转换逻辑。在重写 ApplyToBeatmap 时，先调用 base.ApplyToBeatmap(beatmap) 以利用基础功能。

**属性**：  

- `int BeatsPerMeasure`: 每小节拍数（默认4）。
- `int Subdivisions`: 每拍细分数（默认4）。

**方法**：  

- `override void ApplyToBeatmap(Beatmap beatmap)`: 构建网格矩阵，对每段调用 ApplyTransformation。
- `abstract void ApplyTransformation(List<HitObject> notesInSegment, double gridStartTime)`: 子类重写，实现具体转换逻辑。

### ApplyBeatGridTransformation 类

**描述**：  
基于节拍网格的 Beatmap 转换类。使用指定的 IApplyToHitObject 转换器对网格中的 HitObject 应用转换。

**属性**：  

- `int BeatsPerMeasure`: 每小节拍数（默认4）。
- `int Subdivisions`: 每拍细分数（默认4）。
- `IApplyToHitObject Transformer`: 用于转换 HitObject 的转换器。

**构造函数**：  

- `ApplyBeatGridTransformation(IApplyToHitObject transformer)`: 创建实例，传入转换器。

**方法**：  

- `override void ApplyToBeatmap(Beatmap beatmap)`: 构建网格矩阵，对每段内的 HitObject 调用 Transformer.ApplyToHitObject。

**示例**：  

```csharp
public class HorizontalFlipTransformer : IApplyToHitObject
{
    public void ApplyToHitObject(HitObject hitObject)
    {
        // 水平翻转
        hitObject.Position = new Vector2(512 - hitObject.Position.X, hitObject.Position.Y);
    }
}

// 使用
var transformer = new ApplyBeatGridTransformation(new HorizontalFlipTransformer());
transformer.ApplyToBeatmap(beatmap);
```

**示例**：  

```csharp
public class HorizontalFlipTransformation : ApplyBeatGridTransformation
{
    protected override void ApplyTransformation(List<HitObject> notesInSegment, double gridStartTime)
    {
        foreach (var note in notesInSegment)
        {
            note.Position = new Vector2(512 - note.Position.X, note.Position.Y);
        }
    }
}

// 使用
var transformer = new HorizontalFlipTransformation();
transformer.ApplyToBeatmap(beatmap);
```

## 方法列表

### 1. GetManiaBeatmap<T>(this Beatmap<T> beatmap)

**描述**：  
从通用 `Beatmap<T>` 获取 `ManiaBeatmap`，并验证游戏模式是否为 Mania。如果已经是 Mania 模式，直接返回；否则转换 HitObject 为 ManiaHitObject。

**参数**：  

- `beatmap`: 谱面对象（类型 `Beatmap<T>`，其中 T 继承自 `HitObject`）。

**返回值**：  

- `Beatmap<ManiaHitObject>`: 转换后的 Mania 谱面对象。

**异常**：  

- `InvalidModeException`: 如果谱面模式不是 Mania，则抛出异常。

**示例**：  

```csharp
var maniaBeatmap = beatmap.GetManiaBeatmap();
```

### 2. GetBPM(this Beatmap beatmap)

**描述**：  
获取谱面的 BPM（每分钟拍数）。直接返回 `beatmap.BPM` 属性。

**参数**：  

- `beatmap`: 谱面对象。

**返回值**：  

- `double`: BPM 数值。

**示例**：  

```csharp
double bpm = beatmap.GetBPM();
```

### 3. BuildMatrix(this Beatmap beatmap)

**描述**：  
按需构建时间-音符矩阵，将 HitObject 按 `StartTime` 分组。键为 `StartTime`，值为该时间的 HitObject 列表。

**参数**：  

- `beatmap`: 谱面对象。

**返回值**：  

- `Dictionary<double, List<HitObject>>`: 时间到音符列表的字典。

**示例**：  

```csharp
var matrix = beatmap.BuildMatrix();
foreach (var kvp in matrix)
{
    double time = kvp.Key;
    List<HitObject> notes = kvp.Value;
    // 处理同时间的音符
}
```

### 4. GetMatrix(this Beatmap beatmap)

**描述**：  
获取用于分析的时间-音符矩阵（按需构建）。内部调用 `BuildMatrix` 方法。

**参数**：  

- `beatmap`: 谱面对象。

**返回值**：  

- `Dictionary<double, List<HitObject>>`: 时间到音符列表的字典。

**示例**：  

```csharp
var matrix = beatmap.GetMatrix();
// 与 BuildMatrix 用法相同
```

### 5. FilterHitObjects(this Beatmap beatmap, Func<HitObject, bool> predicate)

**描述**：  
根据条件过滤谱面中的 HitObject，返回符合条件的音符集合。

**参数**：  

- `beatmap`: 谱面对象。
- `predicate`: 过滤条件函数，接受 `HitObject` 并返回 `bool`。

**返回值**：  

- `IEnumerable<HitObject>`: 符合条件的音符集合。

**示例**：  

```csharp
var filteredNotes = beatmap.FilterHitObjects(note => note.StartTime > 1000);
// 获取 StartTime > 1000ms 的音符
```

### 6. GetMaxCombo(this Beatmap beatmap)

**描述**：  
获取谱面的最大连击数。目前简化实现，返回 HitObject 总数（不考虑实际连击逻辑）。

**参数**：  

- `beatmap`: 谱面对象。

**返回值**：  

- `int`: 最大连击数。

**示例**：  

```csharp
int maxCombo = beatmap.GetMaxCombo();
```

### 7. GetPlayableLength(this Beatmap beatmap)

**描述**：  
获取谱面的可玩时长（从第一个 HitObject 的 StartTime 到最后一个的 EndTime）。

**参数**：  

- `beatmap`: 谱面对象。

**返回值**：  

- `double`: 可玩时长（毫秒）。如果无 HitObject，返回 0。

**示例**：  

```csharp
double length = beatmap.GetPlayableLength();
```

### 8. BuildBeatGridMatrix(this Beatmap beatmap, int beatsPerMeasure = 4, int subdivisions = 4)

**描述**：  
按节拍网格构建时间矩阵，用于分段处理 HitObject。基于 TimingPoints 计算网格节点，将 HitObject 按 `StartTime` 分配到最近的网格开始时间（小于等于 StartTime 的最大网格节点）。支持 BPM 区间、向前回滚和余数处理。

**参数**：  

- `beatmap`: 谱面对象。
- `beatsPerMeasure`: 每小节拍数（默认 4，即 4/4 拍）。
- `subdivisions`: 每拍细分数（节拍细度，默认 4，即每拍 4 个网格）。

**返回值**：  

- `Dictionary<double, List<HitObject>>`: 网格开始时间到 HitObject 列表的字典。键为网格开始时间（毫秒），值为该网格段内的 HitObject。

**示例**：  

```csharp
var gridMatrix = beatmap.BuildBeatGridMatrix(beatsPerMeasure: 4, subdivisions: 4);
foreach (var kvp in gridMatrix)
{
    double gridStartTime = kvp.Key;  // 网格开始时间
    List<HitObject> notesInSegment = kvp.Value;  // 该段内的音符

    // 示例：水平翻转该段内的音符
    foreach (var note in notesInSegment)
    {
        note.Position = new Vector2(512 - note.Position.X, note.Position.Y);
    }
}
```

## 注意事项

- 所有方法都是静态扩展方法，使用 `this Beatmap` 参数。
- `BuildBeatGridMatrix` 处理 BPM 变化区间，但余数区间处理标记为 TODO，可能需要进一步完善。
- 使用前确保谱面数据完整（例如 TimingPoints 和 HitObjects）。
- 示例代码假设相关类（如 `Vector2`）已导入。
