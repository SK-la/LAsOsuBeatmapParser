//! osu!standard difficulty aggregation over converter/evaluator output.
//!
//! The converter-facing view contains the geometry-dependent evaluator outputs from a playable osu map.
//! Use the ruleset converter to derive evaluator inputs from a parsed beatmap.

use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::attributes::OsuDifficultyAttributes;
use crate::mods::Mod;

const SECTION_LENGTH: f64 = 400.0;
const MAX_SECTION_LENGTH: f64 = 400.0;
const SECTION_DECAY: f64 = 0.9;
const AIM_DECAY: f64 = 0.2;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OsuObjectDifficultyInput {
    pub start_time_ms: f64,
    pub previous_start_time_ms: f64,
    /// Unclamped rate-adjusted interval from the preceding object.
    pub delta_time_ms: Option<f64>,
    pub adjusted_delta_time_ms: f64,
    pub last_object_end_delta_time_ms: f64,
    pub overall_difficulty: f64,
    pub hit_window_great_ms: f64,
    pub preempt_ms: f64,
    pub small_circle_bonus: f64,
    pub is_slider: bool,
    pub is_spinner: bool,
    pub aim_snap_with_slider: f64,
    pub aim_snap_without_slider: f64,
    pub aim_agility: f64,
    pub aim_flow_with_slider: f64,
    pub aim_flow_without_slider: f64,
    pub speed_evaluator: f64,
    pub speed_rhythm_multiplier: f64,
    pub reading_evaluator: f64,
    pub flashlight_evaluator: f64,
    pub slider_travel_distance: f64,
    pub slider_travel_time_ms: f64,
    pub has_slider_geometry_context: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OsuBeatmapMetadata {
    pub hit_circle_count: i32,
    pub slider_count: i32,
    pub spinner_count: i32,
    pub max_combo: i32,
    pub nested_score_per_object: f64,
    pub legacy_score_base_multiplier: f64,
    pub maximum_legacy_combo_score: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OsuDifficultyError {
    InvalidClockRate,
    NonFiniteObjectFeature { object_index: usize },
    MissingSliderGeometry { object_index: usize },
    InvalidMetadata,
}

impl Display for OsuDifficultyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidClockRate => f.write_str("osu clock rate must be finite and positive"),
            Self::NonFiniteObjectFeature { object_index } => write!(
                f,
                "osu object {object_index} has a non-finite evaluator feature"
            ),
            Self::MissingSliderGeometry { object_index } => write!(
                f,
                "osu slider {object_index} has no nested geometry/evaluator context"
            ),
            Self::InvalidMetadata => {
                f.write_str("osu beatmap counts or score metadata are invalid")
            }
        }
    }
}

impl Error for OsuDifficultyError {}

/// Official osu!standard skill aggregation and difficulty-attribute calculator.
pub struct OsuDifficultyCalculator;

impl OsuDifficultyCalculator {
    pub const VERSION: u32 = 20260706;

    pub fn calculate(
        objects: &[OsuObjectDifficultyInput],
        metadata: OsuBeatmapMetadata,
        mods: &[Mod],
        clock_rate: f64,
    ) -> Result<OsuDifficultyAttributes, OsuDifficultyError> {
        if !clock_rate.is_finite() || clock_rate <= 0.0 {
            return Err(OsuDifficultyError::InvalidClockRate);
        }
        if metadata.hit_circle_count < 0
            || metadata.slider_count < 0
            || metadata.spinner_count < 0
            || metadata.max_combo < 0
            || !metadata.nested_score_per_object.is_finite()
            || !metadata.legacy_score_base_multiplier.is_finite()
            || !metadata.maximum_legacy_combo_score.is_finite()
        {
            return Err(OsuDifficultyError::InvalidMetadata);
        }
        for (index, object) in objects.iter().enumerate() {
            let values = [
                object.start_time_ms,
                object.previous_start_time_ms,
                object.adjusted_delta_time_ms,
                object.last_object_end_delta_time_ms,
                object.overall_difficulty,
                object.hit_window_great_ms,
                object.preempt_ms,
                object.small_circle_bonus,
                object.aim_snap_with_slider,
                object.aim_snap_without_slider,
                object.aim_agility,
                object.aim_flow_with_slider,
                object.aim_flow_without_slider,
                object.speed_evaluator,
                object.speed_rhythm_multiplier,
                object.reading_evaluator,
                object.flashlight_evaluator,
                object.slider_travel_distance,
                object.slider_travel_time_ms,
            ];
            if values.iter().any(|value| !value.is_finite()) {
                return Err(OsuDifficultyError::NonFiniteObjectFeature {
                    object_index: index,
                });
            }
            if object.is_slider && !object.has_slider_geometry_context {
                return Err(OsuDifficultyError::MissingSliderGeometry {
                    object_index: index,
                });
            }
            if index > 0 && object.start_time_ms < objects[index - 1].start_time_ms {
                return Err(OsuDifficultyError::NonFiniteObjectFeature {
                    object_index: index,
                });
            }
        }

        let relax = has_mod(mods, Mod::Relax);
        let autopilot = has_mod(mods, Mod::Autopilot);
        let touch = has_mod(mods, Mod::TouchDevice);
        let flashlight_enabled = has_mod(mods, Mod::Flashlight);

        let mut aim_all_strain = VariableStrain::new();
        let mut aim_no_slider_strain = VariableStrain::new();
        let mut aim_all_objects = Vec::with_capacity(objects.len());
        let mut aim_no_slider_objects = Vec::with_capacity(objects.len());
        let mut aim_slider_values = Vec::new();
        let mut aim_no_slider_slider_values = Vec::new();
        let mut speed_strain = 0.0;
        let mut speed_objects = Vec::with_capacity(objects.len());
        let mut speed_slider_values = Vec::new();
        let mut reading_strain = 0.0;
        let mut reading_objects = Vec::with_capacity(objects.len());
        let mut reduced_note_count = 0usize;
        let mut first_reading_time = None;
        let mut flashlight_strain = StrainSkill::new(0.15);

        for object in objects {
            let delta = object
                .delta_time_ms
                .unwrap_or(object.adjusted_delta_time_ms);
            let od_factor = 0.985 + object.overall_difficulty.max(0.0).powi(2) / 4000.0;
            let aim_all = if autopilot {
                0.0
            } else {
                aim_difficulty(
                    object.aim_snap_with_slider,
                    object.aim_agility,
                    object.aim_flow_with_slider,
                    touch,
                    relax,
                ) * od_factor
            };
            let aim_no_slider = if autopilot {
                0.0
            } else {
                aim_difficulty(
                    object.aim_snap_without_slider,
                    object.aim_agility,
                    object.aim_flow_without_slider,
                    touch,
                    relax,
                ) * od_factor
            };
            let all_strain = aim_all_strain.process(
                object.start_time_ms,
                object.previous_start_time_ms,
                object.adjusted_delta_time_ms,
                aim_all,
            );
            let no_slider_strain = aim_no_slider_strain.process(
                object.start_time_ms,
                object.previous_start_time_ms,
                object.adjusted_delta_time_ms,
                aim_no_slider,
            );
            aim_all_objects.push(all_strain);
            aim_no_slider_objects.push(no_slider_strain);
            if object.is_slider {
                aim_slider_values.push(all_strain);
                aim_no_slider_slider_values.push(no_slider_strain);
            }

            let speed_decay = 0.3_f64.powf(object.adjusted_delta_time_ms / 1000.0);
            let speed_base = if relax {
                0.0
            } else {
                object.speed_evaluator * if autopilot { 0.5 } else { 1.0 }
            };
            speed_strain = speed_strain * speed_decay + speed_base * (1.0 - speed_decay) * 1.16;
            let speed = speed_strain * object.speed_rhythm_multiplier;
            speed_objects.push(speed);
            if object.is_slider {
                speed_slider_values.push(speed);
            }

            let reading_decay = 0.8_f64.powf(delta / 1000.0);
            let reading_base = if touch {
                object.reading_evaluator.powf(0.89)
            } else {
                object.reading_evaluator
            };
            let reading_mod = if relax {
                0.4
            } else if autopilot {
                0.1
            } else {
                1.0
            };
            let reading_base = reading_base
                * reading_mod
                * (0.825 + object.overall_difficulty.max(0.0).powf(2.2) / 1125.0);
            reading_strain =
                reading_strain * reading_decay + reading_base * (1.0 - reading_decay) * 2.5;
            reading_objects.push(reading_strain);
            let first_time = *first_reading_time.get_or_insert(object.start_time_ms);
            if object.start_time_ms <= first_time + 60_000.0 {
                reduced_note_count += 1;
            }

            let flashlight_decay = 0.15_f64.powf(delta / 1000.0);
            let flashlight_base = if touch {
                object.flashlight_evaluator.powf(0.9)
            } else {
                object.flashlight_evaluator
            };
            let flashlight_mod = if autopilot {
                0.4
            } else if relax {
                0.7
            } else {
                1.0
            };
            let flashlight_base = if flashlight_enabled {
                flashlight_base * flashlight_mod * od_factor
            } else {
                0.0
            };
            flashlight_strain.process(
                object.start_time_ms,
                object.previous_start_time_ms,
                flashlight_decay,
                flashlight_base,
            );
        }

        let aim_value = aim_all_strain.difficulty();
        let aim_without_value = aim_no_slider_strain.difficulty();
        let speed_value = harmonic(&speed_objects, 20.0, 0.9);
        let speed_weight_sum = harmonic_weight_sum(&speed_objects, 20.0, 0.9);
        let reading_transformed = transform_reading(&reading_objects, reduced_note_count);
        let reading_value = harmonic(&reading_transformed, 1.0, 0.9);
        let flashlight_value = flashlight_strain
            .difficulty(metadata.hit_circle_count + metadata.slider_count + metadata.spinner_count);
        let aim_difficulty = aim_value.powf(0.63) * 0.02275;
        let aim_without_rating = aim_without_value.powf(0.63) * 0.02275;
        let speed_difficulty = speed_value.sqrt() * 0.0675;
        let reading_difficulty = reading_value.sqrt() * 0.0675;
        let flashlight_difficulty = flashlight_enabled.then_some(flashlight_value.sqrt() * 0.0675);
        let aim_performance = 4.0 * aim_difficulty.powi(3);
        let speed_performance = 4.0 * speed_difficulty.powi(3);
        let reading_performance = 4.0 * reading_difficulty.powi(3);
        let flashlight_performance =
            flashlight_difficulty.map_or(0.0, |value| 25.0 * value.powi(2));
        let cognition = sum_cognition(reading_performance, flashlight_performance);
        let base_performance = norm(1.1, &[aim_performance, speed_performance, cognition]);
        let star_rating = (base_performance * 1.12).cbrt();
        let aim_difficult_strain_count = top_weighted_strains(&aim_all_objects, aim_value, 0.9);
        let speed_difficult_strain_count =
            top_weighted_objects(&speed_objects, speed_value, speed_weight_sum, 0.88, 10.0);
        let reading_weight_sum = harmonic_weight_sum(&reading_transformed, 1.0, 0.9);
        let reading_difficult_note_count = top_weighted_objects(
            &reading_objects,
            reading_value,
            reading_weight_sum,
            1.15,
            5.0,
        );
        let aim_no_slider_top_sliders =
            top_weighted_sliders(&aim_no_slider_slider_values, aim_without_value);
        let speed_top_sliders =
            top_weighted_harmonic_sliders(&speed_slider_values, speed_value, speed_weight_sum);
        let aim_difficult_slider_count = difficult_sliders(&aim_slider_values);
        let speed_note_count = relevant_object_count(&speed_objects);

        Ok(OsuDifficultyAttributes {
            mods: mods.to_vec(),
            star_rating,
            max_combo: metadata.max_combo,
            aim_difficulty,
            aim_difficult_slider_count,
            speed_difficulty,
            speed_note_count,
            flashlight_difficulty,
            reading_difficulty,
            slider_factor: if aim_difficulty > 0.0 {
                aim_without_rating / aim_difficulty
            } else {
                1.0
            },
            aim_top_weighted_slider_factor: aim_no_slider_top_sliders
                / (top_weighted_strains(&aim_no_slider_objects, aim_without_value, SECTION_DECAY)
                    - aim_no_slider_top_sliders)
                    .max(1.0),
            speed_top_weighted_slider_factor: speed_top_sliders
                / (speed_difficult_strain_count - speed_top_sliders).max(1.0),
            aim_difficult_strain_count,
            speed_difficult_strain_count,
            reading_difficult_note_count,
            nested_score_per_object: metadata.nested_score_per_object,
            legacy_score_base_multiplier: metadata.legacy_score_base_multiplier,
            maximum_legacy_combo_score: metadata.maximum_legacy_combo_score,
            hit_circle_count: metadata.hit_circle_count,
            slider_count: metadata.slider_count,
            spinner_count: metadata.spinner_count,
        })
    }
}

fn has_mod(mods: &[Mod], target: Mod) -> bool {
    mods.iter()
        .any(|modifier| std::mem::discriminant(modifier) == std::mem::discriminant(&target))
}

fn aim_difficulty(snap: f64, agility: f64, flow: f64, touch: bool, relax: bool) -> f64 {
    let snap = snap * 70.9;
    let agility = agility * 2.35;
    let flow = flow * 242.0;
    let combined = norm(1.2, &[snap, agility]);
    let ratio = flow / combined;
    let p_snap = if ratio == 0.0 {
        0.0
    } else if ratio.is_nan() {
        1.0
    } else {
        logistic(-7.27 * ratio.ln())
    };
    let combined = if touch {
        norm(1.2, &[snap.powf(0.89), agility])
    } else {
        combined
    };
    let combined = if relax { combined * 0.75 } else { combined };
    let flow = if relax { flow * 0.6 } else { flow };
    (combined * p_snap + flow * (1.0 - p_snap)) * 1.12
}

fn sum_cognition(reading: f64, flashlight: f64) -> f64 {
    if reading <= 0.0 {
        return flashlight;
    }
    if flashlight <= 0.0 {
        return reading;
    }
    norm(
        1.1,
        &[
            reading,
            flashlight * (flashlight / reading).clamp(0.25, 1.0),
        ],
    )
}

fn norm(power: f64, values: &[f64]) -> f64 {
    values
        .iter()
        .map(|value| value.powf(power))
        .sum::<f64>()
        .powf(1.0 / power)
}

fn logistic(exponent: f64) -> f64 {
    1.0 / (1.0 + exponent.exp())
}

fn harmonic(values: &[f64], scale: f64, decay: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| b.total_cmp(a));
    let mut output = 0.0;
    let mut index = 0usize;
    for value in sorted.into_iter().filter(|value| *value > 0.0) {
        let item_index = index as f64;
        let weight = (1.0 + scale / (1.0 + item_index))
            / (item_index.powf(decay) + 1.0 + scale / (1.0 + item_index));
        output += value * weight;
        index += 1;
    }
    output
}

fn harmonic_weight_sum(values: &[f64], scale: f64, decay: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| b.total_cmp(a));
    sorted
        .into_iter()
        .filter(|value| *value > 0.0)
        .enumerate()
        .map(|(index, _)| {
            let item_index = index as f64;
            (1.0 + scale / (1.0 + item_index))
                / (item_index.powf(decay) + 1.0 + scale / (1.0 + item_index))
        })
        .sum()
}

fn top_weighted_objects(
    values: &[f64],
    difficulty: f64,
    weight_sum: f64,
    midpoint: f64,
    multiplier: f64,
) -> f64 {
    if weight_sum == 0.0 {
        return 0.0;
    }
    let consistent = difficulty / weight_sum;
    values
        .iter()
        .map(|value| 1.1 * logistic(multiplier * (midpoint - value / consistent)))
        .sum()
}

fn top_weighted_sliders(values: &[f64], difficulty: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let consistent = difficulty * (1.0 - SECTION_DECAY);
    if consistent == 0.0 {
        return 0.0;
    }
    values
        .iter()
        .map(|value| 1.1 * logistic(10.0 * (0.88 - value / consistent)))
        .sum()
}

fn top_weighted_harmonic_sliders(values: &[f64], difficulty: f64, weight_sum: f64) -> f64 {
    if values.is_empty() || weight_sum == 0.0 {
        return 0.0;
    }
    let consistent = difficulty / weight_sum;
    if consistent == 0.0 {
        return 0.0;
    }
    values
        .iter()
        .map(|value| 1.1 * logistic(10.0 * (0.88 - value / consistent)))
        .sum()
}

fn top_weighted_strains(values: &[f64], difficulty: f64, decay: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let consistent = difficulty * (1.0 - decay);
    if consistent == 0.0 {
        return values.len() as f64;
    }
    values
        .iter()
        .map(|value| 1.1 * logistic(10.0 * (0.88 - value / consistent)))
        .sum()
}

fn relevant_object_count(values: &[f64]) -> f64 {
    let max = values.iter().copied().fold(0.0, f64::max);
    if max == 0.0 {
        return 0.0;
    }
    values
        .iter()
        .map(|value| logistic(12.0 * (0.5 - value / max)))
        .sum()
}

fn difficult_sliders(values: &[f64]) -> f64 {
    let max = values.iter().copied().fold(0.0, f64::max);
    if max == 0.0 {
        return 0.0;
    }
    values
        .iter()
        .map(|value| logistic(12.0 * (0.5 - value / max)))
        .sum()
}

fn transform_reading(values: &[f64], reduced_note_count: usize) -> Vec<f64> {
    let mut transformed: Vec<f64> = values
        .iter()
        .copied()
        .filter(|value| *value > 0.0)
        .collect();
    let count = transformed.len().min(reduced_note_count);
    for index in 0..count {
        let ratio = (index as f64 / reduced_note_count.max(1) as f64).clamp(0.0, 1.0);
        let scale = (1.0 + 9.0 * ratio).log10();
        transformed[index] *= scale;
    }
    transformed
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct StrainPeak {
    value: f64,
    section_length: f64,
}

struct VariableStrain {
    initialized: bool,
    begin: f64,
    end: f64,
    peak: f64,
    last_strain: f64,
    queued: Vec<(f64, f64)>,
    peaks: Vec<StrainPeak>,
    stored_length: f64,
}

impl VariableStrain {
    fn new() -> Self {
        Self {
            initialized: false,
            begin: 0.0,
            end: 0.0,
            peak: 0.0,
            last_strain: 0.0,
            queued: Vec::new(),
            peaks: Vec::new(),
            stored_length: 0.0,
        }
    }

    fn process(&mut self, time: f64, previous_time: f64, adjusted_delta: f64, input: f64) -> f64 {
        let decay = AIM_DECAY.powf(adjusted_delta / 1000.0);
        let strain = self.last_strain * decay + input * (1.0 - decay);
        if !self.initialized {
            self.initialized = true;
            self.begin = time;
            self.end = time + MAX_SECTION_LENGTH;
            self.peak = strain;
            self.last_strain = strain;
            return strain;
        }
        self.backfill(time, previous_time);
        if strain > self.peak {
            self.queued.clear();
            self.save_peak(time - self.begin);
            self.begin = time;
            self.end = time + MAX_SECTION_LENGTH;
            self.peak = strain;
        } else {
            while self
                .queued
                .last()
                .is_some_and(|(queued, _)| *queued < strain)
            {
                self.queued.pop();
            }
            self.queued.push((strain, time));
        }
        self.last_strain = strain;
        strain
    }

    fn backfill(&mut self, time: f64, previous_time: f64) {
        while time > self.end {
            self.save_peak(self.end - self.begin);
            self.begin = self.end;
            if !self.queued.is_empty() {
                let (queued, start) = self.queued.remove(0);
                self.end = start + MAX_SECTION_LENGTH;
                let initial = self.initial_strain(self.begin, previous_time);
                self.peak = initial.max(queued);
            } else {
                self.end = self.begin + MAX_SECTION_LENGTH;
                self.peak = self.initial_strain(self.begin, previous_time);
            }
        }
    }

    fn initial_strain(&self, time: f64, previous_time: f64) -> f64 {
        self.last_strain * AIM_DECAY.powf((time - previous_time) / 1000.0)
    }

    fn save_peak(&mut self, length: f64) {
        let peak = StrainPeak {
            value: self.peak,
            section_length: round_to_even(length),
        };
        let index = self
            .peaks
            .partition_point(|existing| existing.value > peak.value);
        self.peaks.insert(index, peak);
        self.stored_length += length;
        while self.stored_length > 11.0 / (1.0 - SECTION_DECAY) * MAX_SECTION_LENGTH {
            let Some(peak) = self.peaks.pop() else {
                break;
            };
            self.stored_length -= peak.section_length;
        }
    }

    fn difficulty(&self) -> f64 {
        let mut peaks = self.peaks.clone();
        peaks.push(StrainPeak {
            value: self.peak,
            section_length: round_to_even(self.end - self.begin),
        });
        peaks.retain(|peak| peak.value > 0.0);
        peaks.sort_by(|a, b| b.value.total_cmp(&a.value));

        // VariableLengthStrainSkill reduces the first four seconds of highest peaks into 20 ms pieces.
        let mut time = 0.0;
        let mut skip = 0usize;
        let mut pieces = Vec::new();
        while skip < peaks.len() && time < 4000.0 {
            let peak = peaks[skip];
            let mut elapsed = 0.0;
            while elapsed < peak.section_length {
                let section = (peak.section_length - elapsed).min(20.0);
                let scale = ((time + elapsed) / 4000.0).clamp(0.0, 1.0);
                let log = (1.0 + 9.0 * scale).log10();
                pieces.push(StrainPeak {
                    value: peak.value * (0.727 + 0.273 * log),
                    section_length: round_to_even(section),
                });
                elapsed += section;
            }
            time += peak.section_length;
            skip += 1;
        }
        pieces.extend_from_slice(&peaks[skip..]);
        pieces.sort_by(|a, b| b.value.total_cmp(&a.value));
        let mut accumulated = 0.0;
        let mut elapsed = 0.0;
        for peak in pieces {
            let start = elapsed;
            let end = elapsed + peak.section_length / MAX_SECTION_LENGTH;
            accumulated += peak.value * (SECTION_DECAY.powf(start) - SECTION_DECAY.powf(end));
            elapsed = end;
        }
        accumulated / (1.0 - SECTION_DECAY)
    }
}

fn round_to_even(value: f64) -> f64 {
    value.round_ties_even()
}

struct StrainSkill {
    decay: f64,
    strain: f64,
    section_end: f64,
    section_peak: f64,
    initialized: bool,
    peaks: Vec<f64>,
}
impl StrainSkill {
    fn new(decay: f64) -> Self {
        Self {
            decay,
            strain: 0.0,
            section_end: 0.0,
            section_peak: 0.0,
            initialized: false,
            peaks: Vec::new(),
        }
    }
    fn process(&mut self, time: f64, previous: f64, decay_factor: f64, input: f64) {
        if !self.initialized {
            self.initialized = true;
            self.section_end = (time / SECTION_LENGTH).ceil() * SECTION_LENGTH;
        }
        while time > self.section_end {
            self.peaks.push(self.section_peak);
            self.section_peak =
                self.strain * self.decay.powf((self.section_end - previous) / 1000.0);
            self.section_end += SECTION_LENGTH;
        }
        self.strain *= decay_factor;
        self.strain += input * 0.058;
        self.section_peak = self.section_peak.max(self.strain);
    }
    fn difficulty(&self, total_objects: i32) -> f64 {
        let mut peaks = self.peaks.clone();
        peaks.push(self.section_peak);
        let mut sum: f64 = peaks.into_iter().filter(|peak| *peak > 0.0).sum();
        sum *= 0.7
            + 0.1 * (total_objects as f64 / 200.0).min(1.0)
            + if total_objects > 200 {
                0.2 * ((total_objects - 200) as f64 / 200.0).min(1.0)
            } else {
                0.0
            };
        sum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_invalid_inputs_follow_calculator_contract() {
        let metadata = OsuBeatmapMetadata {
            max_combo: 1,
            ..Default::default()
        };
        let empty = OsuDifficultyCalculator::calculate(&[], metadata, &[], 1.0).unwrap();
        assert_eq!(empty.star_rating, 0.0);
        assert_eq!(empty.max_combo, 1);
        assert_eq!(
            OsuDifficultyCalculator::calculate(&[], metadata, &[], 0.0),
            Err(OsuDifficultyError::InvalidClockRate)
        );
    }

    #[test]
    fn one_feature_object_produces_finite_skill_axes() {
        let object = OsuObjectDifficultyInput {
            start_time_ms: 1000.0,
            previous_start_time_ms: 500.0,
            adjusted_delta_time_ms: 500.0,
            last_object_end_delta_time_ms: 500.0,
            overall_difficulty: 5.0,
            hit_window_great_ms: 49.5,
            preempt_ms: 1200.0,
            small_circle_bonus: 1.0,
            aim_snap_with_slider: 0.02,
            aim_snap_without_slider: 0.02,
            aim_agility: 0.02,
            aim_flow_with_slider: 0.001,
            aim_flow_without_slider: 0.001,
            speed_evaluator: 2.0,
            speed_rhythm_multiplier: 1.0,
            reading_evaluator: 0.2,
            flashlight_evaluator: 0.1,
            ..Default::default()
        };
        let result = OsuDifficultyCalculator::calculate(
            &[object],
            OsuBeatmapMetadata {
                hit_circle_count: 1,
                max_combo: 1,
                ..Default::default()
            },
            &[],
            1.0,
        )
        .unwrap();
        assert!(result.star_rating.is_finite() && result.star_rating > 0.0);
        assert!(result.aim_difficulty > 0.0);
        assert!(result.speed_difficulty > 0.0);
        assert_eq!(result.reading_difficulty, 0.0);
    }

    #[test]
    fn one_object_speed_only_golden_matches_lazer_skill_order() {
        // C# SpeedEvaluator at OD5, 1000ms interval and no next object: 1000/1000 * (1/(1-.3)) = 10/7.
        // Speed applies skill multiplier 1.16 and strain interpolation (1 - .3), yielding 1.16.
        let object = OsuObjectDifficultyInput {
            start_time_ms: 1000.0,
            previous_start_time_ms: 0.0,
            adjusted_delta_time_ms: 1000.0,
            last_object_end_delta_time_ms: 1000.0,
            overall_difficulty: 5.0,
            hit_window_great_ms: 99.0,
            preempt_ms: 1200.0,
            small_circle_bonus: 1.0,
            speed_evaluator: 10.0 / 7.0,
            speed_rhythm_multiplier: 1.0,
            ..Default::default()
        };
        let result = OsuDifficultyCalculator::calculate(
            &[object],
            OsuBeatmapMetadata {
                hit_circle_count: 2,
                max_combo: 2,
                ..Default::default()
            },
            &[],
            1.0,
        )
        .unwrap();
        let expected = 0.11984652300612501;
        assert!((result.star_rating - expected).abs() < 1e-14);
        assert!((result.speed_difficulty - 1.16_f64.sqrt() * 0.0675).abs() < 1e-14);
        assert_eq!(result.aim_difficulty, 0.0);
    }

    #[test]
    fn slider_without_nested_geometry_is_rejected() {
        let slider = OsuObjectDifficultyInput {
            is_slider: true,
            ..Default::default()
        };
        assert_eq!(
            OsuDifficultyCalculator::calculate(&[slider], OsuBeatmapMetadata::default(), &[], 1.0),
            Err(OsuDifficultyError::MissingSliderGeometry { object_index: 0 })
        );
    }
}
