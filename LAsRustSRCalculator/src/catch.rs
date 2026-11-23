//! Pure osu!catch movement difficulty calculation.
//!
//! This is the post-conversion core only. The caller must supply palpable objects in the form consumed by
//! `CatchBeatmap.GetPalpableObjects`, exclude `Banana` and `TinyDroplet`, and provide the converter's effective X,
//! hyper-dash state, distance-to-hyper-dash, half catcher width, playback rate, and full beatmap max combo.
//! The first remaining palpable object is used only as the movement baseline. Converting standard hit objects,
//! generating Catch objects, and calculating hyper-dash metadata remain upstream responsibilities.

use crate::attributes::CatchDifficultyAttributes;
use crate::mods::Mod;

pub const NORMALIZED_HALF_CATCHER_WIDTH: f32 = 41.0;
const ABSOLUTE_PLAYER_POSITIONING_ERROR: f32 = 16.0;
const DIRECTION_CHANGE_BONUS: f64 = 21.0;
const DIFFICULTY_MULTIPLIER: f64 = 4.59;
const SECTION_LENGTH: f64 = 750.0;
const STRAIN_DECAY_BASE: f64 = 0.2;
const DECAY_WEIGHT: f64 = 0.94;

/// One already-converted, palpable Catch object used by the difficulty core.
///
/// The position and hyper-dash fields correspond to lazer's `PalpableCatchHitObject` after beatmap processing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CatchObjectInput {
    pub start_time_ms: f64,
    pub effective_x: f32,
    pub hyper_dash: bool,
    pub distance_to_hyper_dash: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CatchDifficultyError {
    InvalidHalfCatcherWidth(f32),
    InvalidClockRate(f64),
    InvalidMaxCombo(i32),
    NonFiniteObject { index: usize },
}

impl std::fmt::Display for CatchDifficultyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHalfCatcherWidth(value) => {
                write!(
                    f,
                    "half catcher width must be finite and positive, got {value}"
                )
            }
            Self::InvalidClockRate(value) => {
                write!(f, "clock rate must be finite and positive, got {value}")
            }
            Self::InvalidMaxCombo(value) => write!(f, "max combo cannot be negative, got {value}"),
            Self::NonFiniteObject { index } => {
                write!(
                    f,
                    "catch object at index {index} contains a non-finite value"
                )
            }
        }
    }
}

impl std::error::Error for CatchDifficultyError {}

/// Calculates Catch star rating from stable-sorted palpable objects.
///
/// `objects` should already exclude bananas and tiny droplets. The function stable-sorts by start time to match
/// `GetPalpableObjects`; input order is retained for equal-time objects. `half_catcher_width` must be the value
/// produced by Catch's beatmap difficulty conversion, including its circle-size adjustment.
pub fn calculate_catch_difficulty(
    objects: &[CatchObjectInput],
    half_catcher_width: f32,
    clock_rate: f64,
    max_combo: i32,
    mods: &[Mod],
) -> Result<CatchDifficultyAttributes, CatchDifficultyError> {
    if !half_catcher_width.is_finite() || half_catcher_width <= 0.0 {
        return Err(CatchDifficultyError::InvalidHalfCatcherWidth(
            half_catcher_width,
        ));
    }
    if !clock_rate.is_finite() || clock_rate <= 0.0 {
        return Err(CatchDifficultyError::InvalidClockRate(clock_rate));
    }
    if max_combo < 0 {
        return Err(CatchDifficultyError::InvalidMaxCombo(max_combo));
    }
    for (index, object) in objects.iter().enumerate() {
        if !object.start_time_ms.is_finite()
            || !object.effective_x.is_finite()
            || !object.distance_to_hyper_dash.is_finite()
        {
            return Err(CatchDifficultyError::NonFiniteObject { index });
        }
    }

    let mut ordered_objects = objects.to_vec();
    // Rust's slice sort is stable, matching LINQ OrderBy for equal start times.
    ordered_objects.sort_by(|a, b| a.start_time_ms.total_cmp(&b.start_time_ms));

    let difficulty =
        calculate_movement_difficulty(&ordered_objects, half_catcher_width, clock_rate);

    Ok(CatchDifficultyAttributes {
        mods: mods.to_vec(),
        star_rating: DIFFICULTY_MULTIPLIER * difficulty.sqrt(),
        max_combo,
    })
}

#[derive(Clone, Copy, Debug)]
struct MovementHitObject {
    start_time: f64,
    delta_time: f64,
    strain_time: f64,
    clock_rate: f64,
    last_hyper_dash: bool,
    last_distance_to_hyper_dash: f32,
    player_position: f32,
    distance_moved: f32,
    exact_distance_moved: f32,
}

fn calculate_movement_difficulty(
    objects: &[CatchObjectInput],
    half_catcher_width: f32,
    clock_rate: f64,
) -> f64 {
    weighted_peak_sum(&mut movement_section_peaks(
        objects,
        half_catcher_width,
        clock_rate,
    ))
}

fn movement_section_peaks(
    objects: &[CatchObjectInput],
    half_catcher_width: f32,
    clock_rate: f64,
) -> Vec<f64> {
    let hit_objects = preprocess_objects(objects, half_catcher_width, clock_rate);
    if hit_objects.is_empty() {
        return Vec::new();
    }

    let mut current_strain = 0.0;
    let mut current_section_peak = 0.0;
    let mut current_section_end = 0.0;
    let mut strain_peaks = Vec::new();

    for index in 0..hit_objects.len() {
        let current = hit_objects[index];
        if index == 0 {
            current_section_end = (current.start_time / SECTION_LENGTH).ceil() * SECTION_LENGTH;
        }

        while current.start_time > current_section_end {
            strain_peaks.push(current_section_peak);
            let previous_start_time = hit_objects[index - 1].start_time;
            current_section_peak =
                current_strain * decay(current_section_end - previous_start_time);
            current_section_end += SECTION_LENGTH;
        }

        current_strain *= decay(current.delta_time);
        current_strain += evaluate_movement(index, &hit_objects);
        current_section_peak = current_section_peak.max(current_strain);
    }

    strain_peaks.push(current_section_peak);
    strain_peaks
}

fn preprocess_objects(
    objects: &[CatchObjectInput],
    half_catcher_width: f32,
    clock_rate: f64,
) -> Vec<MovementHitObject> {
    if objects.len() < 2 {
        return Vec::new();
    }

    let scaling_factor = NORMALIZED_HALF_CATCHER_WIDTH / half_catcher_width;
    let normalized_positions: Vec<f32> = objects
        .iter()
        .map(|object| object.effective_x * scaling_factor)
        .collect();

    let mut hit_objects: Vec<MovementHitObject> = Vec::with_capacity(objects.len() - 1);
    for index in 0..objects.len() - 1 {
        let last = objects[index];
        let current = objects[index + 1];
        let delta_time = (current.start_time_ms - last.start_time_ms) / clock_rate;
        let strain_time = delta_time.max(40.0);
        let normalized_position = normalized_positions[index + 1];
        let last_normalized_position = normalized_positions[index];
        let last_player_position = if index == 0 {
            last_normalized_position
        } else {
            hit_objects[index - 1].player_position
        };

        let mut player_position = last_player_position.clamp(
            normalized_position
                - (NORMALIZED_HALF_CATCHER_WIDTH - ABSOLUTE_PLAYER_POSITIONING_ERROR),
            normalized_position
                + (NORMALIZED_HALF_CATCHER_WIDTH - ABSOLUTE_PLAYER_POSITIONING_ERROR),
        );
        let distance_moved = player_position - last_player_position;
        let exact_distance_moved = normalized_position - last_player_position;

        // The previous object's hyper-dash sets the post-catch player position after the current move is measured.
        if last.hyper_dash {
            player_position = normalized_position;
        }

        hit_objects.push(MovementHitObject {
            start_time: current.start_time_ms / clock_rate,
            delta_time,
            strain_time,
            clock_rate,
            last_hyper_dash: last.hyper_dash,
            last_distance_to_hyper_dash: last.distance_to_hyper_dash,
            player_position,
            distance_moved,
            exact_distance_moved,
        });
    }
    hit_objects
}

fn evaluate_movement(index: usize, objects: &[MovementHitObject]) -> f64 {
    let current = objects[index];
    let previous = index.checked_sub(1).map(|i| objects[i]);
    let previous_previous = index.checked_sub(2).map(|i| objects[i]);
    let catcher_speed_multiplier = current.clock_rate;
    let weighted_strain_time = current.strain_time + 13.0 + (3.0 / catcher_speed_multiplier);
    let absolute_distance = current.distance_moved.abs();
    let mut distance_addition = absolute_distance as f64;
    distance_addition = distance_addition.powf(1.3) / 510.0;
    let sqrt_strain = weighted_strain_time.sqrt();
    let mut edge_dash_bonus = 0.0;

    if absolute_distance > 0.1 {
        if let Some(last) = previous {
            let last_distance = last.distance_moved.abs();
            if last_distance > 0.1 && sign(current.distance_moved) != sign(last.distance_moved) {
                let bonus_factor = absolute_distance.min(50.0f32) / 50.0f32;
                let antiflow_factor = ((last_distance.min(70.0f32) / 70.0f32) as f64).max(0.38);
                let weighted_time_ratio = weighted_strain_time / 1000.0;
                let direction_bonus = DIRECTION_CHANGE_BONUS / (last.strain_time + 16.0).sqrt()
                    * bonus_factor as f64
                    * antiflow_factor
                    * (1.0 - pow_integer(weighted_time_ratio, 3)).max(0.0);
                distance_addition += direction_bonus;
            }
        }

        let base_move = 12.5 * (absolute_distance.min(NORMALIZED_HALF_CATCHER_WIDTH * 2.0) as f64)
            / (NORMALIZED_HALF_CATCHER_WIDTH * 6.0) as f64
            / sqrt_strain;
        distance_addition += base_move;
    }

    let mut linear_spacing_count = 0usize;
    for skip in 0..index.min(10) {
        let previous = objects[index - skip - 1];
        if sign(current.distance_moved) != sign(previous.distance_moved)
            || current.distance_moved == 0.0
            || previous.distance_moved == 0.0
        {
            break;
        }

        let current_spacing = (current.distance_moved as f64 / current.strain_time).abs();
        let previous_spacing = (previous.distance_moved as f64 / previous.strain_time).abs();
        let relative_difference = (current_spacing / previous_spacing - 1.0).abs();
        if relative_difference > 0.05 {
            break;
        }
        linear_spacing_count += 1;
    }

    distance_addition *= pow_seven_tenths(linear_spacing_count);

    if current.last_distance_to_hyper_dash <= 20.0 {
        if !current.last_hyper_dash {
            edge_dash_bonus += 5.7;
        }

        let edge_distance = (20.0 - current.last_distance_to_hyper_dash as f64) / 20.0;
        let edge_time = (current.strain_time * catcher_speed_multiplier).min(265.0) / 265.0;
        distance_addition *= 1.0 + edge_dash_bonus * edge_distance * edge_time.powf(1.5);
    }

    if let (Some(last), Some(last_last)) = (previous, previous_previous) {
        if current.exact_distance_moved.abs() <= NORMALIZED_HALF_CATCHER_WIDTH * 2.0
            && current.exact_distance_moved == -last.exact_distance_moved
            && last.exact_distance_moved == -last_last.exact_distance_moved
            && current.strain_time == last.strain_time
            && last.strain_time == last_last.strain_time
        {
            distance_addition = 0.0;
        }
    }

    distance_addition / weighted_strain_time
}

fn sign(value: f32) -> i8 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

fn decay(ms: f64) -> f64 {
    STRAIN_DECAY_BASE.powf(ms / 1000.0)
}

fn pow_integer(value: f64, exponent: i32) -> f64 {
    match exponent {
        0 => 1.0,
        1 => value,
        2 => value * value,
        3 => value * value * value,
        4 => value * value * value * value,
        5 => value * value * value * value * value,
        _ => value.powf(exponent as f64),
    }
}

fn pow_seven_tenths(exponent: usize) -> f64 {
    match exponent {
        0 => 1.0,
        1 => 0.7,
        2 => 0.7 * 0.7,
        3 => 0.7 * 0.7 * 0.7,
        4 => 0.7 * 0.7 * 0.7 * 0.7,
        5 => 0.7 * 0.7 * 0.7 * 0.7 * 0.7,
        _ => 0.7f64.powf(exponent as f64),
    }
}

fn weighted_peak_sum(peaks: &mut [f64]) -> f64 {
    peaks.sort_by(|a, b| b.total_cmp(a));
    let mut difficulty = 0.0;
    let mut weight = 1.0;
    for peak in peaks.iter().copied().filter(|peak| *peak > 0.0) {
        difficulty += peak * weight;
        weight *= DECAY_WEIGHT;
    }
    difficulty
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(time: f64, x: f32) -> CatchObjectInput {
        CatchObjectInput {
            start_time_ms: time,
            effective_x: x,
            hyper_dash: false,
            distance_to_hyper_dash: 100.0,
        }
    }

    fn calculate(objects: &[CatchObjectInput]) -> CatchDifficultyAttributes {
        calculate_catch_difficulty(objects, 41.0, 1.0, 123, &[Mod::HardRock]).unwrap()
    }

    #[test]
    fn empty_and_single_object_only_return_zero_stars() {
        let empty = calculate(&[]);
        assert_eq!(empty.star_rating, 0.0);
        assert_eq!(empty.max_combo, 123);
        assert_eq!(empty.mods, vec![Mod::HardRock]);

        let single = calculate(&[object(0.0, 100.0)]);
        assert_eq!(single.star_rating, 0.0);
    }

    #[test]
    fn one_movement_matches_hand_calculated_golden() {
        // Half width 41 makes normalized coordinates identical to the supplied X values.
        // The catcher may stop 25 units from the target, so moving from 0 to 41 requires 16 units.
        // strain = ((16^1.3 / 510) + (12.5 * 16 / (246 * sqrt(116)))) / 116.
        let attributes =
            calculate_catch_difficulty(&[object(0.0, 0.0), object(100.0, 41.0)], 41.0, 1.0, 2, &[])
                .unwrap();
        assert!((attributes.star_rating - 0.1637078986752814).abs() < 1e-14);
        assert_eq!(attributes.max_combo, 2);

        let scaled =
            calculate_catch_difficulty(&[object(0.0, 0.0), object(100.0, 82.0)], 82.0, 1.0, 2, &[])
                .unwrap();
        assert_eq!(attributes.star_rating, scaled.star_rating);
    }

    #[test]
    fn strain_time_floor_caps_very_short_intervals_at_40ms() {
        let fast = calculate(&[object(0.0, 0.0), object(10.0, 41.0)]);
        let floor = calculate(&[object(0.0, 0.0), object(40.0, 41.0)]);
        assert_eq!(fast.star_rating, floor.star_rating);

        let normal_rate =
            calculate_catch_difficulty(&[object(0.0, 0.0), object(100.0, 41.0)], 41.0, 1.0, 2, &[])
                .unwrap();
        let faster_rate =
            calculate_catch_difficulty(&[object(0.0, 0.0), object(100.0, 41.0)], 41.0, 2.0, 2, &[])
                .unwrap();
        assert!(faster_rate.star_rating > normal_rate.star_rating);
    }

    #[test]
    fn palpable_objects_are_stably_sorted_by_start_time() {
        let ordered = [object(0.0, 0.0), object(100.0, 41.0), object(200.0, 82.0)];
        let unsorted = [ordered[2], ordered[0], ordered[1]];
        assert_eq!(calculate(&ordered), calculate(&unsorted));
    }

    #[test]
    fn hyper_dash_places_player_at_previous_object_after_movement() {
        let mut first = object(0.0, 0.0);
        first.hyper_dash = true;
        let inputs = [first, object(100.0, 82.0), object(200.0, 123.0)];
        let with_dash = preprocess_objects(&inputs, 41.0, 1.0);
        let mut without_dash = inputs;
        without_dash[0].hyper_dash = false;
        let without_dash = preprocess_objects(&without_dash, 41.0, 1.0);
        assert_eq!(with_dash[0].player_position, 82.0);
        assert_eq!(with_dash[1].distance_moved, 16.0);
        assert_eq!(without_dash[1].distance_moved, 41.0);
    }

    #[test]
    fn edge_dash_and_linear_spacing_properties_hold() {
        let base = [object(0.0, 0.0), object(100.0, 41.0), object(200.0, 82.0)];
        let mut edge = base;
        edge[1].distance_to_hyper_dash = 0.0;
        let mut edge_hyper = edge;
        edge_hyper[1].hyper_dash = true;
        let base_stars = calculate(&base).star_rating;
        assert!(calculate(&edge).star_rating > base_stars);
        assert!(calculate(&edge_hyper).star_rating <= calculate(&edge).star_rating);

        let movement = |strain_time| MovementHitObject {
            start_time: 0.0,
            delta_time: strain_time,
            strain_time,
            clock_rate: 1.0,
            last_hyper_dash: false,
            last_distance_to_hyper_dash: 100.0,
            player_position: 0.0,
            distance_moved: 20.0,
            exact_distance_moved: 30.0,
        };
        let regular = [
            movement(100.0),
            movement(100.0),
            movement(100.0),
            movement(100.0),
        ];
        let irregular = [
            movement(200.0),
            movement(100.0),
            movement(100.0),
            movement(100.0),
        ];
        assert!(evaluate_movement(3, &regular) < evaluate_movement(3, &irregular));
    }

    #[test]
    fn section_peak_weights_decay_by_094() {
        let peaks = [10.0, 5.0, 0.0];
        assert_eq!(weighted_peak_sum(&mut peaks.clone()), 14.7);
    }

    #[test]
    fn section_boundaries_use_strict_greater_than_at_750ms() {
        let at_boundary = [
            object(0.0, 0.0),
            object(100.0, 41.0),
            object(750.0, 82.0),
            object(750.001, 123.0),
        ];
        let after_second_boundary = [
            object(0.0, 0.0),
            object(100.0, 41.0),
            object(1500.0, 82.0),
            object(1500.001, 123.0),
        ];
        assert_eq!(movement_section_peaks(&at_boundary, 41.0, 1.0).len(), 2);
        assert_eq!(
            movement_section_peaks(&after_second_boundary, 41.0, 1.0).len(),
            3
        );
    }

    #[test]
    fn invalid_configuration_is_rejected() {
        assert!(calculate_catch_difficulty(&[], 0.0, 1.0, 0, &[]).is_err());
        assert!(calculate_catch_difficulty(&[], 41.0, f64::NAN, 0, &[]).is_err());
        assert!(calculate_catch_difficulty(&[], 41.0, 1.0, -1, &[]).is_err());
        assert!(
            calculate_catch_difficulty(&[object(0.0, f32::INFINITY)], 41.0, 1.0, 0, &[]).is_err()
        );
    }
}
