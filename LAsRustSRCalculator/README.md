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
- CSI Mania chart feature extraction and Sunny Dan interval lookup/fallback estimates.
- Shared slider path geometry and slider-event generation used by ruleset converters.

The crate is still under migration. CSI feature measurements and Sunny Dan mapping are present, but CSI pattern classification, LeoBlack, Dan skillset buckets/ladders/player aggregation remain pending. The low-level `osu::OsuDifficultyCalculator` accepts evaluator features; the `osu_converter::OsuDifficultyCalculator` entry builds those features from a native `.osu` map. Mirrored sliders are explicitly unsupported. Ruleset-specific mod support is limited to the mods implemented by each calculator.

## Rust API

```rust,ignore
let beatmap = OsuBeatmap::parse_bytes(osu_file_bytes)?;
let difficulty = ManiaDifficultyCalculator.calculate(&beatmap, &[])?;
```

The ruleset calculators accept typed `Mod` values and return typed difficulty attributes. Algorithm-specific chart skills return dense vectors indexed by enums whose `id()` values match Ez2Lazer's persisted IDs.

Native callers can use `ez_chart_difficulty_from_bytes_v1` with a borrowed byte buffer and an output record containing a `double` star rating followed by two 32-bit integers for maximum combo and ruleset ID. Mania Mina axes are also available from `ez_mina_chart_axes_from_bytes_v1`; its output is eight `f64` values in the stable Overall, Stream, Jumpstream, Handstream, Stamina, Jack, Chordjack, Technical order. Both V1 byte APIs avoid serialization and leave output untouched when parsing or calculation fails.

## Build and tests

```sh
cargo check --tests --manifest-path LAsRustSRCalculator/Cargo.toml
cargo test --manifest-path LAsRustSRCalculator/Cargo.toml
```

The crate builds as both `rlib` and `cdylib`. Current Windows test runs may need an MSVC linker; the repository's CI can run the ordinary `cargo test` command on a native Rust target.
