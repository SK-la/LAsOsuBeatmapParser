# Rust osu! Analysis

`rust_sr_calculator` is an independent Rust library and C-compatible library for osu! beatmap analysis. It consumes `.osu` bytes directly; it does not serialize maps or results through JSON. Public chart axes use typed enums, Ez2Lazer IDs, and dense `f64` vectors.

## Implemented foundation

- Strict UTF-8 `.osu` parsing from `&[u8]`, `Read`, `BufRead`, and file path, with source order retained for combo semantics.
- Four-mode raw chart data, timing/control points, break periods, samples, primary object types, slider source tags, and legacy difficulty normalization.
- Typed difficulty/performance attributes, mod descriptions, score result counts, and calculator traits.
- Mania native-map conversion, official difficulty and score PP, Sunny SR parity correction, Catch conversion/difficulty/score PP, and Taiko native plus osu! slider conversion/difficulty/score PP.
- PP+ chart axis calculators and perfect-play PP+ formulas for all four modes. osu! PP+ takes playable geometry.
- Mina chart and play axes using the same patched MinaCalc 0.74.0 binary as Ez2Lazer, hosted by the `wasmi` interpreter.
- KPS graph and column statistics, native Mania measurements, and beatmap tag extraction.
- CSI Mania chart feature extraction and full RC/LN pattern classification (`EzPatternAnalyzer`).
- LeoBlack Mania row, primitive, pattern, cluster, category, and weighted share analysis (`EzLeoBlackPatternAnalyzer`).
- Four-key JackDemand classification using CSI pattern and LeoBlack cluster measurements.
- Sunny SR4 chart skill extraction, persisted IDs, and radar axis layout.
- Sunny Dan interval lookup/fallback estimates, chart-side RC/LN estimates, and pure skillset bucket filing.
- Shared slider path geometry and slider-event generation used by ruleset converters.

Parity work remains. The Mania key-pattern helper family is still being ported; C ABI coverage for algorithm outputs also needs expansion beyond difficulty and Mina axes for a future Ez2Lazer integration. Player clear-history aggregation, Realm/database/UI integration, and persistent cache behavior stay in Ez2Lazer and are outside this standalone chart library. Dan skillset filing accepts CSI and LeoBlack features as typed inputs so the passes compose without serialization. The low-level `osu::OsuDifficultyCalculator` accepts evaluator features; `osu_converter::OsuDifficultyCalculator` builds those features from a native `.osu` map. Mirrored osu!standard sliders are explicitly unsupported. Ruleset-specific mod support is limited to the mods implemented by each calculator.

## Rust API

```rust,ignore
use rust_sr_calculator::beatmap::OsuBeatmap;
use rust_sr_calculator::calculator::calculate_difficulty;

let beatmap = OsuBeatmap::parse_bytes(osu_file_bytes)?;
let difficulty = calculate_difficulty(&beatmap, &[])?;
```

The runtime-selected calculator follows Ez2Lazer's ruleset calculator split and returns typed difficulty attributes. Concrete difficulty and performance calculators also implement the matching Rust traits. Ruleset calculators accept typed `Mod` values; algorithm-specific chart skills return dense vectors indexed by enums whose `id()` values match Ez2Lazer's persisted IDs. CSI, LeoBlack, Dan estimate, and Dan filing remain separate typed passes, matching their Ez2Lazer analyzer boundaries.

Native callers can use `ez_chart_difficulty_from_bytes_v1` with a borrowed byte buffer and an output record containing a `double` star rating followed by two 32-bit integers for maximum combo and ruleset ID. Mania Mina axes are also available from `ez_mina_chart_axes_from_bytes_v1`; its output is eight `f64` values in the stable Overall, Stream, Jumpstream, Handstream, Stamina, Jack, Chordjack, Technical order. Both V1 byte APIs avoid serialization and leave output untouched when parsing or calculation fails.

## Build and tests

```sh
cargo check --tests --manifest-path LAsRustSRCalculator/Cargo.toml
cargo test --manifest-path LAsRustSRCalculator/Cargo.toml
```

The crate builds as both `rlib` and `cdylib`. Current Windows test runs may need an MSVC linker; the repository's CI can run the ordinary `cargo test` command on a native Rust target.
