# LAsOsuBeatmapParser

A lightweight, high-performance C# library for parsing osu! .osu files, building type-safe Beatmap object models.

## Features

- **Type Safety**: Strong-typed objects with enums for game modes and hit object types
- **Modular Design**: Separated parsers, models, and extensions
- **High Performance**: Asynchronous parsing and stream processing for large files
- **Extensible**: Support for custom parsing logic and game mode extensions
- **Comprehensive**: Supports all osu! game modes, with special focus on Mania

## Installation

```bash
dotnet add package LAsOsuBeatmapParser
```

## Usage

### Basic Parsing

```csharp
using LAsOsuBeatmapParser;

// Synchronous parsing
var decoder = new BeatmapDecoder();
Beatmap beatmap = decoder.Decode("path/to/beatmap.osu");

// Asynchronous parsing
Beatmap beatmap = await decoder.DecodeAsync("path/to/beatmap.osu");
```

### Working with Mania Beatmaps

```csharp
// Get Mania-specific beatmap
ManiaBeatmap maniaBeatmap = beatmap.GetManiaBeatmap();

// Access Mania-specific properties
int keyCount = maniaBeatmap.KeyCount;
double bpm = maniaBeatmap.BPM; // Now a property
var matrix = maniaBeatmap.Matrix; // Now a property
```

### Fluent API

```csharp
var bpm = new BeatmapDecoder()
    .Decode("beatmap.osu")
    .GetManiaBeatmap()
    .BPM;
```

### Serialization

```csharp
// To JSON
string json = JsonSerializer.Serialize(beatmap);

// From JSON
Beatmap beatmap = JsonSerializer.Deserialize<Beatmap>(json);
```

### Validation

```csharp
var errors = BeatmapValidator.Validate(beatmap);
if (errors.Any())
{
    // Handle validation errors
}
```

## API Reference

### Core Classes

- `Beatmap`: Core beatmap model
  - `BPM`: Calculated BPM property
  - `Matrix`: Time-note matrix for analysis
- `ManiaBeatmap`: Mania-specific beatmap model
- `BeatmapDecoder`: Main parser class
- `BeatmapExtensions`: Extension methods

### Models

- `BeatmapMetadata`: Title, artist, creator, etc.
- `BeatmapDifficulty`: HP, CS, OD, AR, etc.
- `TimingPoint`: BPM and timing information
- `HitObject`: Base class for hit objects
  - `HitCircle`: Standard hit circle
  - `Slider`: Slider object
  - `Spinner`: Spinner object
  - `ManiaHold`: Mania hold note

### Enums

- `GameMode`: Standard, Taiko, Catch, Mania
- `HitObjectType`: Circle, Slider, Spinner, ManiaHold

## Requirements

- .NET 6.0 or later
- System.Text.Json (included)

## Code Quality

This library is designed as a reusable component, so some public APIs may appear "unused" within the library itself but are intended for external consumption.

### Warning Suppressions

The following compiler warnings are intentionally suppressed in library projects:

- **CS0219**: Variable is assigned but its value is never used
- **CS0169**: Field is never used
- **CS0414**: Field is assigned but its value is never used
- **CS0649**: Field is never assigned to, and will always have its default value

These suppressions are configured in:
- Project file: `src/LAsOsuBeatmapParser.csproj`
- EditorConfig: `.editorconfig`

### Handling Unused Code

For library development, consider these approaches:

1. **Public APIs**: Keep them - they're for external use
2. **Private members**: Use `#pragma warning disable/restore` for intentional cases
3. **Parameters**: Prefix with `_` for unused parameters: `void Method(int _unusedParam)`

## Contributing

Contributions are welcome! Please feel free to submit pull requests or open issues.

## License

MIT License
