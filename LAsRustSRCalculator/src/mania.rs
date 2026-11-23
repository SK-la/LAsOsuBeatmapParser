//! Official osu!mania difficulty calculation over already-converted Mania notes.
//!
//! This module intentionally accepts normalized note times and columns instead of parser hit-object types. The
//! caller must first run the ruleset converter (including legacy pattern generation, if applicable) and pass its
//! playable Mania objects. It mirrors the current C# `ManiaDifficultyCalculator` / `Strain` path; .osu conversion
//! is a separate layer.

use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::attributes::ManiaDifficultyAttributes;
use crate::beatmap::{GameMode, OsuBeatmap};
use crate::calculator::{CalculationError, DifficultyCalculator};
use crate::mania_converter::{ManiaBeatmapConverter, ManiaConversionError};
use crate::mods::{Mod, calculate_rate_with_mods};
use crate::normalization::normalize;

const SECTION_LENGTH_MS: f64 = 400.0;
const SECTION_DECAY: f64 = 0.9;
const INDIVIDUAL_DECAY: f64 = 0.125;
const OVERALL_DECAY: f64 = 0.30;
const DIFFICULTY_MULTIPLIER: f64 = 0.018;
const RELEASE_THRESHOLD_MS: f64 = 30.0;
const HOLD_OVERLAP_TOLERANCE_MS: f64 = 1.0;
const MAX_COLUMNS: usize = 18;

/// One note from a converted Mania beatmap.
///
/// Taps have `end_time_ms == start_time_ms`; holds carry their resolved release time. Times are in source-map
/// milliseconds, before playback-rate adjustment. The column is zero-based.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ManiaNote {
    pub start_time_ms: f64,
    pub end_time_ms: f64,
    pub column: usize,
}

/// Invalid normalized inputs to [`ManiaStrainCalculator`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManiaDifficultyError {
    InvalidColumnCount(usize),
    ColumnOutOfRange { column: usize, total_columns: usize },
    NonFiniteTime { note_index: usize },
    EndBeforeStart { note_index: usize },
    InvalidClockRate,
    MaxComboOverflow,
}

impl Display for ManiaDifficultyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidColumnCount(columns) => write!(
                f,
                "Mania column count must be in 1..={MAX_COLUMNS}, got {columns}"
            ),
            Self::ColumnOutOfRange {
                column,
                total_columns,
            } => {
                write!(
                    f,
                    "Mania note column {column} is outside 0..{total_columns}"
                )
            }
            Self::NonFiniteTime { note_index } => {
                write!(f, "Mania note {note_index} has a non-finite time")
            }
            Self::EndBeforeStart { note_index } => {
                write!(f, "Mania note {note_index} ends before it starts")
            }
            Self::InvalidClockRate => {
                f.write_str("Mania clock rate must be finite and greater than zero")
            }
            Self::MaxComboOverflow => {
                f.write_str("Mania max combo exceeds the C# Int32 result range")
            }
        }
    }
}

impl Error for ManiaDifficultyError {}

/// Port of the official Mania strain difficulty calculation over converted notes.
pub struct ManiaStrainCalculator;

impl ManiaStrainCalculator {
    /// Calculate official Mania star rating and maximum combo.
    ///
    /// `notes` must be the playable objects emitted by the Mania ruleset converter. `total_columns` is the
    /// converted beatmap's total stage column count. `clock_rate` is the combined playback rate (1.0 without rate
    /// mods); object times are divided by it before strain and section calculations, matching C#.
    pub fn calculate(
        notes: &[ManiaNote],
        total_columns: usize,
        clock_rate: f64,
    ) -> Result<ManiaDifficultyAttributes, ManiaDifficultyError> {
        if !(1..=MAX_COLUMNS).contains(&total_columns) {
            return Err(ManiaDifficultyError::InvalidColumnCount(total_columns));
        }

        if !clock_rate.is_finite() || clock_rate <= 0.0 {
            return Err(ManiaDifficultyError::InvalidClockRate);
        }

        let mut max_combo = 0_i32;

        for (note_index, note) in notes.iter().enumerate() {
            if !note.start_time_ms.is_finite() || !note.end_time_ms.is_finite() {
                return Err(ManiaDifficultyError::NonFiniteTime { note_index });
            }

            if note.end_time_ms < note.start_time_ms {
                return Err(ManiaDifficultyError::EndBeforeStart { note_index });
            }

            if note.column >= total_columns {
                return Err(ManiaDifficultyError::ColumnOutOfRange {
                    column: note.column,
                    total_columns,
                });
            }

            // C# HoldNote combo: 1 + (int)((EndTime - StartTime) / 100). Taps have a zero duration and
            // therefore contribute one as well.
            let object_combo = 1.0 + ((note.end_time_ms - note.start_time_ms) / 100.0).trunc();
            if object_combo > i32::MAX as f64 || max_combo as f64 + object_combo > i32::MAX as f64 {
                return Err(ManiaDifficultyError::MaxComboOverflow);
            }
            max_combo += object_combo as i32;
        }

        if notes.len() < 2 {
            return Ok(ManiaDifficultyAttributes {
                mods: Vec::new(),
                star_rating: 0.0,
                max_combo,
            });
        }

        // C# sorts on (int)Math.Round(StartTime), using midpoint-to-even rounding. The sort is stable for equal
        // keys, preserving converter order for chord notes.
        let mut sorted: Vec<(usize, ManiaNote)> = notes.iter().copied().enumerate().collect();
        sorted.sort_by(|(left_index, left), (right_index, right)| {
            let left_time = left.start_time_ms.round_ties_even() as i32;
            let right_time = right.start_time_ms.round_ties_even() as i32;
            left_time
                .cmp(&right_time)
                .then_with(|| left_index.cmp(right_index))
        });

        let difficulty_objects = build_difficulty_objects(&sorted, total_columns, clock_rate);
        let difficulty_value = calculate_strain_difficulty(&difficulty_objects);

        Ok(ManiaDifficultyAttributes {
            mods: Vec::new(),
            star_rating: difficulty_value * DIFFICULTY_MULTIPLIER,
            max_combo,
        })
    }
}

/// Official osu!mania beatmap difficulty calculator.
pub struct ManiaDifficultyCalculator;

impl ManiaDifficultyCalculator {
    /// Calculator version from Ez2Lazer's `ManiaDifficultyCalculator`.
    pub const VERSION: u32 = 20241007;

    /// Calculate Mania difficulty from an `.osu` beatmap and its mods.
    ///
    /// Legacy sliders and spinners require the Mania pattern converter; they are rejected until that converter is
    /// available rather than silently counted as one tap.
    pub fn calculate(
        &self,
        beatmap: &OsuBeatmap,
        mods: &[Mod],
    ) -> Result<ManiaDifficultyAttributes, CalculationError> {
        let normalized = normalize(beatmap)
            .map_err(|error| CalculationError::InvalidBeatmap(error.to_string()))?;
        let (notes, effective_columns) =
            ManiaBeatmapConverter::convert(&normalized, mods).map_err(map_conversion_error)?;
        let clock_rate = calculate_rate_with_mods(mods)
            .map_err(|error| CalculationError::InvalidMod(error.to_string()))?;
        let mut attributes =
            ManiaStrainCalculator::calculate(&notes, effective_columns, clock_rate)
                .map_err(|error| CalculationError::InvalidBeatmap(error.to_string()))?;
        attributes.mods = mods.to_vec();
        Ok(attributes)
    }
}

fn map_conversion_error(error: ManiaConversionError) -> CalculationError {
    match error {
        ManiaConversionError::UnsupportedMode(actual) => CalculationError::WrongRuleset {
            expected: GameMode::Mania,
            actual,
        },
        ManiaConversionError::UnknownConversionMod(acronym) => CalculationError::InvalidMod(
            format!("unknown conversion mod `{acronym}` may change Mania columns or patterns"),
        ),
        ManiaConversionError::InvalidColumnCount(columns) => CalculationError::InvalidMod(format!(
            "Mania total column count must be in 1..=18, got {columns}"
        )),
        ManiaConversionError::SliderPathDistanceUnavailable { .. }
        | ManiaConversionError::InvalidTimingPoint { .. }
        | ManiaConversionError::InvalidSliderVelocity { .. }
        | ManiaConversionError::SliderSpanCountOverflow { .. }
        | ManiaConversionError::NonFiniteTime { .. }
        | ManiaConversionError::EndBeforeStart { .. } => {
            CalculationError::InvalidBeatmap(error.to_string())
        }
    }
}

impl DifficultyCalculator for ManiaDifficultyCalculator {
    type Attributes = ManiaDifficultyAttributes;

    fn version(&self) -> u32 {
        Self::VERSION
    }

    fn calculate(
        &self,
        beatmap: &OsuBeatmap,
        mods: &[Mod],
    ) -> Result<Self::Attributes, CalculationError> {
        ManiaDifficultyCalculator::calculate(self, beatmap, mods)
    }
}

#[derive(Clone, Copy, Debug)]
struct NoteWindow {
    start_time_ms: f64,
    end_time_ms: f64,
}

#[derive(Clone, Debug)]
struct DifficultyObject {
    start_time_ms: f64,
    end_time_ms: f64,
    delta_time_ms: f64,
    previous_start_time_ms: f64,
    column_strain_time_ms: f64,
    column: usize,
    previous_by_column: Vec<Option<NoteWindow>>,
}

fn build_difficulty_objects(
    sorted_notes: &[(usize, ManiaNote)],
    total_columns: usize,
    clock_rate: f64,
) -> Vec<DifficultyObject> {
    let mut difficulty_objects = Vec::with_capacity(sorted_notes.len() - 1);
    let mut previous_by_column: Vec<Option<NoteWindow>> = vec![None; total_columns];
    let mut previous_start_time = sorted_notes[0].1.start_time_ms / clock_rate;

    // C# creates no ManiaDifficultyHitObject for the first playable object. It is used only as the time
    // predecessor for the first difficulty object; it does not seed per-column history or hold overlap.
    for (sorted_index, &(_, note)) in sorted_notes.iter().enumerate().skip(1) {
        let previous_note = sorted_notes[sorted_index - 1].1;
        let start_time_ms = note.start_time_ms / clock_rate;
        let end_time_ms = note.end_time_ms / clock_rate;
        // The common C# DifficultyHitObject computes the delta in this order: subtract source timestamps, then
        // divide by the clock rate. Keep the order because the two equivalent expressions can round differently.
        let delta_time_ms = (note.start_time_ms - previous_note.start_time_ms) / clock_rate;
        let column_strain_time_ms = previous_by_column[note.column]
            .map(|previous| start_time_ms - previous.start_time_ms)
            .unwrap_or(start_time_ms);

        difficulty_objects.push(DifficultyObject {
            start_time_ms,
            end_time_ms,
            delta_time_ms,
            previous_start_time_ms: previous_start_time,
            column_strain_time_ms,
            column: note.column,
            previous_by_column: previous_by_column.clone(),
        });

        previous_by_column[note.column] = Some(NoteWindow {
            start_time_ms,
            end_time_ms,
        });
        previous_start_time = start_time_ms;
    }

    difficulty_objects
}

fn calculate_strain_difficulty(objects: &[DifficultyObject]) -> f64 {
    let mut individual_strains = vec![
        0.0;
        objects
            .iter()
            .map(|object| object.column)
            .max()
            .unwrap_or(0)
            + 1
    ];
    let mut highest_individual_strain = 0.0;
    let mut overall_strain = 1.0;
    let mut current_strain = 0.0;

    let mut section_peaks = Vec::new();
    let mut current_section_peak = 0.0;
    let mut current_section_end = 0.0;

    for (index, current) in objects.iter().enumerate() {
        if index == 0 {
            // StrainSkill aligns section ends to the first difficulty object's 400ms grid.
            current_section_end =
                (current.start_time_ms / SECTION_LENGTH_MS).ceil() * SECTION_LENGTH_MS;
        }

        while current.start_time_ms > current_section_end {
            section_peaks.push(current_section_peak);
            current_section_peak = calculate_initial_strain(
                current_section_end,
                current,
                highest_individual_strain,
                overall_strain,
            );
            current_section_end += SECTION_LENGTH_MS;
        }

        individual_strains[current.column] *=
            decay(INDIVIDUAL_DECAY, current.column_strain_time_ms);
        individual_strains[current.column] += evaluate_individual_strain(current);

        // Notes within 1ms are treated as a chord: retain the hardest individual strain regardless of processing
        // order. This condition intentionally has no lower bound, matching C# for rounded-time sort edge cases.
        highest_individual_strain = if current.delta_time_ms <= 1.0 {
            highest_individual_strain.max(individual_strains[current.column])
        } else {
            individual_strains[current.column]
        };

        overall_strain *= decay(OVERALL_DECAY, current.delta_time_ms);
        overall_strain += evaluate_overall_strain(current);

        // StrainDecaySkill uses a base of 1 for Mania. Its CurrentStrain telescopes to the current combined
        // individual + overall strain; retain the same operations to keep floating-point ordering aligned.
        let strain_value = highest_individual_strain + overall_strain - current_strain;
        current_strain *= decay(1.0, current.delta_time_ms);
        current_strain += strain_value;
        current_section_peak = current_section_peak.max(current_strain);
    }

    if !objects.is_empty() {
        section_peaks.push(current_section_peak);
    }

    section_peaks.retain(|peak| *peak > 0.0);
    section_peaks.sort_by(|left, right| right.total_cmp(left));

    let mut difficulty = 0.0;
    let mut weight = 1.0;
    for peak in section_peaks {
        difficulty += peak * weight;
        weight *= SECTION_DECAY;
    }
    difficulty
}

fn calculate_initial_strain(
    section_time_ms: f64,
    current: &DifficultyObject,
    highest_individual_strain: f64,
    overall_strain: f64,
) -> f64 {
    let elapsed = section_time_ms - current.previous_start_time_ms;
    decay(INDIVIDUAL_DECAY, elapsed) * highest_individual_strain
        + decay(OVERALL_DECAY, elapsed) * overall_strain
}

fn evaluate_individual_strain(current: &DifficultyObject) -> f64 {
    let is_held = current.previous_by_column.iter().flatten().any(|previous| {
        definitely_bigger(
            previous.end_time_ms,
            current.end_time_ms,
            HOLD_OVERLAP_TOLERANCE_MS,
        ) && definitely_bigger(
            current.start_time_ms,
            previous.start_time_ms,
            HOLD_OVERLAP_TOLERANCE_MS,
        )
    });

    2.0 * if is_held { 1.25 } else { 1.0 }
}

fn evaluate_overall_strain(current: &DifficultyObject) -> f64 {
    let mut is_overlapping = false;
    let mut closest_end_time_ms = (current.end_time_ms - current.start_time_ms).abs();
    let mut is_held = false;

    for previous in current.previous_by_column.iter().flatten() {
        is_overlapping |= definitely_bigger(
            previous.end_time_ms,
            current.start_time_ms,
            HOLD_OVERLAP_TOLERANCE_MS,
        ) && definitely_bigger(
            current.end_time_ms,
            previous.end_time_ms,
            HOLD_OVERLAP_TOLERANCE_MS,
        ) && definitely_bigger(
            current.start_time_ms,
            previous.start_time_ms,
            HOLD_OVERLAP_TOLERANCE_MS,
        );

        if definitely_bigger(
            previous.end_time_ms,
            current.end_time_ms,
            HOLD_OVERLAP_TOLERANCE_MS,
        ) && definitely_bigger(
            current.start_time_ms,
            previous.start_time_ms,
            HOLD_OVERLAP_TOLERANCE_MS,
        ) {
            is_held = true;
        }

        closest_end_time_ms =
            closest_end_time_ms.min((current.end_time_ms - previous.end_time_ms).abs());
    }

    let hold_addition = if is_overlapping {
        1.0 / (1.0 + (0.27 * (RELEASE_THRESHOLD_MS - closest_end_time_ms)).exp())
    } else {
        0.0
    };

    (1.0 + hold_addition) * if is_held { 1.25 } else { 1.0 }
}

fn definitely_bigger(left: f64, right: f64, tolerance: f64) -> bool {
    left - right > tolerance
}

fn decay(decay_base: f64, delta_time_ms: f64) -> f64 {
    decay_base.powf(delta_time_ms / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beatmap::OsuBeatmap;

    fn note(start_time_ms: f64, end_time_ms: f64, column: usize) -> ManiaNote {
        ManiaNote {
            start_time_ms,
            end_time_ms,
            column,
        }
    }

    fn calculate(notes: &[ManiaNote], columns: usize, rate: f64) -> ManiaDifficultyAttributes {
        ManiaStrainCalculator::calculate(notes, columns, rate).unwrap()
    }

    #[test]
    fn calculator_api_reads_osu_bytes_and_matches_two_note_lazer_golden() {
        let beatmap = OsuBeatmap::parse_bytes(
            b"osu file format v14\n[General]\nMode:3\n[Difficulty]\nCircleSize:4\n[HitObjects]\n64,192,0,1,0\n192,192,1000,1,0\n",
        )
        .unwrap();
        let calculator = ManiaDifficultyCalculator;
        let attributes = calculator.calculate(&beatmap, &[]).unwrap();

        assert_eq!(attributes.max_combo, 2);
        assert!((attributes.star_rating - 0.0594).abs() < 1e-14);
        assert!(attributes.mods.is_empty());
        assert_eq!(ManiaDifficultyCalculator::VERSION, 20241007);
    }

    #[test]
    fn calculator_uses_realized_bezier_duration_without_declared_length() {
        let beatmap = OsuBeatmap::parse_bytes(
            b"osu file format v14\n[General]\nMode:3\n[Difficulty]\nCircleSize:4\n[HitObjects]\n64,192,0,2,0,B|128:192|256:192,1,0\n",
        )
        .unwrap();
        let result = ManiaDifficultyCalculator.calculate(&beatmap, &[]).unwrap();
        assert_eq!(result.max_combo, 14);
    }

    #[test]
    fn official_mania_fixture_matches_lazer_star_and_max_combo_golden() {
        let beatmap =
            OsuBeatmap::parse_bytes(include_bytes!("../tests/fixtures/mania-diffcalc-test.osu"))
                .unwrap();
        for (mods, expected) in [
            ([].as_slice(), 2.3493769750220914_f64),
            ([Mod::DoubleTime].as_slice(), 2.797245912537965),
        ] {
            let attributes = ManiaDifficultyCalculator.calculate(&beatmap, mods).unwrap();
            assert_eq!(attributes.max_combo, 242);
            let relative_error = (attributes.star_rating - expected).abs() / expected.abs();
            assert!(
                relative_error <= 2e-5,
                "mods {mods:?}: expected {expected}, got {} ({relative_error:e} relative error)",
                attributes.star_rating
            );
        }
    }

    #[test]
    fn empty_and_single_note_charts_have_zero_stars() {
        assert_eq!(
            calculate(&[], 4, 1.0),
            ManiaDifficultyAttributes {
                star_rating: 0.0,
                max_combo: 0,
                ..Default::default()
            }
        );
        assert_eq!(
            calculate(&[note(100.0, 100.0, 0)], 4, 1.0),
            ManiaDifficultyAttributes {
                star_rating: 0.0,
                max_combo: 1,
                ..Default::default()
            }
        );
    }

    #[test]
    fn same_time_two_note_chart_matches_strain_baseline() {
        // The first playable object is only a time predecessor; the second contributes individual 2 + overall 2.
        let attributes = calculate(&[note(0.0, 0.0, 0), note(0.0, 0.0, 1)], 4, 1.0);
        assert_eq!(attributes.star_rating, 4.0 * DIFFICULTY_MULTIPLIER);
        assert_eq!(attributes.max_combo, 2);
    }

    #[test]
    fn source_order_is_stably_sorted_by_rounded_start_time() {
        let ordered = [
            note(100.0, 100.0, 0),
            note(200.0, 200.0, 1),
            note(300.0, 300.0, 2),
        ];
        let shuffled = [ordered[2], ordered[0], ordered[1]];
        assert_eq!(
            calculate(&ordered, 4, 1.0).star_rating,
            calculate(&shuffled, 4, 1.0).star_rating
        );
    }

    #[test]
    fn first_playable_hold_only_sets_delta_baseline() {
        // ManiaDifficultyCalculator skips the first playable object when creating difficulty objects, so its hold
        // does not enter per-column overlap history for the second note.
        let attributes = calculate(&[note(0.0, 500.0, 0), note(100.0, 100.0, 1)], 4, 1.0);
        let expected = (2.0 + 1.0 + decay(OVERALL_DECAY, 100.0)) * DIFFICULTY_MULTIPLIER;
        assert!((attributes.star_rating - expected).abs() < 1e-14);
    }

    #[test]
    fn max_combo_uses_truncated_hundred_millisecond_hold_ticks() {
        let attributes = calculate(
            &[
                note(0.0, 0.0, 0),
                note(100.0, 199.999, 1), // 1 + trunc(99.999 / 100) = 1
                note(200.0, 400.0, 2),   // 1 + trunc(200 / 100) = 3
            ],
            4,
            1.0,
        );
        assert_eq!(attributes.max_combo, 5);
    }

    #[test]
    fn active_hold_increases_individual_and_overall_strain() {
        let current = DifficultyObject {
            start_time_ms: 100.0,
            end_time_ms: 200.0,
            delta_time_ms: 100.0,
            previous_start_time_ms: 0.0,
            column_strain_time_ms: 100.0,
            column: 1,
            previous_by_column: vec![
                Some(NoteWindow {
                    start_time_ms: 0.0,
                    end_time_ms: 500.0,
                }),
                None,
            ],
        };

        assert_eq!(evaluate_individual_strain(&current), 2.5);
        assert_eq!(evaluate_overall_strain(&current), 1.25);
    }

    #[test]
    fn overlap_release_bonus_uses_the_thirty_millisecond_logistic_midpoint() {
        let current = DifficultyObject {
            start_time_ms: 50.0,
            end_time_ms: 130.0,
            delta_time_ms: 50.0,
            previous_start_time_ms: 0.0,
            column_strain_time_ms: 50.0,
            column: 1,
            previous_by_column: vec![
                Some(NoteWindow {
                    start_time_ms: 0.0,
                    end_time_ms: 100.0,
                }),
                None,
            ],
        };

        let expected = 1.0 + 1.0 / (1.0 + (0.27 * (RELEASE_THRESHOLD_MS - 30.0)).exp());
        assert_eq!(evaluate_overall_strain(&current), expected);
    }

    #[test]
    fn rate_adjustment_changes_time_decay() {
        let chart = [note(0.0, 0.0, 0), note(1000.0, 1000.0, 1)];
        let normal = calculate(&chart, 4, 1.0).star_rating;
        let double_time = calculate(&chart, 4, 2.0).star_rating;
        assert!(double_time > normal);
    }

    #[test]
    fn validates_column_time_duration_and_rate_boundaries() {
        assert_eq!(
            ManiaStrainCalculator::calculate(&[], 0, 1.0),
            Err(ManiaDifficultyError::InvalidColumnCount(0))
        );
        assert_eq!(
            ManiaStrainCalculator::calculate(&[], MAX_COLUMNS + 1, 1.0),
            Err(ManiaDifficultyError::InvalidColumnCount(MAX_COLUMNS + 1))
        );
        assert_eq!(
            ManiaStrainCalculator::calculate(&[note(0.0, 0.0, 4)], 4, 1.0),
            Err(ManiaDifficultyError::ColumnOutOfRange {
                column: 4,
                total_columns: 4
            })
        );
        assert_eq!(
            ManiaStrainCalculator::calculate(&[note(f64::NAN, 0.0, 0)], 4, 1.0),
            Err(ManiaDifficultyError::NonFiniteTime { note_index: 0 })
        );
        assert_eq!(
            ManiaStrainCalculator::calculate(&[note(10.0, 9.0, 0)], 4, 1.0),
            Err(ManiaDifficultyError::EndBeforeStart { note_index: 0 })
        );
        assert_eq!(
            ManiaStrainCalculator::calculate(&[], 4, 0.0),
            Err(ManiaDifficultyError::InvalidClockRate)
        );
        assert_eq!(
            ManiaStrainCalculator::calculate(&[], 4, f64::INFINITY),
            Err(ManiaDifficultyError::InvalidClockRate)
        );
    }
}
