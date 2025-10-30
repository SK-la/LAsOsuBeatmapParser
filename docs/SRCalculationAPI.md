# SR计算API说明文档

## 概述

本库提供了星级评分（Star Rating, SR）计算功能，支持Mania模式的谱面SR计算。SR计算基于[xxy的Star-Rating-Rebirth库](https://github.com/sunnyxxy/Star-Rating-Rebirth)的C#移植版本，并支持自定义交叉矩阵以适应不同需求。

## 主要类

### SRCalculator

`SRCalculator` 是SR计算的核心类，提供同步和异步的SR计算方法。

#### SRCalculator 方法

- `double CalculateSR<T>(IBeatmap<T> beatmap, out Dictionary<string, long> times)`  
  同步计算谱面的SR值。  
  - `beatmap`: 谱面对象。  
  - `times`: 输出计算耗时。  
  - 返回: SR值。

- `Task<(double sr, Dictionary<string, long> times)> CalculateSRAsync<T>(IBeatmap<T> beatmap)`  
  异步计算谱面的SR值。

- `double CalculateSRFromFileCS(string filePath)`  
  从.osu文件路径计算SR。  
  - `filePath`: .osu文件路径。  
  - 返回: SR值或负数错误码（失败）。

- `double CalculateSRFromContentCS(string content)`  
  从.osu文件内容字符串计算SR。  
  - `content`: .osu文件内容。  
  - 返回: SR值或负数错误码（失败）。

### SRCalculatorRust

`SRCalculatorRust` 提供基于Rust实现的SR计算器，性能为C#算法的2倍，且几乎无内存压力。

#### SRCalculatorRust 方法

- `double CalculateSR_FromFile(string filePath)`  
  从文件路径计算SR，使用Rust实现。  
  - 返回: SR值或负数错误码（失败）。

## 自定义功能

### CrossMatrixProvider

`CrossMatrixProvider` 允许自定义交叉矩阵，用于覆盖默认的键位权重分布。

#### 方法

- `double[]? GetMatrix(int K)`获取指定键数K的交叉矩阵。

  - `K`: 键数（1开始）。
  - 返回: 矩阵数组或null（不支持）。
- `void SetCustomMatrix(int K, double[] matrix)`设置自定义交叉矩阵。

  - `K`: 键数。
  - `matrix`: 自定义矩阵数组。

#### 示例

```csharp
// 设置4K的自定义矩阵
double[] custom4K = { 0.2, 0.3, 0.2, 0.3 };
CrossMatrixProvider.SetCustomMatrix(4, custom4K);

// 计算SR时会使用自定义矩阵
double sr = SRCalculator.Instance.CalculateSR(beatmap, out var times);

// 清除自定义矩阵，恢复默认
CrossMatrixProvider.SetCustomMatrix(4, null);
```

### 注意事项

- 自定义矩阵必须与键数匹配长度。
- 如果不支持的键数，GetMatrix返回null，SR计算会抛出异常并记录日志。
- 默认矩阵支持K=1到18，但奇数K>10标记为不支持。

## 错误处理

SR计算方法会捕获异常并返回错误值。所有实现使用统一的错误码系统：

### 统一错误码

- `-2.0`: 路径字符串无效
- `-3.0`: 文件打开失败或文件不存在
- `-4.0`: 解析失败或异常
- `-5.0`: 数据非法或非Mania模式
- `-6.0`: SR计算失败
- `-7.0`: SR计算panic
- 其他负值: 未知错误

### SRCalculator（C#实现）

`CalculateSRFromFileCS` 返回负数错误码表示失败，使用上述统一错误码。

### SRCalculatorRust（Rust实现）

`CalculateSR_FromFile` 返回负数错误码表示失败，使用上述统一错误码。

检查控制台日志以获取详细错误信息。
