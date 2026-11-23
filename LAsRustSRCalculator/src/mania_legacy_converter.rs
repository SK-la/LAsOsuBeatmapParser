//! Seeded osu!standard-to-Mania legacy conversion.
//!
//! The random generator and pattern decisions follow Ez2Lazer's Mania legacy converters. This is a
//! separate path from [`crate::mania_converter::ManiaBeatmapConverter`], which handles native Mania
//! maps and preserves their columns.

use crate::beatmap::GameMode;
use crate::mania::ManiaNote;
use crate::mods::Mod;
use crate::normalization::{NormalizedBeatmap, NormalizedHitObject, NormalizedHitObjectKind};
use crate::slider_path::{SliderPath, SliderPathOptions};

const MAX_COLUMNS: usize = 18;
const MAX_DENSITY_NOTES: usize = 24;
const HIT_FINISH: i32 = 4;
const HIT_CLAP: i32 = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LegacyManiaConversionError {
    UnsupportedSourceMode(GameMode),
    InvalidColumnCount(usize),
    InvalidSliderPath(usize),
    InvalidTimingPoint(usize),
    InvalidTime(usize),
    UnknownConversionMod(String),
}

impl std::fmt::Display for LegacyManiaConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSourceMode(mode) => write!(
                f,
                "legacy Mania conversion requires an osu!standard source, got {mode:?}"
            ),
            Self::InvalidColumnCount(count) => write!(
                f,
                "Mania key count must be in 1..={MAX_COLUMNS}, got {count}"
            ),
            Self::InvalidSliderPath(index) => {
                write!(f, "standard slider {index} has no usable path distance")
            }
            Self::InvalidTimingPoint(index) => {
                write!(f, "standard slider {index} has no valid timing point")
            }
            Self::InvalidTime(index) => {
                write!(f, "standard hit object {index} has a non-finite time")
            }
            Self::UnknownConversionMod(acronym) => write!(
                f,
                "unknown conversion mod `{acronym}` may change Mania columns or patterns"
            ),
        }
    }
}

impl std::error::Error for LegacyManiaConversionError {}

#[derive(Clone, Debug, Default, PartialEq)]
struct Pattern {
    notes: Vec<ManiaNote>,
}

impl Pattern {
    fn contains(&self, column: usize) -> bool {
        self.notes.iter().any(|note| note.column == column)
    }

    fn column_count(&self) -> usize {
        self.notes
            .iter()
            .map(|note| note.column)
            .collect::<std::collections::HashSet<_>>()
            .len()
    }
}

#[derive(Clone, Copy, Debug)]
struct LegacyRandom {
    x: u32,
    y: u32,
    z: u32,
    w: u32,
}

impl LegacyRandom {
    fn new(seed: i32) -> Self {
        Self {
            x: seed as u32,
            y: 842_502_087,
            z: 3_579_807_591,
            w: 273_326_509,
        }
    }

    fn next_u32(&mut self) -> u32 {
        let t = self.x ^ self.x.wrapping_shl(11);
        self.x = self.y;
        self.y = self.z;
        self.z = self.w;
        self.w = self.w ^ (self.w >> 19) ^ t ^ (t >> 8);
        self.w
    }

    fn next(&mut self) -> u32 {
        self.next_u32() & 0x7fff_ffff
    }

    fn next_double(&mut self) -> f64 {
        self.next() as f64 / 2_147_483_648.0
    }

    fn next_column(&mut self, lower: usize, upper: usize) -> usize {
        lower + (self.next_double() * (upper - lower) as f64) as usize
    }
}

#[derive(Clone, Copy)]
struct Flags(u16);

impl Flags {
    const FORCE_STACK: u16 = 1;
    const FORCE_NOT_STACK: u16 = 2;
    const KEEP_SINGLE: u16 = 4;
    const LOW_PROBABILITY: u16 = 8;
    const GATHERED: u16 = 1 << 7;
    const MIRROR: u16 = 1 << 8;
    const REVERSE: u16 = 1 << 9;
    const CYCLE: u16 = 1 << 10;
    const STAIR: u16 = 1 << 11;
    const REVERSE_STAIR: u16 = 1 << 12;

    fn has(self, value: u16) -> bool {
        self.0 & value != 0
    }
    fn insert(&mut self, value: u16) {
        self.0 |= value;
    }
}

/// Converts an osu!standard beatmap to the playable Mania note sequence.
pub struct ManiaLegacyBeatmapConverter;

impl ManiaLegacyBeatmapConverter {
    pub fn convert(
        beatmap: &NormalizedBeatmap,
        mods: &[Mod],
    ) -> Result<(Vec<ManiaNote>, usize), LegacyManiaConversionError> {
        if matches!(beatmap.mode, GameMode::Mania | GameMode::Taiko) {
            return Err(LegacyManiaConversionError::UnsupportedSourceMode(
                beatmap.mode,
            ));
        }
        if let Some(Mod::Other(acronym)) = mods
            .iter()
            .find(|modifier| matches!(modifier, Mod::Other(_)))
        {
            return Err(LegacyManiaConversionError::UnknownConversionMod(
                acronym.clone(),
            ));
        }

        let mut columns = infer_column_count(beatmap);
        for modifier in mods {
            if let Mod::KeyCount { keys } = modifier {
                columns = usize::from(*keys).clamp(1, MAX_COLUMNS);
            }
        }
        if !(1..=MAX_COLUMNS).contains(&columns) {
            return Err(LegacyManiaConversionError::InvalidColumnCount(columns));
        }

        let mut state = ConversionState::new(beatmap, columns);
        for (index, object) in beatmap.hit_objects.iter().enumerate() {
            if !object.start_time.is_finite() || !object.end_time().is_finite() {
                return Err(LegacyManiaConversionError::InvalidTime(index));
            }
            state.convert_object(object, index)?;
        }

        state
            .notes
            .sort_by(|a, b| a.start_time_ms.total_cmp(&b.start_time_ms));
        Ok((state.notes, columns))
    }
}

fn infer_column_count(beatmap: &NormalizedBeatmap) -> usize {
    let difficulty = beatmap.difficulty;
    let rounded_od = difficulty.overall_difficulty.round_ties_even();
    let total = beatmap.hit_objects.len();
    if total > 0 {
        let special = beatmap
            .hit_objects
            .iter()
            .filter(|object| {
                matches!(
                    object.kind,
                    NormalizedHitObjectKind::Slider { .. }
                        | NormalizedHitObjectKind::Hold { .. }
                        | NormalizedHitObjectKind::Spinner { .. }
                )
            })
            .count();
        let ratio = special as f64 / total as f64;
        if ratio < 0.2 {
            return 7;
        }
        if ratio < 0.3 || difficulty.circle_size.round_ties_even() >= 5.0 {
            return if rounded_od > 5.0 { 7 } else { 6 };
        }
        if ratio > 0.6 {
            return if rounded_od > 4.0 { 5 } else { 4 };
        }
    }
    (rounded_od as i32 + 1).clamp(4, 7) as usize
}

struct ConversionState<'a> {
    map: &'a NormalizedBeatmap,
    columns: usize,
    random_start: usize,
    random: LegacyRandom,
    previous: Pattern,
    previous_circle_stair: u16,
    last_note_time: f64,
    last_position: [f64; 2],
    density_times: std::collections::VecDeque<f64>,
    density: f64,
    conversion_difficulty: f64,
    notes: Vec<ManiaNote>,
    rng_states: Vec<(f64, [u32; 4])>,
}

impl<'a> ConversionState<'a> {
    fn new(map: &'a NormalizedBeatmap, columns: usize) -> Self {
        let d = map.difficulty;
        let drain_circle = ((d.drain_rate as f32 + d.circle_size as f32).round_ties_even()) as i32;
        let seed = drain_circle
            .wrapping_mul(20)
            .wrapping_add(((d.overall_difficulty as f32) as f64 * 41.2) as i32)
            .wrapping_add((d.approach_rate as f32).round_ties_even() as i32);
        let first = map
            .hit_objects
            .first()
            .map_or(0.0, |object| object.start_time);
        let last = map
            .hit_objects
            .last()
            .map_or(0.0, |object| object.start_time);
        let break_time: f64 = map.breaks.iter().map(|period| period.duration()).sum();
        let mut drain_seconds = ((last - first - break_time) / 1000.0) as i32;
        if drain_seconds == 0 {
            drain_seconds = 10_000;
        }
        let conversion_difficulty = (((d.drain_rate + d.approach_rate.clamp(4.0, 7.0)) / 1.5
            + map.hit_objects.len() as f64 / drain_seconds as f64 * 9.0)
            / 38.0)
            * 5.0
            / 1.15;

        Self {
            map,
            columns,
            random_start: usize::from(columns == 8),
            random: LegacyRandom::new(seed),
            previous: Pattern::default(),
            previous_circle_stair: 1 << 11,
            last_note_time: 0.0,
            last_position: [0.0, 0.0],
            density_times: std::collections::VecDeque::new(),
            density: i32::MAX as f64,
            conversion_difficulty: conversion_difficulty.min(12.0),
            notes: Vec::with_capacity(map.hit_objects.len()),
            rng_states: Vec::with_capacity(map.hit_objects.len()),
        }
    }

    fn convert_object(
        &mut self,
        object: &NormalizedHitObject,
        index: usize,
    ) -> Result<(), LegacyManiaConversionError> {
        match &object.kind {
            NormalizedHitObjectKind::Circle => {
                self.compute_density(object.start_time);
                let pattern = self.circle_pattern(object);
                self.update_circle_stair(object, &pattern);
                self.record_note(object.start_time, object.position);
                self.notes.extend(pattern.notes.iter().copied());
                self.previous = pattern;
            }
            NormalizedHitObjectKind::Slider {
                raw_curve,
                path_control_points,
                repeat_count,
                pixel_length,
                edge_sounds,
            } => {
                let distance = SliderPath::from_legacy_curve(
                    raw_curve,
                    path_control_points,
                    *pixel_length,
                    self.map.version,
                    SliderPathOptions::default(),
                )
                .map_err(|_| LegacyManiaConversionError::InvalidSliderPath(index))?
                .distance();
                let start_time = object.start_time.round_ties_even() as i32;
                let beat = timing_beat_length_at(self.map, object.start_time)
                    .ok_or(LegacyManiaConversionError::InvalidTimingPoint(index))?;
                let sv = slider_velocity_at(self.map, object.start_time);
                let adjusted = beat * ((100.0 / sv).clamp(10.0, 10_000.0) / 100.0);
                let span_count = repeat_count.saturating_add(1).max(1);
                let end_time = (start_time as f64
                    + distance * adjusted * span_count as f64 * 0.01
                        / self.map.difficulty.slider_multiplier)
                    .floor() as i32;
                let segment = if span_count == 0 {
                    0
                } else {
                    (end_time - start_time) / span_count as i32
                };
                let original = self.slider_pattern(
                    object,
                    start_time,
                    end_time,
                    segment,
                    span_count as usize,
                    edge_sounds,
                );
                let patterns = if original.notes.len() == 1 {
                    vec![original]
                } else {
                    let (middle, end): (Vec<_>, Vec<_>) = original
                        .notes
                        .into_iter()
                        .partition(|note| note.end_time_ms.round_ties_even() as i32 != end_time);
                    vec![Pattern { notes: middle }, Pattern { notes: end }]
                };
                for pattern in patterns {
                    for note in &pattern.notes {
                        self.notes.push(*note);
                    }
                    self.previous = pattern;
                }
                for span in 0..=span_count {
                    let time = object.start_time + segment as f64 * span as f64;
                    self.record_note(time, object.position);
                    self.compute_density(time);
                }
            }
            NormalizedHitObjectKind::Spinner { end_time } => {
                let end = end_time.trunc() as i32;
                let duration = end as f64 - object.start_time;
                let hold = duration >= 100.0;
                let special_finish = self.columns == 8 && object.hitsound & HIT_FINISH != 0;
                let lower = usize::from(self.columns == 8);
                let column = if special_finish && duration < 1000.0 {
                    0
                } else {
                    let random_column = self.random_column(lower);
                    if self.previous.column_count() < self.columns {
                        let previous = self.previous.clone();
                        Self::find_available(
                            &mut self.random,
                            random_column,
                            lower,
                            self.columns,
                            |_, random| random.next_column(lower, self.columns),
                            |_| true,
                            &[&previous],
                        )
                    } else {
                        random_column
                    }
                };
                let note = ManiaNote {
                    start_time_ms: object.start_time,
                    end_time_ms: if hold { end as f64 } else { object.start_time },
                    column,
                };
                self.notes.push(note);
                self.record_note(end as f64, [256.0, 192.0]);
                self.compute_density(end as f64);
            }
            NormalizedHitObjectKind::Hold { end_time } => {
                let note = ManiaNote {
                    start_time_ms: object.start_time,
                    end_time_ms: *end_time,
                    column: self.column(object.position[0], false),
                };
                self.notes.push(note);
                self.record_note(*end_time, object.position);
                self.compute_density(*end_time);
            }
        }
        self.rng_states.push((
            object.start_time,
            [self.random.w, self.random.x, self.random.y, self.random.z],
        ));
        Ok(())
    }

    fn circle_pattern(&mut self, object: &NormalizedHitObject) -> Pattern {
        let delta = object.start_time - self.last_note_time;
        let dx = object.position[0] - self.last_position[0];
        let dy = object.position[1] - self.last_position[1];
        let separation = (dx * dx + dy * dy).sqrt();
        let beat = timing_beat_length_at(self.map, object.start_time).unwrap_or(1000.0);
        let kiai = kiai_at(self.map, object.start_time);
        let mut flags = Flags(0);
        if delta <= 80.0 {
            flags.insert(Flags::FORCE_NOT_STACK | Flags::KEEP_SINGLE);
        } else if delta <= 95.0 {
            flags.insert(Flags::FORCE_NOT_STACK | Flags::KEEP_SINGLE | self.previous_circle_stair);
        } else if delta <= 105.0 {
            flags.insert(Flags::FORCE_NOT_STACK | Flags::LOW_PROBABILITY);
        } else if delta <= 125.0 {
            flags.insert(Flags::FORCE_NOT_STACK);
        } else if delta <= 135.0 && separation < 20.0 {
            flags.insert(Flags::CYCLE | Flags::KEEP_SINGLE);
        } else if delta <= 150.0 && separation < 20.0 {
            flags.insert(Flags::FORCE_STACK | Flags::LOW_PROBABILITY);
        } else if separation < 20.0 && self.density >= beat / 2.5 {
            flags.insert(Flags::REVERSE | Flags::LOW_PROBABILITY);
        } else if self.density >= beat / 2.5 && !kiai {
            flags.insert(Flags::LOW_PROBABILITY);
        }

        if !flags.has(Flags::KEEP_SINGLE) {
            if object.hitsound & HIT_FINISH != 0 && self.columns != 8 {
                flags.insert(Flags::MIRROR);
            } else if object.hitsound & HIT_CLAP != 0 {
                flags.insert(Flags::GATHERED);
            }
        }

        let first_column = self.previous.notes.first().map_or(0, |note| note.column);
        if self.columns == 1 {
            return Pattern {
                notes: vec![self.note(object.start_time, object.start_time, 0)],
            };
        }
        if flags.has(Flags::REVERSE) && !self.previous.notes.is_empty() {
            return Pattern {
                notes: self
                    .previous
                    .notes
                    .iter()
                    .filter(|note| note.column >= self.random_start)
                    .map(|note| {
                        self.note(
                            object.start_time,
                            object.start_time,
                            self.random_start + self.columns - note.column - 1,
                        )
                    })
                    .collect(),
            };
        }
        if flags.has(Flags::CYCLE)
            && self.previous.notes.len() == 1
            && (self.columns != 8 || first_column != 0)
            && (self.columns % 2 == 0 || first_column != self.columns / 2)
        {
            return Pattern {
                notes: vec![self.note(
                    object.start_time,
                    object.start_time,
                    self.random_start + self.columns - first_column - 1,
                )],
            };
        }
        if flags.has(Flags::FORCE_STACK) && !self.previous.notes.is_empty() {
            return Pattern {
                notes: self
                    .previous
                    .notes
                    .iter()
                    .filter(|note| note.column >= self.random_start)
                    .map(|note| self.note(object.start_time, object.start_time, note.column))
                    .collect(),
            };
        }
        if self.previous.notes.len() == 1 {
            if flags.has(Flags::STAIR) {
                let col = if first_column + 1 == self.columns {
                    self.random_start
                } else {
                    first_column + 1
                };
                return Pattern {
                    notes: vec![self.note(object.start_time, object.start_time, col)],
                };
            }
            if flags.has(Flags::REVERSE_STAIR) {
                let col = if first_column == self.random_start {
                    self.columns - 1
                } else {
                    first_column - 1
                };
                return Pattern {
                    notes: vec![self.note(object.start_time, object.start_time, col)],
                };
            }
        }
        if flags.has(Flags::KEEP_SINGLE) {
            return self.circle_random_notes(object, 1, flags);
        }

        let d = self.conversion_difficulty;
        let (p2, p3) = if flags.has(Flags::MIRROR) {
            if d > 6.5 {
                (0.38, 0.12)
            } else if d > 4.0 {
                (0.17, 0.0)
            } else {
                (0.0, 0.0)
            }
        } else if d > 6.5 {
            if flags.has(Flags::LOW_PROBABILITY) {
                (0.78, 0.42)
            } else {
                (1.0, 0.62)
            }
        } else if d > 4.0 {
            if flags.has(Flags::LOW_PROBABILITY) {
                (0.35, 0.08)
            } else {
                (0.52, 0.15)
            }
        } else if d > 2.0 {
            if flags.has(Flags::LOW_PROBABILITY) {
                (0.18, 0.0)
            } else {
                (0.45, 0.0)
            }
        } else {
            (0.0, 0.0)
        };
        if flags.has(Flags::MIRROR) {
            return self.circle_mirrored(object, 0.12, p2, p3, flags);
        }
        let p2 = if object.hitsound & HIT_CLAP != 0 {
            1.0
        } else {
            p2
        };
        let count = self.random_count(p2, p3, 0.0, 0.0, 0.0, true, false);
        self.circle_random_notes(object, count, flags)
    }

    fn circle_random_notes(
        &mut self,
        object: &NormalizedHitObject,
        count: usize,
        flags: Flags,
    ) -> Pattern {
        let mut pattern = Pattern::default();
        let mut next = self.column(object.position[0], true);
        let mut safe_count = count;
        let allow_stacking = !flags.has(Flags::FORCE_NOT_STACK);
        if !allow_stacking {
            safe_count = safe_count.min(
                self.columns
                    .saturating_sub(self.random_start + self.previous.column_count()),
            );
        }
        for _ in 0..safe_count {
            next = Self::find_available(
                &mut self.random,
                next,
                self.random_start,
                self.columns,
                |last, random| {
                    if flags.has(Flags::GATHERED) {
                        if last + 1 == self.columns {
                            self.random_start
                        } else {
                            last + 1
                        }
                    } else {
                        random.next_column(self.random_start, self.columns)
                    }
                },
                |column| allow_stacking || !self.previous.contains(column),
                &[&pattern],
            );
            pattern
                .notes
                .push(self.note(object.start_time, object.start_time, next));
        }
        if self.random_start > 0
            && object.hitsound & (HIT_CLAP | HIT_FINISH) == (HIT_CLAP | HIT_FINISH)
        {
            pattern
                .notes
                .push(self.note(object.start_time, object.start_time, 0));
        }
        pattern
    }

    fn circle_mirrored(
        &mut self,
        object: &NormalizedHitObject,
        center_probability: f64,
        p2: f64,
        p3: f64,
        flags: Flags,
    ) -> Pattern {
        if flags.has(Flags::FORCE_NOT_STACK) {
            let force_p2 = 0.5 + p2 / 2.0;
            let force_p3 = p2;
            let force_p4 = (p2 + p3) / 2.0;
            let force_p5 = p3;
            let force_p2 = if object.hitsound & HIT_CLAP != 0 {
                1.0
            } else {
                force_p2
            };
            let count = self.random_count(force_p2, force_p3, force_p4, force_p5, 0.0, true, false);
            return self.circle_random_notes(object, count, flags);
        }
        let mut center_probability = center_probability;
        let (mut p2, mut p3) = (p2, p3);
        match self.columns {
            2 => {
                center_probability = 0.0;
                p2 = 0.0;
                p3 = 0.0;
            }
            3 => {
                center_probability = center_probability.min(0.03);
                p2 = 0.0;
                p3 = 0.0;
            }
            4 => {
                center_probability = 0.0;
                p2 = 1.0 - ((1.0 - p2) * 2.0).max(0.8);
                p3 = 0.0;
            }
            5 => {
                center_probability = center_probability.min(0.03);
                p3 = 0.0;
            }
            6 => {
                center_probability = 0.0;
                p2 = 1.0 - ((1.0 - p2) * 2.0).max(0.5);
                p3 = 1.0 - ((1.0 - p3) * 2.0).max(0.85);
            }
            _ => {}
        }
        p2 = p2.clamp(0.0, 1.0);
        p3 = p3.clamp(0.0, 1.0);
        let center = self.random.next_double();
        let count = self.random_count(p2, p3, 0.0, 0.0, 0.0, true, false);
        let add_center = self.columns % 2 != 0 && count != 3 && center > 1.0 - center_probability;
        let limit = (self.columns / 2).max(1);
        let mut pattern = Pattern::default();
        let mut next = self.random_column(self.random_start.min(limit.saturating_sub(1)));
        for _ in 0..count {
            next = Self::find_available(
                &mut self.random,
                next,
                self.random_start,
                limit,
                |_, random| random.next_column(self.random_start, limit),
                |_| true,
                &[&pattern],
            );
            pattern
                .notes
                .push(self.note(object.start_time, object.start_time, next));
            pattern.notes.push(self.note(
                object.start_time,
                object.start_time,
                self.random_start + self.columns - next - 1,
            ));
        }
        if add_center {
            pattern
                .notes
                .push(self.note(object.start_time, object.start_time, self.columns / 2));
        }
        if self.random_start > 0
            && object.hitsound & (HIT_CLAP | HIT_FINISH) == (HIT_CLAP | HIT_FINISH)
        {
            pattern
                .notes
                .push(self.note(object.start_time, object.start_time, 0));
        }
        pattern
    }

    fn random_count(
        &mut self,
        p2: f64,
        p3: f64,
        p4: f64,
        p5: f64,
        p6: f64,
        clamp_columns: bool,
        _unused: bool,
    ) -> usize {
        let (mut p2, mut p3, mut p4, mut p5, mut p6) = (p2, p3, p4, p5, p6);
        if clamp_columns {
            match self.columns {
                2 => {
                    p2 = 0.0;
                    p3 = 0.0;
                    p4 = 0.0;
                    p5 = 0.0;
                    p6 = 0.0;
                }
                3 => {
                    p2 = p2.min(0.1);
                    p3 = 0.0;
                    p4 = 0.0;
                    p5 = 0.0;
                    p6 = 0.0;
                }
                4 => {
                    p2 = p2.min(0.23);
                    p3 = p3.min(0.04);
                    p4 = 0.0;
                    p5 = 0.0;
                    p6 = 0.0;
                }
                5 => {
                    p3 = p3.min(0.15);
                    p4 = p4.min(0.03);
                    p5 = 0.0;
                    p6 = 0.0;
                }
                _ => {}
            }
        }
        let value = self.random.next_double();
        if value >= 1.0 - p6 {
            6
        } else if value >= 1.0 - p5 {
            5
        } else if value >= 1.0 - p4 {
            4
        } else if value >= 1.0 - p3 {
            3
        } else if value >= 1.0 - p2 {
            2
        } else {
            1
        }
    }

    fn slider_pattern(
        &mut self,
        object: &NormalizedHitObject,
        start: i32,
        end: i32,
        segment: i32,
        spans: usize,
        edge_sounds: &[i32],
    ) -> Pattern {
        let kiai = kiai_at(self.map, object.start_time);
        let mut flags = if kiai {
            Flags(0)
        } else {
            Flags(Flags::LOW_PROBABILITY)
        };
        let duration = end.saturating_sub(start);
        if self.columns == 1 {
            return Pattern {
                notes: vec![self.note(start as f64, end as f64, 0)],
            };
        }
        if spans > 1 {
            if segment <= 90 {
                return self.slider_holds(object, start, end, 1);
            }
            if segment <= 120 {
                flags.insert(Flags::FORCE_NOT_STACK);
                return self.slider_taps(object, start, segment, spans + 1, flags);
            }
            if segment <= 160 {
                return self.slider_stair(object, start, segment, spans);
            }
            if segment <= 200 && self.conversion_difficulty > 3.0 {
                return self.slider_multiple(object, start, segment, spans);
            }
            if duration >= 4000 {
                let count = self.random_count(0.23, 0.0, 0.0, 0.0, 0.0, true, false);
                return self.slider_holds(object, start, end, count);
            }
            if segment > 400 && spans < self.columns.saturating_sub(1 + self.random_start) {
                return self.slider_tiled(object, start, end, segment, spans, flags);
            }
            return self.slider_hold_and_notes(
                object,
                start,
                end,
                segment,
                spans,
                flags,
                edge_sounds,
            );
        }
        if segment <= 110 {
            if self.previous.column_count() < self.columns {
                flags.insert(Flags::FORCE_NOT_STACK);
            }
            return self.slider_taps(
                object,
                start,
                segment,
                if segment < 80 { 1 } else { 2 },
                flags,
            );
        }
        let d = self.conversion_difficulty;
        let (p2, p3, p4) = if d > 6.5 {
            if flags.has(Flags::LOW_PROBABILITY) {
                (0.78, 0.3, 0.0)
            } else {
                (0.85, 0.36, 0.03)
            }
        } else if d > 4.0 {
            if flags.has(Flags::LOW_PROBABILITY) {
                (0.43, 0.08, 0.0)
            } else {
                (0.56, 0.18, 0.0)
            }
        } else if d > 2.5 {
            if flags.has(Flags::LOW_PROBABILITY) {
                (0.3, 0.0, 0.0)
            } else {
                (0.37, 0.08, 0.0)
            }
        } else if flags.has(Flags::LOW_PROBABILITY) {
            (0.17, 0.0, 0.0)
        } else {
            (0.27, 0.0, 0.0)
        };
        let can_force_double = !flags.has(Flags::LOW_PROBABILITY)
            && (object.hitsound & (HIT_CLAP | HIT_FINISH) != 0
                || edge_sounds
                    .first()
                    .is_some_and(|sound| sound & (HIT_CLAP | HIT_FINISH) != 0));
        let p2 = if can_force_double { 1.0 } else { p2 };
        let count = self.random_count(p2, p3, p4, 0.0, 0.0, true, false);
        self.slider_holds(object, start, end, count)
    }

    fn slider_holds(
        &mut self,
        object: &NormalizedHitObject,
        start: i32,
        end: i32,
        count: usize,
    ) -> Pattern {
        let usable = self
            .columns
            .saturating_sub(self.random_start + self.previous.column_count());
        let mut pattern = Pattern::default();
        let mut next = self.random_column(self.random_start);
        for _ in 0..count.min(usable) {
            next = Self::find_available(
                &mut self.random,
                next,
                self.random_start,
                self.columns,
                |_, r| r.next_column(self.random_start, self.columns),
                |_| true,
                &[&pattern, &self.previous],
            );
            pattern
                .notes
                .push(self.note(start as f64, end as f64, next));
        }
        for _ in 0..count.saturating_sub(usable) {
            next = Self::find_available(
                &mut self.random,
                next,
                self.random_start,
                self.columns,
                |_, r| r.next_column(self.random_start, self.columns),
                |_| true,
                &[&pattern],
            );
            pattern
                .notes
                .push(self.note(start as f64, end as f64, next));
        }
        let _ = object;
        pattern
    }

    fn slider_taps(
        &mut self,
        object: &NormalizedHitObject,
        mut start: i32,
        segment: i32,
        count: usize,
        flags: Flags,
    ) -> Pattern {
        let mut pattern = Pattern::default();
        let mut next = self.column(object.position[0], true);
        if flags.has(Flags::FORCE_NOT_STACK) && self.previous.column_count() < self.columns {
            next = Self::find_available(
                &mut self.random,
                next,
                self.random_start,
                self.columns,
                |_, r| r.next_column(self.random_start, self.columns),
                |_| true,
                &[&self.previous],
            );
        }
        let mut last = next;
        for _ in 0..count {
            pattern
                .notes
                .push(self.note(start as f64, start as f64, next));
            next = Self::find_available(
                &mut self.random,
                next,
                self.random_start,
                self.columns,
                |_, r| r.next_column(self.random_start, self.columns),
                |c| c != last,
                &[],
            );
            last = next;
            start = start.saturating_add(segment);
        }
        pattern
    }

    fn slider_stair(
        &mut self,
        object: &NormalizedHitObject,
        mut start: i32,
        segment: i32,
        spans: usize,
    ) -> Pattern {
        let mut pattern = Pattern::default();
        let mut col = self.column(object.position[0], true);
        let mut up = self.random.next_double() > 0.5;
        for _ in 0..=spans {
            pattern
                .notes
                .push(self.note(start as f64, start as f64, col));
            start = start.saturating_add(segment);
            if up {
                if col >= self.columns - 1 {
                    up = false;
                    col -= 1
                } else {
                    col += 1
                }
            } else if col <= self.random_start {
                up = true;
                col += 1
            } else {
                col -= 1
            };
        }
        pattern
    }

    fn slider_multiple(
        &mut self,
        object: &NormalizedHitObject,
        mut start: i32,
        segment: i32,
        spans: usize,
    ) -> Pattern {
        let legacy = (4..=8).contains(&self.columns);
        let interval = self
            .random
            .next_column(1, self.columns - usize::from(legacy));
        let mut col = self.column(object.position[0], true);
        let mut out = Pattern::default();
        for _ in 0..=spans {
            out.notes.push(self.note(start as f64, start as f64, col));
            col += interval;
            if col >= self.columns - self.random_start {
                col = col - self.columns - self.random_start + usize::from(legacy)
            };
            col += self.random_start;
            if self.columns > 2 {
                out.notes.push(self.note(start as f64, start as f64, col));
            }
            col = self.random_column(self.random_start);
            start = start.saturating_add(segment);
        }
        out
    }

    fn slider_tiled(
        &mut self,
        object: &NormalizedHitObject,
        start: i32,
        end: i32,
        segment: i32,
        spans: usize,
        flags: Flags,
    ) -> Pattern {
        let mut out = Pattern::default();
        let mut col = self.column(object.position[0], true);
        let repeat = spans.min(self.columns);
        if flags.has(Flags::FORCE_NOT_STACK) && self.previous.column_count() < self.columns {
            col = Self::find_available(
                &mut self.random,
                col,
                self.random_start,
                self.columns,
                |_, r| r.next_column(self.random_start, self.columns),
                |_| true,
                &[&self.previous],
            );
        }
        for i in 0..repeat {
            col = Self::find_available(
                &mut self.random,
                col,
                self.random_start,
                self.columns,
                |_, r| r.next_column(self.random_start, self.columns),
                |_| true,
                &[&out],
            );
            out.notes
                .push(self.note((start + i as i32 * segment) as f64, end as f64, col));
        }
        out
    }

    fn slider_hold_and_notes(
        &mut self,
        object: &NormalizedHitObject,
        start: i32,
        end: i32,
        segment: i32,
        spans: usize,
        flags: Flags,
        edge_sounds: &[i32],
    ) -> Pattern {
        let mut out = Pattern::default();
        let mut hold = self.column(object.position[0], true);
        if flags.has(Flags::FORCE_NOT_STACK) && self.previous.column_count() < self.columns {
            hold = Self::find_available(
                &mut self.random,
                hold,
                self.random_start,
                self.columns,
                |_, r| r.next_column(self.random_start, self.columns),
                |_| true,
                &[&self.previous],
            );
        }
        out.notes.push(self.note(start as f64, end as f64, hold));
        let d = self.conversion_difficulty;
        let extras = if d > 6.5 {
            self.random_count(0.63, 0.0, 0.0, 0.0, 0.0, false, false)
        } else if d > 4.0 {
            self.random_count(
                if self.columns < 6 { 0.12 } else { 0.45 },
                0.0,
                0.0,
                0.0,
                0.0,
                false,
                false,
            )
        } else if d > 2.5 {
            self.random_count(
                if self.columns < 6 { 0.0 } else { 0.24 },
                0.0,
                0.0,
                0.0,
                0.0,
                false,
                false,
            )
        } else {
            0
        }
        .min(self.columns.saturating_sub(1));
        let mut col = self.random_column(self.random_start);
        for row in 0..=spans {
            let sound = edge_sounds.get(row).copied().unwrap_or(object.hitsound);
            if row == 0 && sound & (HIT_FINISH | HIT_CLAP | 2) == 0 {
                continue;
            }
            let time = (start + row as i32 * segment) as f64;
            let mut row_pat = Pattern::default();
            for _ in 0..extras {
                col = Self::find_available(
                    &mut self.random,
                    col,
                    self.random_start,
                    self.columns,
                    |_, r| r.next_column(self.random_start, self.columns),
                    |c| c != hold,
                    &[&row_pat],
                );
                row_pat.notes.push(self.note(time, time, col));
            }
            out.notes.extend(row_pat.notes);
        }
        out
    }

    fn find_available(
        random: &mut LegacyRandom,
        initial: usize,
        lower: usize,
        upper: usize,
        mut next: impl FnMut(usize, &mut LegacyRandom) -> usize,
        valid: impl Fn(usize) -> bool,
        patterns: &[&Pattern],
    ) -> usize {
        let is_valid =
            |column: usize| valid(column) && patterns.iter().all(|p| !p.contains(column));
        if is_valid(initial) {
            return initial;
        }
        if !(lower..upper).any(is_valid) {
            return initial.min(upper.saturating_sub(1));
        }
        let mut column = initial;
        let max = upper.saturating_sub(lower).saturating_mul(10_000).max(1);
        for _ in 0..max {
            column = next(column, random).clamp(lower, upper - 1);
            if is_valid(column) {
                return column;
            }
        }
        (lower..upper)
            .find(|&column| is_valid(column))
            .unwrap_or(lower)
    }

    fn random_column(&mut self, lower: usize) -> usize {
        self.random.next_column(lower, self.columns)
    }
    fn column(&self, x: f64, special: bool) -> usize {
        if special && self.columns == 8 {
            ((x as f32 / (512.0_f32 / 7.0)).floor() as i32).clamp(0, 6) as usize + 1
        } else {
            ((x as f32 / (512.0_f32 / self.columns as f32)).floor() as i32)
                .clamp(0, self.columns as i32 - 1) as usize
        }
    }
    fn note(&self, start: f64, end: f64, column: usize) -> ManiaNote {
        ManiaNote {
            start_time_ms: start,
            end_time_ms: end,
            column,
        }
    }
    fn record_note(&mut self, time: f64, position: [f64; 2]) {
        self.last_note_time = time;
        self.last_position = position;
    }
    fn compute_density(&mut self, time: f64) {
        self.density_times.push_back(time);
        if self.density_times.len() > MAX_DENSITY_NOTES {
            self.density_times.pop_front();
        }
        if self.density_times.len() >= 2 {
            self.density = (self.density_times.back().unwrap()
                - self.density_times.front().unwrap())
                / self.density_times.len() as f64;
        }
    }
    fn update_circle_stair(&mut self, object: &NormalizedHitObject, pattern: &Pattern) {
        let delta = object.start_time - self.last_note_time;
        if !(80.0 < delta && delta <= 95.0) {
            return;
        }
        if self.previous_circle_stair == Flags::STAIR
            && pattern
                .notes
                .iter()
                .any(|note| note.column == self.columns - 1)
        {
            self.previous_circle_stair = Flags::REVERSE_STAIR;
        } else if self.previous_circle_stair == Flags::REVERSE_STAIR
            && pattern
                .notes
                .iter()
                .any(|note| note.column == self.random_start)
        {
            self.previous_circle_stair = Flags::STAIR;
        }
    }
}

fn timing_beat_length_at(beatmap: &NormalizedBeatmap, time: f64) -> Option<f64> {
    beatmap
        .control_points
        .timing
        .iter()
        .rev()
        .find(|p| p.time <= time)
        .or_else(|| beatmap.control_points.timing.first())
        .map(|p| p.beat_length)
        .filter(|v| v.is_finite() && *v > 0.0)
}

fn slider_velocity_at(beatmap: &NormalizedBeatmap, time: f64) -> f64 {
    beatmap
        .control_points
        .difficulty
        .iter()
        .rev()
        .find(|p| p.time <= time)
        .or_else(|| beatmap.control_points.difficulty.first())
        .map(|p| p.speed_multiplier)
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(1.0)
}

fn kiai_at(beatmap: &NormalizedBeatmap, time: f64) -> bool {
    beatmap
        .control_points
        .effect
        .iter()
        .rev()
        .find(|p| p.time <= time)
        .or_else(|| beatmap.control_points.effect.first())
        .is_some_and(|p| p.kiai)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beatmap::OsuBeatmap;
    use crate::normalization::normalize;

    #[test]
    fn standard_conversion_uses_seeded_pattern_generation() {
        let source = include_bytes!("../tests/fixtures/mania-standard-basic.osu");
        let map = normalize(&OsuBeatmap::parse_bytes(source).unwrap()).unwrap();
        let (mut first, keys) = ManiaLegacyBeatmapConverter::convert(&map, &[]).unwrap();
        let (second, _) = ManiaLegacyBeatmapConverter::convert(&map, &[]).unwrap();
        assert_eq!(keys, 5);
        assert_eq!(first, second);
        assert!(first.iter().all(|note| note.column < keys));
        first.sort_by(|a, b| a.start_time_ms.total_cmp(&b.start_time_ms));
        let actual = first
            .iter()
            .map(|note| {
                format!(
                    "{:.0},{:.0},{}",
                    note.start_time_ms, note.end_time_ms, note.column
                )
            })
            .collect::<Vec<_>>();
        let expected = include_str!("../tests/fixtures/mania-standard-basic.golden.csv")
            .lines()
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn conversion_mod_overrides_inferred_column_count() {
        let source=b"osu file format v14\n[General]\nMode:0\n[Difficulty]\nCircleSize:4\nOverallDifficulty:5\n[HitObjects]\n64,192,500,1,0,0:0:0:0:\n";
        let map = normalize(&OsuBeatmap::parse_bytes(source).unwrap()).unwrap();
        let (_, keys) =
            ManiaLegacyBeatmapConverter::convert(&map, &[Mod::KeyCount { keys: 6 }]).unwrap();
        assert_eq!(keys, 6);
    }

    #[test]
    fn legacy_random_states_match_reference_progress() {
        let source = include_bytes!("../tests/fixtures/mania-standard-basic.osu");
        let map = normalize(&OsuBeatmap::parse_bytes(source).unwrap()).unwrap();
        let mut state = ConversionState::new(&map, 5);
        for (index, object) in map.hit_objects.iter().enumerate() {
            state.convert_object(object, index).unwrap();
        }
        let actual = state
            .rng_states
            .iter()
            .map(|(time, random)| {
                format!(
                    "{time:.0},{},{},{},{}",
                    random[1], random[2], random[3], random[0]
                )
            })
            .collect::<Vec<_>>();
        let expected = include_str!("../tests/fixtures/mania-standard-basic-rng.golden.csv")
            .lines()
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
