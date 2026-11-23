//! Legacy native-Mania `.osu` object conversion for the Mania difficulty vertical slice.
//!
//! This mirrors the native-Mania branches of Ez2Lazer's `ManiaBeatmapConverter`,
//! `PassThroughPatternGenerator`, and `SpinnerPatternGenerator`. It is intentionally not a converter for osu!standard
//! maps played as Mania: those maps require the probabilistic legacy pattern generators, which are a separate port.
//!
//! Slider duration follows `ConvertSlider.Duration = SpanCount * Path.Distance / Velocity`. When osu! supplies a
//! positive pixel length, `SliderPath.ExpectedDistance` makes the final path distance that declared length except
//! for degenerate/ambiguous path cases. Without a declared length, only exact linear paths are calculated here;
//! Bezier/Catmull/perfect-curve approximation is returned as an explicit error until the framework path
//! approximator is ported.

use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::beatmap::{GameMode, SliderCurveType, TimingPoint};
use crate::mania::ManiaNote;
use crate::mods::Mod;
use crate::normalization::{NormalizedBeatmap, NormalizedHitObjectKind};

const MAX_MANIA_COLUMNS: usize = 18;
const BASE_SCORING_DISTANCE: f64 = 100.0;
const DEFAULT_BEAT_LENGTH_MS: f64 = 1000.0;
const MIN_BEAT_LENGTH_MS: f64 = 0.6;
const MAX_BEAT_LENGTH_MS: f64 = 600_000.0;
const MIN_SLIDER_VELOCITY_MULTIPLIER: f64 = 0.1;
const MAX_SLIDER_VELOCITY_MULTIPLIER: f64 = 10.0;
const SPINNER_FINISH_HOLD_CUTOFF_MS: f64 = 1000.0;
const SPINNER_HOLD_MINIMUM_MS: f64 = 100.0;
const HIT_FINISH: i32 = 4;

/// Errors where the normalized input cannot be converted with the information currently retained in its raw model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManiaConversionError {
    UnsupportedMode(GameMode),
    InvalidColumnCount(usize),
    UnknownConversionMod(String),
    NonFiniteTime {
        object_index: usize,
    },
    EndBeforeStart {
        object_index: usize,
    },
    InvalidTimingPoint {
        object_index: usize,
    },
    InvalidSliderVelocity {
        object_index: usize,
    },
    SliderSpanCountOverflow {
        object_index: usize,
    },
    SliderPathDistanceUnavailable {
        object_index: usize,
        curve_type: SliderCurveType,
    },
    AmbiguousSliderPathExtension {
        object_index: usize,
    },
}

impl Display for ManiaConversionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedMode(mode) => {
                write!(f, "Mania converter requires mode=Mania, got {mode:?}")
            }
            Self::InvalidColumnCount(columns) => {
                write!(
                    f,
                    "Mania column count must be in 1..={MAX_MANIA_COLUMNS}, got {columns}"
                )
            }
            Self::UnknownConversionMod(acronym) => {
                write!(
                    f,
                    "unknown conversion mod `{acronym}` may change Mania columns or patterns"
                )
            }
            Self::NonFiniteTime { object_index } => {
                write!(f, "Mania hit object {object_index} has a non-finite time")
            }
            Self::EndBeforeStart { object_index } => {
                write!(f, "Mania hit object {object_index} ends before it starts")
            }
            Self::InvalidTimingPoint { object_index } => {
                write!(f, "Mania slider {object_index} has no valid timing basis")
            }
            Self::InvalidSliderVelocity { object_index } => {
                write!(
                    f,
                    "Mania slider {object_index} has a non-positive or non-finite velocity"
                )
            }
            Self::SliderSpanCountOverflow { object_index } => {
                write!(f, "Mania slider {object_index} span count overflows")
            }
            Self::SliderPathDistanceUnavailable {
                object_index,
                curve_type,
            } => write!(
                f,
                "Mania slider {object_index} has no declared pixel length and its {curve_type:?} path distance is not implemented"
            ),
            Self::AmbiguousSliderPathExtension { object_index } => write!(
                f,
                "Mania slider {object_index} has a non-linear repeated endpoint; the current normalized path model cannot determine lazer's extension exception"
            ),
        }
    }
}

impl Error for ManiaConversionError {}

/// Converts a normalized native Mania beatmap into the note view consumed by [`crate::mania::ManiaStrainCalculator`].
pub struct ManiaBeatmapConverter;

impl ManiaBeatmapConverter {
    /// Convert native Mania objects to playable `(notes, total_columns)`.
    ///
    /// In native Mania maps, key-count mods are ignored by the C# converter. Split Stages only changes the
    /// internal stage arrangement for even 12–18 key maps and preserves total columns, so it does not change this
    /// flat note view. Rate mods do not rewrite source times here; the difficulty calculator applies clock rate.
    pub fn convert(
        beatmap: &NormalizedBeatmap,
        mods: &[Mod],
    ) -> Result<(Vec<ManiaNote>, usize), ManiaConversionError> {
        if beatmap.mode != GameMode::Mania {
            return Err(ManiaConversionError::UnsupportedMode(beatmap.mode));
        }

        for modifier in mods {
            if let Mod::Other(acronym) = modifier {
                return Err(ManiaConversionError::UnknownConversionMod(acronym.clone()));
            }
        }

        let mut target_columns = ((beatmap.difficulty.circle_size as f32) as f64)
            .round_ties_even()
            .max(1.0) as usize;
        target_columns = target_columns.min(MAX_MANIA_COLUMNS);

        let mut dual_stage = false;
        for modifier in mods {
            if matches!(modifier, Mod::SplitStages)
                && target_columns > 10
                && target_columns <= MAX_MANIA_COLUMNS
                && target_columns % 2 == 0
            {
                target_columns /= 2;
                dual_stage = true;
            }
        }

        let total_columns = target_columns * if dual_stage { 2 } else { 1 };
        if !(1..=MAX_MANIA_COLUMNS).contains(&total_columns) {
            return Err(ManiaConversionError::InvalidColumnCount(total_columns));
        }

        let mut notes = Vec::with_capacity(beatmap.hit_objects.len());
        let mut spinner_random = LegacyRandom::new(spinner_seed(beatmap));

        for (object_index, object) in beatmap.hit_objects.iter().enumerate() {
            if !object.start_time.is_finite() || !object.end_time().is_finite() {
                return Err(ManiaConversionError::NonFiniteTime { object_index });
            }

            let column = legacy_column(object.position[0], total_columns);

            let note = match &object.kind {
                NormalizedHitObjectKind::Circle => ManiaNote {
                    start_time_ms: object.start_time,
                    end_time_ms: object.start_time,
                    column,
                },
                NormalizedHitObjectKind::Hold { end_time } => {
                    if *end_time < object.start_time {
                        return Err(ManiaConversionError::EndBeforeStart { object_index });
                    }

                    ManiaNote {
                        start_time_ms: object.start_time,
                        end_time_ms: *end_time,
                        column,
                    }
                }
                NormalizedHitObjectKind::Slider {
                    raw_curve,
                    path_control_points,
                    repeat_count,
                    pixel_length,
                } => {
                    let distance = slider_distance(
                        object_index,
                        raw_curve.curve_type,
                        path_control_points,
                        *pixel_length,
                    )?;
                    let span_count = repeat_count
                        .checked_add(1)
                        .ok_or(ManiaConversionError::SliderSpanCountOverflow { object_index })?;
                    let beat_length = timing_beat_length_at(beatmap, object.start_time)
                        .ok_or(ManiaConversionError::InvalidTimingPoint { object_index })?;
                    let slider_velocity_multiplier =
                        slider_velocity_multiplier_at(beatmap, object.start_time);
                    let velocity = BASE_SCORING_DISTANCE
                        * beatmap.difficulty.slider_multiplier
                        * slider_velocity_multiplier
                        / beat_length;

                    if !velocity.is_finite() || velocity <= 0.0 {
                        return Err(ManiaConversionError::InvalidSliderVelocity { object_index });
                    }

                    let mut effective_span_count = span_count;
                    if distance == 0.0 && span_count > 1 {
                        // C# resets repeats on a zero-distance slider. Its AlmostEquals epsilon is owned by the
                        // framework and unavailable here, so ambiguous near-zero paths are surfaced rather than guessed.
                        effective_span_count = 1;
                    } else if distance > 0.0 && distance < 1e-7 && span_count > 1 {
                        return Err(ManiaConversionError::SliderPathDistanceUnavailable {
                            object_index,
                            curve_type: raw_curve.curve_type,
                        });
                    }

                    let duration = effective_span_count as f64 * distance / velocity;
                    ManiaNote {
                        start_time_ms: object.start_time,
                        end_time_ms: object.start_time + duration,
                        column,
                    }
                }
                NormalizedHitObjectKind::Spinner { end_time } => {
                    if *end_time < object.start_time {
                        return Err(ManiaConversionError::EndBeforeStart { object_index });
                    }

                    let spinner_end_time = end_time.trunc();
                    let generate_hold =
                        spinner_end_time - object.start_time >= SPINNER_HOLD_MINIMUM_MS;
                    let is_short_finish_spinner = total_columns == 8
                        && object.hitsound & HIT_FINISH != 0
                        && spinner_end_time - object.start_time < SPINNER_FINISH_HOLD_CUTOFF_MS;

                    let spinner_column = if is_short_finish_spinner {
                        0
                    } else {
                        let lower_bound = if total_columns == 8 { 1 } else { 0 };
                        spinner_random.next_column(lower_bound, total_columns)
                    };

                    ManiaNote {
                        start_time_ms: object.start_time,
                        end_time_ms: if generate_hold {
                            spinner_end_time
                        } else {
                            object.start_time
                        },
                        column: spinner_column,
                    }
                }
            };

            notes.push(note);
        }

        // BeatmapConverter orders converted objects by StartTime after conversion; preserve source order for ties.
        notes.sort_by(|left, right| left.start_time_ms.total_cmp(&right.start_time_ms));

        Ok((notes, total_columns))
    }
}

fn legacy_column(x: f64, total_columns: usize) -> usize {
    let x = x as f32;
    let column_width = 512.0_f32 / total_columns as f32;
    ((x / column_width).floor() as i32).clamp(0, total_columns as i32 - 1) as usize
}

fn timing_beat_length_at(beatmap: &NormalizedBeatmap, time: f64) -> Option<f64> {
    let mut points: Vec<(usize, &TimingPoint)> = beatmap.timing_points.iter().enumerate().collect();
    points.sort_by(|(left_index, left), (right_index, right)| {
        left.time
            .total_cmp(&right.time)
            .then_with(|| left_index.cmp(right_index))
    });

    let mut red_points = Vec::new();
    let mut index = 0;
    while index < points.len() {
        let group_time = points[index].1.time;
        let mut first_red = None;
        while index < points.len() && points[index].1.time == group_time {
            let point = points[index].1;
            if point.uninherited && first_red.is_none() {
                // LegacyBeatmapDecoder's same-time pending-point merge keeps the first red-line timing point.
                first_red = Some(point);
            }
            index += 1;
        }
        if let Some(point) = first_red {
            red_points.push(point);
        }
    }

    let point = red_points
        .iter()
        .rev()
        .find(|point| point.time <= time)
        .copied()
        .or_else(|| red_points.first().copied());

    match point {
        Some(point) if point.beat_length.is_finite() => Some(
            point
                .beat_length
                .clamp(MIN_BEAT_LENGTH_MS, MAX_BEAT_LENGTH_MS),
        ),
        Some(_) => None,
        None => Some(DEFAULT_BEAT_LENGTH_MS),
    }
}

fn slider_velocity_multiplier_at(beatmap: &NormalizedBeatmap, time: f64) -> f64 {
    let mut timing_points: Vec<(usize, &TimingPoint)> =
        beatmap.timing_points.iter().enumerate().collect();
    timing_points.sort_by(|(left_index, left), (right_index, right)| {
        left.time
            .total_cmp(&right.time)
            .then_with(|| left_index.cmp(right_index))
    });

    let mut groups = Vec::new();
    let mut index = 0;
    while index < timing_points.len() {
        let group_time = timing_points[index].1.time;
        let mut first_red = false;
        let mut last_green = None;

        while index < timing_points.len() && timing_points[index].1.time == group_time {
            let point = timing_points[index].1;
            if point.uninherited {
                first_red = true;
            } else {
                last_green = Some(point);
            }
            index += 1;
        }

        let multiplier = if let Some(green) = last_green {
            if green.beat_length < 0.0 {
                (100.0 / -green.beat_length).clamp(
                    MIN_SLIDER_VELOCITY_MULTIPLIER,
                    MAX_SLIDER_VELOCITY_MULTIPLIER,
                )
            } else {
                1.0
            }
        } else if first_red {
            1.0
        } else {
            continue;
        };

        groups.push((group_time, multiplier));
    }

    groups
        .iter()
        .rev()
        .find(|(point_time, _)| *point_time <= time)
        .or_else(|| groups.first())
        .map(|(_, multiplier)| *multiplier)
        .unwrap_or(1.0)
}

fn slider_distance(
    object_index: usize,
    curve_type: SliderCurveType,
    path_control_points: &[[f64; 2]],
    expected_distance: Option<f64>,
) -> Result<f64, ManiaConversionError> {
    // ConvertHitObjectParser turns a declared length of zero into `null`, meaning use calculated path distance.
    let expected_distance = expected_distance.filter(|distance| *distance != 0.0);

    // ConvertHitObjectParser prepends the slider origin. Its Vec2 point arithmetic is f32, including the relative
    // subtraction already represented by `path_control_points` in the normalized model.
    let mut points = Vec::with_capacity(path_control_points.len() + 1);
    points.push([0.0_f32, 0.0_f32]);
    points.extend(
        path_control_points
            .iter()
            .map(|point| [point[0] as f32, point[1] as f32]),
    );

    if points
        .iter()
        .any(|point| !point[0].is_finite() || !point[1].is_finite())
    {
        return Err(ManiaConversionError::SliderPathDistanceUnavailable {
            object_index,
            curve_type,
        });
    }

    if points.len() <= 1 || points.iter().all(|point| *point == points[0]) {
        return Ok(0.0);
    }

    let is_exact_polyline = curve_type == SliderCurveType::Linear || points.len() <= 2;
    let has_repeated_endpoint =
        points.len() >= 2 && points[points.len() - 1] == points[points.len() - 2];

    if is_exact_polyline {
        let calculated_distance = polyline_distance(&points);
        return match expected_distance {
            Some(expected) if expected.is_finite() && expected >= 0.0 => {
                // SliderPath avoids extending a path whose last two calculated vertices are identical.
                if expected > calculated_distance && has_repeated_endpoint {
                    Ok(calculated_distance)
                } else {
                    Ok(expected)
                }
            }
            Some(_) => Err(ManiaConversionError::SliderPathDistanceUnavailable {
                object_index,
                curve_type,
            }),
            None => Ok(calculated_distance),
        };
    }

    if expected_distance.is_none() {
        return Err(ManiaConversionError::SliderPathDistanceUnavailable {
            object_index,
            curve_type,
        });
    }

    let expected = expected_distance.unwrap();
    if !expected.is_finite() || expected < 0.0 {
        return Err(ManiaConversionError::SliderPathDistanceUnavailable {
            object_index,
            curve_type,
        });
    }

    if has_repeated_endpoint {
        return Err(ManiaConversionError::AmbiguousSliderPathExtension { object_index });
    }

    // For a non-degenerate path with declared pixel length, SliderPath's final Distance is exactly
    // ExpectedDistance after shortening/extension. The normalized model lacks the segment/degree data needed to
    // reconstruct the exceptional zero-length endpoint case, which is rejected above.
    Ok(expected)
}

fn polyline_distance(points: &[[f32; 2]]) -> f64 {
    points
        .windows(2)
        .map(|segment| {
            let x = segment[1][0] - segment[0][0];
            let y = segment[1][1] - segment[0][1];
            ((x * x + y * y).sqrt()) as f64
        })
        .sum()
}

fn spinner_seed(beatmap: &NormalizedBeatmap) -> i32 {
    // ManiaBeatmapConverter seeds LegacyRandom before applying any conversion mod. Beatmap difficulty fields are
    // floats in C#, so retain the f32 addition/rounding for drain + circle size and approach rate.
    let drain_and_circle = (beatmap.difficulty.drain_rate as f32
        + beatmap.difficulty.circle_size as f32)
        .round_ties_even() as i32;
    let overall_term =
        ((beatmap.difficulty.overall_difficulty as f32) as f64 * 41.2).trunc() as i32;
    let approach_term = (beatmap.difficulty.approach_rate as f32).round_ties_even() as i32;

    drain_and_circle
        .wrapping_mul(20)
        .wrapping_add(overall_term)
        .wrapping_add(approach_term)
}

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

    fn next_double(&mut self) -> f64 {
        (self.next_u32() & 0x7fff_ffff) as f64 / 2_147_483_648.0
    }

    fn next_column(&mut self, lower_bound: usize, upper_bound: usize) -> usize {
        (lower_bound as f64 + self.next_double() * (upper_bound - lower_bound) as f64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beatmap::{
        ControlPoints, DifficultyControlPoint, EffectControlPoint, TimingControlPoint,
    };
    use crate::normalization::{NormalizedDifficulty, NormalizedHitObject};

    fn beatmap(circle_size: f64, objects: Vec<NormalizedHitObject>) -> NormalizedBeatmap {
        NormalizedBeatmap {
            version: 128,
            mode: GameMode::Mania,
            difficulty: NormalizedDifficulty {
                drain_rate: 5.0,
                circle_size,
                overall_difficulty: 5.0,
                approach_rate: 5.0,
                slider_multiplier: 1.4,
                slider_tick_rate: 1.0,
            },
            preview_time: None,
            timing_points: Vec::new(),
            control_points: ControlPoints {
                timing: Vec::<TimingControlPoint>::new(),
                difficulty: Vec::<DifficultyControlPoint>::new(),
                sample: Vec::new(),
                effect: Vec::<EffectControlPoint>::new(),
            },
            breaks: Vec::new(),
            hit_objects: objects,
        }
    }

    fn normalized_object(
        x: f64,
        start_time: f64,
        hitsound: i32,
        kind: NormalizedHitObjectKind,
    ) -> NormalizedHitObject {
        NormalizedHitObject {
            position: [x, 192.0],
            start_time,
            legacy_type_flags: 0,
            hitsound,
            new_combo: false,
            combo_offset: 0,
            column: None,
            kind,
            hit_sample: None,
        }
    }

    #[test]
    fn converts_native_notes_holds_and_recomputes_columns() {
        let map = beatmap(
            4.0,
            vec![
                normalized_object(127.9, 100.0, 0, NormalizedHitObjectKind::Circle),
                normalized_object(
                    128.0,
                    200.0,
                    0,
                    NormalizedHitObjectKind::Hold { end_time: 500.0 },
                ),
            ],
        );

        let (notes, columns) = ManiaBeatmapConverter::convert(&map, &[]).unwrap();
        assert_eq!(columns, 4);
        assert_eq!(
            notes[0],
            ManiaNote {
                start_time_ms: 100.0,
                end_time_ms: 100.0,
                column: 0
            }
        );
        assert_eq!(
            notes[1],
            ManiaNote {
                start_time_ms: 200.0,
                end_time_ms: 500.0,
                column: 1
            }
        );
    }

    #[test]
    fn native_key_count_mod_is_ignored_and_split_stages_preserves_total_columns() {
        let twelve_key_map = beatmap(
            12.0,
            vec![normalized_object(
                260.0,
                100.0,
                0,
                NormalizedHitObjectKind::Circle,
            )],
        );
        let (_, native_columns) =
            ManiaBeatmapConverter::convert(&twelve_key_map, &[Mod::KeyCount { keys: 7 }]).unwrap();
        let (_, split_columns) =
            ManiaBeatmapConverter::convert(&twelve_key_map, &[Mod::SplitStages]).unwrap();
        assert_eq!(native_columns, 12);
        assert_eq!(split_columns, 12);

        let odd_key_map = beatmap(
            11.0,
            vec![normalized_object(
                0.0,
                100.0,
                0,
                NormalizedHitObjectKind::Circle,
            )],
        );
        assert_eq!(
            ManiaBeatmapConverter::convert(&odd_key_map, &[Mod::SplitStages])
                .unwrap()
                .1,
            11
        );
    }

    #[test]
    fn circle_size_rounds_midpoint_to_even_before_columns_are_resolved() {
        let map = beatmap(
            4.5,
            vec![normalized_object(
                511.0,
                100.0,
                0,
                NormalizedHitObjectKind::Circle,
            )],
        );
        let (notes, columns) = ManiaBeatmapConverter::convert(&map, &[]).unwrap();
        assert_eq!(columns, 4);
        assert_eq!(notes[0].column, 3);
    }

    #[test]
    fn slider_expected_length_and_timing_control_points_set_hold_duration() {
        let mut map = beatmap(
            4.0,
            vec![normalized_object(
                0.0,
                100.0,
                0,
                NormalizedHitObjectKind::Slider {
                    raw_curve: crate::beatmap::SliderCurve {
                        curve_type: SliderCurveType::Bezier,
                        points: vec![[100.0, 0.0]],
                    },
                    path_control_points: vec![[100.0, 0.0]],
                    repeat_count: 1,
                    pixel_length: Some(100.0),
                },
            )],
        );
        map.timing_points = vec![
            TimingPoint {
                time: 0.0,
                beat_length: 500.0,
                meter: 4,
                sample_set: 0,
                sample_index: 0,
                volume: 100,
                uninherited: true,
                effects: 0,
            },
            TimingPoint {
                time: 0.0,
                beat_length: -50.0,
                meter: 4,
                sample_set: 0,
                sample_index: 0,
                volume: 100,
                uninherited: false,
                effects: 0,
            },
        ];

        let (notes, _) = ManiaBeatmapConverter::convert(&map, &[]).unwrap();
        let velocity = 100.0 * 1.4 * 2.0 / 500.0;
        assert!((notes[0].end_time_ms - (100.0 + 2.0 * 100.0 / velocity)).abs() < 1e-12);
    }

    #[test]
    fn linear_slider_without_declared_length_uses_its_exact_polyline_distance() {
        let map = beatmap(
            4.0,
            vec![normalized_object(
                0.0,
                100.0,
                0,
                NormalizedHitObjectKind::Slider {
                    raw_curve: crate::beatmap::SliderCurve {
                        curve_type: SliderCurveType::Linear,
                        points: vec![[3.0, 4.0], [6.0, 8.0]],
                    },
                    path_control_points: vec![[3.0, 4.0], [6.0, 8.0]],
                    repeat_count: 0,
                    pixel_length: None,
                },
            )],
        );

        let (notes, _) = ManiaBeatmapConverter::convert(&map, &[]).unwrap();
        let velocity = 100.0 * 1.4 / DEFAULT_BEAT_LENGTH_MS;
        assert!((notes[0].end_time_ms - (100.0 + 10.0 / velocity)).abs() < 1e-12);
    }

    #[test]
    fn last_green_sv_at_a_timestamp_overrides_the_red_line_and_earlier_green_lines() {
        let mut map = beatmap(
            4.0,
            vec![normalized_object(
                0.0,
                100.0,
                0,
                NormalizedHitObjectKind::Slider {
                    raw_curve: crate::beatmap::SliderCurve {
                        curve_type: SliderCurveType::Linear,
                        points: vec![[100.0, 0.0]],
                    },
                    path_control_points: vec![[100.0, 0.0]],
                    repeat_count: 0,
                    pixel_length: Some(100.0),
                },
            )],
        );
        map.timing_points = vec![
            TimingPoint {
                time: 0.0,
                beat_length: 500.0,
                meter: 4,
                sample_set: 0,
                sample_index: 0,
                volume: 100,
                uninherited: true,
                effects: 0,
            },
            TimingPoint {
                time: 100.0,
                beat_length: -50.0,
                meter: 4,
                sample_set: 0,
                sample_index: 0,
                volume: 100,
                uninherited: false,
                effects: 0,
            },
            TimingPoint {
                time: 100.0,
                beat_length: -25.0,
                meter: 4,
                sample_set: 0,
                sample_index: 0,
                volume: 100,
                uninherited: false,
                effects: 0,
            },
        ];

        let (notes, _) = ManiaBeatmapConverter::convert(&map, &[]).unwrap();
        let velocity = 100.0 * 1.4 * 4.0 / 500.0;
        assert!((notes[0].end_time_ms - (100.0 + 100.0 / velocity)).abs() < 1e-12);
    }

    #[test]
    fn zero_length_linear_slider_does_not_extend_and_clears_repeats() {
        let map = beatmap(
            4.0,
            vec![normalized_object(
                0.0,
                100.0,
                0,
                NormalizedHitObjectKind::Slider {
                    raw_curve: crate::beatmap::SliderCurve {
                        curve_type: SliderCurveType::Linear,
                        points: vec![[0.0, 0.0]],
                    },
                    path_control_points: vec![[0.0, 0.0]],
                    repeat_count: 2,
                    pixel_length: Some(100.0),
                },
            )],
        );

        let (notes, _) = ManiaBeatmapConverter::convert(&map, &[]).unwrap();
        assert_eq!(notes[0].start_time_ms, notes[0].end_time_ms);
    }

    #[test]
    fn no_declared_non_linear_slider_distance_is_an_explicit_error() {
        let map = beatmap(
            4.0,
            vec![normalized_object(
                0.0,
                100.0,
                0,
                NormalizedHitObjectKind::Slider {
                    raw_curve: crate::beatmap::SliderCurve {
                        curve_type: SliderCurveType::Bezier,
                        points: vec![[100.0, 50.0], [200.0, 0.0]],
                    },
                    path_control_points: vec![[100.0, 50.0], [200.0, 0.0]],
                    repeat_count: 0,
                    pixel_length: None,
                },
            )],
        );

        assert!(matches!(
            ManiaBeatmapConverter::convert(&map, &[]),
            Err(ManiaConversionError::SliderPathDistanceUnavailable {
                object_index: 0,
                ..
            })
        ));
    }

    #[test]
    fn spinner_random_column_matches_legacy_seed_and_finish_shortcut() {
        let normal = beatmap(
            4.0,
            vec![
                normalized_object(
                    256.0,
                    100.0,
                    0,
                    NormalizedHitObjectKind::Spinner { end_time: 1500.0 },
                ),
                normalized_object(
                    256.0,
                    2000.0,
                    0,
                    NormalizedHitObjectKind::Spinner { end_time: 3400.0 },
                ),
                normalized_object(
                    256.0,
                    4000.0,
                    0,
                    NormalizedHitObjectKind::Spinner { end_time: 5400.0 },
                ),
            ],
        );
        let (normal_notes, _) = ManiaBeatmapConverter::convert(&normal, &[]).unwrap();
        assert_eq!(normal_notes[0].column, 0); // First LegacyRandom sample from seed 391.
        assert_eq!(normal_notes[0].end_time_ms, 1500.0);
        assert_eq!(normal_notes[1].column, 0);
        assert_eq!(normal_notes[2].column, 1);

        let eight_key_finish = beatmap(
            8.0,
            vec![normalized_object(
                256.0,
                100.0,
                HIT_FINISH,
                NormalizedHitObjectKind::Spinner { end_time: 900.0 },
            )],
        );
        let (finish_notes, columns) =
            ManiaBeatmapConverter::convert(&eight_key_finish, &[]).unwrap();
        assert_eq!(columns, 8);
        assert_eq!(finish_notes[0].column, 0);
        assert_eq!(finish_notes[0].end_time_ms, 900.0);
    }

    #[test]
    fn rejects_non_mania_maps_and_unknown_conversion_mods() {
        let mut standard = beatmap(4.0, Vec::new());
        standard.mode = GameMode::Standard;
        assert_eq!(
            ManiaBeatmapConverter::convert(&standard, &[]),
            Err(ManiaConversionError::UnsupportedMode(GameMode::Standard))
        );

        let map = beatmap(4.0, Vec::new());
        assert_eq!(
            ManiaBeatmapConverter::convert(&map, &[Mod::Other("Krr".into())]),
            Err(ManiaConversionError::UnknownConversionMod("Krr".into()))
        );
    }
}
