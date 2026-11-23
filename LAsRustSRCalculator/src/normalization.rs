//! Legacy `.osu` input normalization matching the analysis-relevant behavior of Ez2Lazer's
//! `LegacyBeatmapDecoder` and `ConvertHitObjectParser`.
//!
//! This layer preserves raw slider curve data for ruleset converters. It does not convert sliders into
//! mode-specific objects or expand their paths into runtime geometry. The raw model currently reduces a slider
//! path to one curve tag and coordinate list, so lazer's multi-segment tags, B-spline degree suffixes, duplicate
//! anchor segmentation, and converter-specific path shaping are not reconstructed here.
//!
//! `normalize()` applies first-object and post-spinner combo rules in source order, then stably sorts by start time,
//! matching `ConvertHitObjectParser` and `LegacyBeatmapDecoder`. Raw hit objects retain their source-order index.

use crate::beatmap::{
    BreakPeriod, ControlPoints, GameMode, HitObject as RawHitObject,
    HitObjectKind as RawHitObjectKind, HitSample, OsuBeatmap, SliderCurve, TimingPoint,
};

pub const LEGACY_EARLY_VERSION_OFFSET: f64 = 24.0;
pub const FIRST_FLOAT_COORDINATE_VERSION: u32 = 128;
pub const MAX_MANIA_KEY_COUNT: f64 = 18.0;
pub const MAX_LEGACY_SLIDER_SPAN_COUNT: u32 = 9000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NormalizeOptions {
    /// Ez2Lazer applies legacy offsets by default. Disable only for parity tests which explicitly request it.
    pub apply_offsets: bool,
}

impl Default for NormalizeOptions {
    fn default() -> Self {
        Self {
            apply_offsets: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizationError {
    pub object_index: usize,
    pub message: String,
}

impl std::fmt::Display for NormalizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "hit object {}: {}", self.object_index, self.message)
    }
}

impl std::error::Error for NormalizationError {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalizedDifficulty {
    pub drain_rate: f64,
    pub circle_size: f64,
    pub overall_difficulty: f64,
    pub approach_rate: f64,
    pub slider_multiplier: f64,
    pub slider_tick_rate: f64,
}

impl NormalizedDifficulty {
    fn from_raw(raw: &crate::beatmap::DifficultySettings, mode: GameMode) -> Self {
        let overall_difficulty = raw.overall_difficulty.unwrap_or(5.0).clamp(0.0, 10.0);
        let approach_rate = raw
            .approach_rate
            .unwrap_or_else(|| raw.overall_difficulty.unwrap_or(5.0))
            .clamp(0.0, 10.0);

        Self {
            drain_rate: raw.hp_drain_rate.unwrap_or(5.0).clamp(0.0, 10.0),
            circle_size: raw.circle_size.unwrap_or(5.0).clamp(
                if mode == GameMode::Mania { 1.0 } else { 0.0 },
                if mode == GameMode::Mania {
                    MAX_MANIA_KEY_COUNT
                } else {
                    10.0
                },
            ),
            overall_difficulty,
            approach_rate,
            slider_multiplier: raw.slider_multiplier.unwrap_or(1.4).clamp(0.4, 3.6),
            slider_tick_rate: raw.slider_tick_rate.unwrap_or(1.0).clamp(0.5, 8.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedBeatmap {
    pub version: u32,
    pub mode: GameMode,
    pub difficulty: NormalizedDifficulty,
    /// Preview time is offset like other file times, except the sentinel `-1`.
    pub preview_time: Option<i32>,
    pub timing_points: Vec<TimingPoint>,
    pub control_points: ControlPoints,
    pub breaks: Vec<BreakPeriod>,
    pub hit_objects: Vec<NormalizedHitObject>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedHitObject {
    pub position: [f64; 2],
    pub start_time: f64,
    /// Legacy type bits with new-combo and combo-offset bits removed, like `LegacyType` in lazer.
    pub legacy_type_flags: u32,
    pub hitsound: i32,
    pub new_combo: bool,
    pub combo_offset: u8,
    pub column: Option<usize>,
    pub kind: NormalizedHitObjectKind,
    pub hit_sample: Option<HitSample>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NormalizedHitObjectKind {
    Circle,
    Slider {
        /// Original absolute path points as parsed from the osu file.
        raw_curve: SliderCurve,
        /// Legacy-compatible path points relative to the normalized slider head.
        /// Path segmentation and duplicate-anchor rules are deferred to ruleset converters.
        path_control_points: Vec<[f64; 2]>,
        /// Normalized repeat count; the osu file stores span count (repeat count + 1).
        repeat_count: u32,
        pixel_length: Option<f64>,
    },
    Spinner {
        end_time: f64,
    },
    Hold {
        end_time: f64,
    },
}

impl NormalizedHitObject {
    pub fn end_time(&self) -> f64 {
        match self.kind {
            NormalizedHitObjectKind::Spinner { end_time }
            | NormalizedHitObjectKind::Hold { end_time } => end_time,
            _ => self.start_time,
        }
    }
}

pub fn normalize(raw: &OsuBeatmap) -> Result<NormalizedBeatmap, NormalizationError> {
    normalize_with_options(raw, NormalizeOptions::default())
}

pub fn normalize_with_options(
    raw: &OsuBeatmap,
    options: NormalizeOptions,
) -> Result<NormalizedBeatmap, NormalizationError> {
    let offset = if options.apply_offsets && raw.version < 5 {
        LEGACY_EARLY_VERSION_OFFSET
    } else {
        0.0
    };

    let difficulty = NormalizedDifficulty::from_raw(&raw.difficulty, raw.mode);
    let mut timing_points = raw.timing_points.clone();
    for point in &mut timing_points {
        point.time += offset;
    }

    let mut control_points = raw.control_points.clone();
    offset_control_points(&mut control_points, offset);

    let mut breaks: Vec<BreakPeriod> = raw
        .breaks
        .iter()
        .map(|period| {
            let start_time = period.start_time + offset;
            let end_time = (period.end_time + offset).max(start_time);
            BreakPeriod {
                start_time,
                end_time,
            }
        })
        .collect();
    breaks.sort_by(|a, b| a.start_time.total_cmp(&b.start_time));

    // Combo flags are assigned during legacy parsing before the decoder's stable start-time sort. The raw parser
    // retains source order so this pass can reproduce that behavior for out-of-order files.
    let mut source_objects: Vec<(usize, &RawHitObject)> =
        raw.hit_objects.iter().enumerate().collect();
    source_objects.sort_by_key(|(vector_index, object)| (object.source_order, *vector_index));

    let mut was_first_object = true;
    let mut previous_was_spinner = false;
    let mut hit_objects = Vec::with_capacity(raw.hit_objects.len());
    for (index, (_, object)) in source_objects.into_iter().enumerate() {
        let normalized = normalize_object(
            object,
            index,
            raw.version,
            offset,
            was_first_object,
            previous_was_spinner,
        )?;
        previous_was_spinner = matches!(object.kind, RawHitObjectKind::Spinner { .. });
        was_first_object = false;
        hit_objects.push(normalized);
    }

    // `OrderBy(StartTime)` in the decoder is stable; Rust's slice `sort_by` is stable as well.
    hit_objects.sort_by(|a, b| a.start_time.total_cmp(&b.start_time));
    apply_break_combos(&mut hit_objects, &breaks);

    let preview_time = raw.general.preview_time.map(|time| {
        if time == -1 {
            time
        } else {
            time.wrapping_add(offset as i32)
        }
    });

    Ok(NormalizedBeatmap {
        version: raw.version,
        mode: raw.mode,
        difficulty,
        preview_time,
        timing_points,
        control_points,
        breaks,
        hit_objects,
    })
}

fn normalize_object(
    raw: &RawHitObject,
    object_index: usize,
    version: u32,
    offset: f64,
    first_object: bool,
    previous_was_spinner: bool,
) -> Result<NormalizedHitObject, NormalizationError> {
    let x = legacy_position_component(raw.position[0], version);
    let y = legacy_position_component(raw.position[1], version);
    let position = match raw.kind {
        RawHitObjectKind::Spinner { .. } => [256.0, 192.0],
        _ => [x, y],
    };
    let start_time = raw.start_time + offset;
    let raw_new_combo = raw.type_flags & 4 != 0;
    let raw_combo_offset = ((raw.type_flags & 0x70) >> 4) as u8;
    let combo_object = matches!(
        raw.kind,
        RawHitObjectKind::Circle | RawHitObjectKind::Slider { .. }
    );
    let new_combo = match raw.kind {
        RawHitObjectKind::Circle | RawHitObjectKind::Slider { .. } => {
            first_object || previous_was_spinner || raw_new_combo
        }
        RawHitObjectKind::Spinner { .. } => raw_new_combo,
        RawHitObjectKind::Hold { .. } => false,
    };
    let combo_offset = if combo_object && raw_new_combo {
        raw_combo_offset
    } else {
        0
    };
    let legacy_type_flags = raw.type_flags & !(0x70 | 4);

    let kind = match &raw.kind {
        RawHitObjectKind::Circle => NormalizedHitObjectKind::Circle,
        RawHitObjectKind::Slider {
            curve,
            repeats,
            pixel_length,
            ..
        } => {
            if *repeats > MAX_LEGACY_SLIDER_SPAN_COUNT {
                return Err(NormalizationError {
                    object_index,
                    message: format!(
                        "slider span count {repeats} exceeds {MAX_LEGACY_SLIDER_SPAN_COUNT}"
                    ),
                });
            }
            let path_control_points = curve
                .points
                .iter()
                .map(|point| {
                    let point_x = legacy_path_component(point[0], version) - position[0];
                    let point_y = legacy_path_component(point[1], version) - position[1];
                    [point_x, point_y]
                })
                .collect();
            NormalizedHitObjectKind::Slider {
                raw_curve: curve.clone(),
                path_control_points,
                repeat_count: repeats.saturating_sub(1),
                pixel_length: (*pixel_length > 0.0).then_some(*pixel_length),
            }
        }
        RawHitObjectKind::Spinner { end_time } => {
            let duration = (end_time + offset - start_time).max(0.0);
            NormalizedHitObjectKind::Spinner {
                end_time: start_time + duration,
            }
        }
        RawHitObjectKind::Hold { end_time } => NormalizedHitObjectKind::Hold {
            end_time: (*end_time + offset).max(start_time),
        },
    };

    Ok(NormalizedHitObject {
        position,
        start_time,
        legacy_type_flags,
        hitsound: raw.hitsound,
        new_combo,
        combo_offset,
        column: raw.column,
        kind,
        hit_sample: raw.hit_sample.clone(),
    })
}

fn legacy_position_component(value: f64, version: u32) -> f64 {
    let parsed_float = value_as_legacy_float(value).clamp(0.0, 512.0);
    if version < FIRST_FLOAT_COORDINATE_VERSION {
        parsed_float.trunc()
    } else {
        parsed_float
    }
}

fn legacy_path_component(value: f64, version: u32) -> f64 {
    let parsed_float = value_as_legacy_float(value);
    if version < FIRST_FLOAT_COORDINATE_VERSION {
        parsed_float.trunc()
    } else {
        parsed_float
    }
}

fn value_as_legacy_float(value: f64) -> f64 {
    (value as f32).clamp(-f32::MAX, f32::MAX) as f64
}

fn offset_control_points(points: &mut ControlPoints, offset: f64) {
    for point in &mut points.timing {
        point.time += offset;
    }
    for point in &mut points.difficulty {
        point.time += offset;
    }
    for point in &mut points.sample {
        point.time += offset;
    }
    for point in &mut points.effect {
        point.time += offset;
    }
}

fn apply_break_combos(hit_objects: &mut [NormalizedHitObject], breaks: &[BreakPeriod]) {
    let mut current_break = 0;
    let mut force_new_combo = false;
    for object in hit_objects {
        while current_break < breaks.len() && breaks[current_break].end_time < object.start_time {
            force_new_combo = true;
            current_break += 1;
        }
        object.new_combo |= force_new_combo;
        force_new_combo = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beatmap::{
        BeatmapEvent, DifficultySettings, GeneralSettings, HitObject, HitObjectKind, Metadata,
        SliderCurveType, TimingControlPoint, TimingPoint,
    };

    fn base_map(version: u32, mode: GameMode) -> OsuBeatmap {
        OsuBeatmap {
            version,
            mode,
            general: GeneralSettings {
                preview_time: Some(100),
                mode,
                ..GeneralSettings::default()
            },
            metadata: Metadata::default(),
            difficulty: DifficultySettings::default(),
            timing_points: Vec::new(),
            control_points: ControlPoints::default(),
            breaks: Vec::new(),
            events: Vec::<BeatmapEvent>::new(),
            hit_objects: Vec::new(),
        }
    }

    fn circle(x: f64, y: f64, time: f64, flags: u32) -> HitObject {
        HitObject {
            source_order: 0,
            position: [x, y],
            start_time: time,
            type_flags: flags,
            hitsound: 0,
            new_combo: flags & 4 != 0,
            combo_skip: ((flags & 0x70) >> 4) as u8,
            column: None,
            kind: HitObjectKind::Circle,
            hit_sample: None,
        }
    }

    fn spinner(x: f64, y: f64, start: f64, end: f64, flags: u32) -> HitObject {
        HitObject {
            source_order: 0,
            position: [x, y],
            start_time: start,
            type_flags: flags,
            hitsound: 0,
            new_combo: flags & 4 != 0,
            combo_skip: ((flags & 0x70) >> 4) as u8,
            column: None,
            kind: HitObjectKind::Spinner { end_time: end },
            hit_sample: None,
        }
    }

    fn slider(x: f64, y: f64, time: f64, flags: u32, spans: u32, length: f64) -> HitObject {
        HitObject {
            source_order: 0,
            position: [x, y],
            start_time: time,
            type_flags: flags,
            hitsound: 0,
            new_combo: flags & 4 != 0,
            combo_skip: ((flags & 0x70) >> 4) as u8,
            column: None,
            kind: HitObjectKind::Slider {
                curve: SliderCurve {
                    curve_type: SliderCurveType::Bezier,
                    points: vec![[x + 10.9, y - 20.9]],
                },
                repeats: spans,
                pixel_length: length,
                edge_sounds: vec![],
                edge_sets: vec![],
            },
            hit_sample: None,
        }
    }

    #[test]
    fn matches_legacy_difficulty_defaults_inheritance_and_clamps() {
        let mut raw = base_map(14, GameMode::Standard);
        let normalized = normalize(&raw).unwrap();
        assert_eq!(
            normalized.difficulty,
            NormalizedDifficulty {
                drain_rate: 5.0,
                circle_size: 5.0,
                overall_difficulty: 5.0,
                approach_rate: 5.0,
                slider_multiplier: 1.4,
                slider_tick_rate: 1.0,
            }
        );

        raw.difficulty = DifficultySettings {
            hp_drain_rate: Some(-4.0),
            circle_size: Some(20.0),
            overall_difficulty: Some(12.0),
            approach_rate: None,
            slider_multiplier: Some(8.0),
            slider_tick_rate: Some(0.1),
        };
        let normalized = normalize(&raw).unwrap();
        assert_eq!(
            normalized.difficulty,
            NormalizedDifficulty {
                drain_rate: 0.0,
                circle_size: 10.0,
                overall_difficulty: 10.0,
                approach_rate: 10.0,
                slider_multiplier: 3.6,
                slider_tick_rate: 0.5,
            }
        );

        raw.mode = GameMode::Mania;
        raw.difficulty.circle_size = Some(0.0);
        let normalized = normalize(&raw).unwrap();
        assert_eq!(normalized.difficulty.circle_size, 1.0);
    }

    #[test]
    fn offsets_times_and_breaks_only_for_versions_before_five() {
        let mut raw = base_map(4, GameMode::Standard);
        raw.general.preview_time = Some(-1);
        raw.timing_points.push(TimingPoint {
            time: -24.25,
            beat_length: 500.0,
            meter: 4,
            sample_set: 0,
            sample_index: 0,
            volume: 100,
            uninherited: true,
            effects: 0,
        });
        raw.control_points.timing.push(TimingControlPoint {
            time: -24.25,
            beat_length: 500.0,
            meter: 4,
        });
        raw.breaks.push(BreakPeriod {
            start_time: 100.5,
            end_time: 90.0,
        });
        raw.hit_objects.push(circle(50.0, 70.0, 125.75, 1));
        let normalized = normalize(&raw).unwrap();
        assert_eq!(normalized.preview_time, Some(-1));
        assert_eq!(normalized.hit_objects[0].start_time, 149.75);
        assert_eq!(normalized.timing_points[0].time, -0.25);
        assert_eq!(normalized.control_points.timing[0].time, -0.25);
        assert_eq!(normalized.breaks[0].start_time, 124.5);
        assert_eq!(normalized.breaks[0].end_time, 124.5);

        let without_offsets = normalize_with_options(
            &raw,
            NormalizeOptions {
                apply_offsets: false,
            },
        )
        .unwrap();
        assert_eq!(without_offsets.hit_objects[0].start_time, 125.75);

        let raw_v5 = OsuBeatmap { version: 5, ..raw };
        let normalized = normalize(&raw_v5).unwrap();
        assert_eq!(normalized.hit_objects[0].start_time, 125.75);
        assert_eq!(normalized.breaks[0].start_time, 100.5);
    }

    #[test]
    fn legacy_coordinates_spinner_center_and_hold_end_follow_decoder_rules() {
        let mut raw = base_map(127, GameMode::Mania);
        raw.hit_objects.push(circle(-4.75, 512.9, 5.25, 1));
        raw.hit_objects.push(spinner(15.0, 20.0, 20.5, 10.0, 8));
        raw.hit_objects.push(HitObject {
            source_order: 0,
            position: [40.8, 50.7],
            start_time: 30.5,
            type_flags: 128,
            hitsound: 0,
            new_combo: false,
            combo_skip: 0,
            column: Some(0),
            kind: HitObjectKind::Hold { end_time: 20.0 },
            hit_sample: None,
        });
        let normalized = normalize(&raw).unwrap();
        assert_eq!(normalized.hit_objects[0].position, [0.0, 512.0]);
        assert_eq!(normalized.hit_objects[1].position, [256.0, 192.0]);
        assert_eq!(normalized.hit_objects[1].end_time(), 20.5);
        assert_eq!(normalized.hit_objects[2].position, [40.0, 50.0]);
        assert_eq!(normalized.hit_objects[2].end_time(), 30.5);

        let raw_v128 = OsuBeatmap {
            version: 128,
            ..raw
        };
        let normalized = normalize(&raw_v128).unwrap();
        assert_eq!(normalized.hit_objects[0].position, [0.0, 512.0]);
        assert_eq!(
            normalized.hit_objects[2].position,
            [40.79999923706055, 50.70000076293945]
        );
    }

    #[test]
    fn combo_rules_preserve_parse_order_then_apply_stable_sort_and_breaks() {
        let mut raw = base_map(14, GameMode::Standard);
        raw.hit_objects = vec![
            circle(1.0, 1.0, 300.0, 1), // first in source order, but sorts after the spinner.
            spinner(2.0, 2.0, 100.0, 150.0, 4),
            circle(3.0, 3.0, 300.0, 1 | 0x20), // after spinner: implicit combo, no combo offset.
            circle(3.5, 3.5, 600.0, 1),        // exactly at break end is not "after" the break.
            circle(4.0, 4.0, 700.0, 1),
            circle(5.0, 5.0, 701.0, 1 | 4 | 0x30),
        ];
        raw.breaks = vec![BreakPeriod {
            start_time: 500.0,
            end_time: 600.0,
        }];
        let normalized = normalize(&raw).unwrap();

        // Stable order at t=300 keeps source index 0 before source index 2.
        assert!(normalized.hit_objects[0].new_combo); // raw spinner new-combo flag.
        assert!(normalized.hit_objects[1].new_combo); // first object in source order.
        assert!(normalized.hit_objects[2].new_combo); // follows spinner in source order.
        assert!(!normalized.hit_objects[3].new_combo); // break boundary is strict.
        assert!(normalized.hit_objects[4].new_combo); // first object strictly after the break.
        assert!(normalized.hit_objects[5].new_combo); // explicit flag remains set.
        assert_eq!(normalized.hit_objects[5].combo_offset, 3);
        assert_eq!(normalized.hit_objects[2].combo_offset, 0);
        assert_eq!(
            normalized.hit_objects[1].start_time,
            normalized.hit_objects[2].start_time
        );
        assert_eq!(normalized.hit_objects[1].position, [1.0, 1.0]);
        assert_eq!(normalized.hit_objects[2].position, [3.0, 3.0]);

        let mut spinner_first = base_map(14, GameMode::Standard);
        spinner_first.hit_objects =
            vec![spinner(5.0, 6.0, 10.0, 20.0, 8), circle(7.0, 8.0, 30.0, 1)];
        let normalized = normalize(&spinner_first).unwrap();
        assert!(!normalized.hit_objects[0].new_combo); // Spinner does not inherit the first-object rule.
        assert!(normalized.hit_objects[1].new_combo); // Its successor inherits the after-spinner rule.
    }

    #[test]
    fn parser_retains_source_order_for_combo_rules_before_time_sort() {
        let raw = OsuBeatmap::parse_bytes(
            b"osu file format v14\n[General]\nMode:0\n[HitObjects]\n64,192,300,1,0\n64,192,100,8,0,150,0:0:0:0\n64,192,200,1,0\n",
        )
        .unwrap();
        assert_eq!(raw.hit_objects[0].start_time, 100.0);
        assert_eq!(raw.hit_objects[0].source_order, 1);

        let normalized = normalize(&raw).unwrap();
        assert_eq!(normalized.hit_objects[0].start_time, 100.0);
        assert_eq!(normalized.hit_objects[1].start_time, 200.0);
        assert_eq!(normalized.hit_objects[2].start_time, 300.0);
        assert!(normalized.hit_objects[1].new_combo); // This source object follows a spinner.
        assert!(normalized.hit_objects[2].new_combo); // This was first in source order.
    }

    #[test]
    fn slider_span_length_and_path_normalization_match_legacy_boundaries() {
        let mut raw = base_map(127, GameMode::Standard);
        raw.hit_objects
            .push(slider(100.9, 200.9, 50.25, 2 | 4 | 0x20, 3, 0.0));
        let normalized = normalize(&raw).unwrap();
        let NormalizedHitObjectKind::Slider {
            raw_curve,
            path_control_points,
            repeat_count,
            pixel_length,
        } = &normalized.hit_objects[0].kind
        else {
            panic!("slider expected")
        };
        assert_eq!(*repeat_count, 2);
        assert_eq!(*pixel_length, None);
        assert_eq!(raw_curve.points, vec![[111.80000000000001, 180.0]]);
        assert_eq!(normalized.hit_objects[0].position, [100.0, 200.0]);
        assert_eq!(path_control_points, &[[11.0, -20.0]]);
        assert_eq!(normalized.hit_objects[0].combo_offset, 2);

        raw.version = 128;
        let normalized = normalize(&raw).unwrap();
        let NormalizedHitObjectKind::Slider {
            path_control_points,
            ..
        } = &normalized.hit_objects[0].kind
        else {
            panic!("slider expected")
        };
        assert_eq!(
            path_control_points,
            &[[10.900001525878906, -20.899993896484375]]
        );

        raw.hit_objects[0].kind = HitObjectKind::Slider {
            curve: SliderCurve {
                curve_type: SliderCurveType::Bezier,
                points: vec![[110.9, 180.9]],
            },
            repeats: 0,
            pixel_length: 100.0,
            edge_sounds: vec![],
            edge_sets: vec![],
        };
        let normalized = normalize(&raw).unwrap();
        assert!(matches!(
            normalized.hit_objects[0].kind,
            NormalizedHitObjectKind::Slider {
                repeat_count: 0,
                ..
            }
        ));

        raw.hit_objects[0].kind = HitObjectKind::Slider {
            curve: SliderCurve {
                curve_type: SliderCurveType::Bezier,
                points: vec![[110.9, 180.9]],
            },
            repeats: MAX_LEGACY_SLIDER_SPAN_COUNT + 1,
            pixel_length: 100.0,
            edge_sounds: vec![],
            edge_sets: vec![],
        };
        assert!(normalize(&raw).unwrap_err().message.contains("span count"));
    }
}
