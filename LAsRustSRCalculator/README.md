# Rust osu! Analysis

`rust_sr_calculator` is a standalone Rust crate and C-compatible library for osu! beatmap analysis. The target is parity with Ez2Lazer's four `.osu` rulesets and chart-level analysis APIs, with the same skill-axis IDs and output order. Porting is in progress; the current crate is not yet at full Ez2Lazer feature or numerical parity.

## Current foundation

- Parse `.osu` data from a byte slice, any `Read`/`BufRead` stream, or a file path.
- Represent standard, taiko, catch, and mania maps, their timing/control points, breaks, hit samples, and primary hit-object fields.
- Expose typed Sunny SR, Mina, and PPP skill axes with stable IDs, canonical ordering, and dense `f64` vectors.
- Retain the existing mania SR calculator and C ABI while the four-ruleset analysis API is ported.

Visual storyboard commands, external sample files, and derived slider duration/nested-object generation are handled outside the raw parser model and remain part of the analysis/conversion work.

## Rust API

The parser reads directly from memory without an intermediate text serialization:

```rust,ignore
let beatmap = OsuBeatmap::parse_bytes(osu_file_bytes)?;
```

Equivalent entry points are `OsuBeatmap::parse_reader`, `OsuBeatmap::parse_bufread`, and `OsuBeatmap::parse_file`. Analysis vectors use typed axes rather than string-keyed maps internally:

```rust,ignore
let mut skills = SkillVector::<EzMinaSkillAxis>::zeros();
skills.set(EzMinaSkillAxis::Stream, 3.25);
let stream = skills.get(EzMinaSkillAxis::Stream);
```

## Build and tests

```sh
cargo check --tests --manifest-path LAsRustSRCalculator/Cargo.toml
cargo test --manifest-path LAsRustSRCalculator/Cargo.toml
```

The crate is built as both an `rlib` and a `cdylib`. Interop uses byte slices and fixed-layout values; JSON is not used to transfer maps or analysis results.
