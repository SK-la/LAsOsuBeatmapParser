//! Official osu!taiko score PP calculation.
//!
//! Difficulty attributes are produced by the matching Taiko difficulty calculator. The caller supplies the
//! score's original beatmap OverallDifficulty; difficulty mods are applied here as TaikoPerformanceCalculator
//! does from ScoreInfo.BeatmapInfo.Difficulty. This module consumes the fixed-layout judgement statistics.

use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::attributes::{TaikoDifficultyAttributes, TaikoPerformanceAttributes};
use crate::beatmap::GameMode;
use crate::calculator::{CalculationError, PerformanceCalculator};
use crate::mods::{Mod, calculate_rate_with_mods};
use crate::score::{HitResult, ScoreInfo};

const SQRT_2: f64 = std::f64::consts::SQRT_2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TaikoPerformanceError {
    InvalidRate(f64),
    NonFiniteOverallDifficulty,
}

impl Display for TaikoPerformanceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRate(rate) => write!(f, "invalid Taiko performance playback rate {rate}"),
            Self::NonFiniteOverallDifficulty => {
                f.write_str("Taiko beatmap overall difficulty must be finite")
            }
        }
    }
}

impl Error for TaikoPerformanceError {}

/// Score PP calculator matching the current C# TaikoPerformanceCalculator.
pub struct TaikoPerformanceCalculator;

impl TaikoPerformanceCalculator {
    pub fn calculate_score(
        score: &ScoreInfo,
        attributes: &TaikoDifficultyAttributes,
    ) -> Result<TaikoPerformanceAttributes, CalculationError> {
        let overall_difficulty = score
            .beatmap_difficulty
            .as_ref()
            .and_then(|difficulty| difficulty.overall_difficulty)
            .unwrap_or(5.0);
        let source_mode = score.ruleset.ok_or_else(|| {
            CalculationError::InvalidBeatmap(
                "Taiko performance requires a source ruleset".to_owned(),
            )
        })?;
        Self::calculate(
            score,
            attributes,
            overall_difficulty,
            source_mode != GameMode::Taiko,
        )
        .map_err(|error| CalculationError::InvalidBeatmap(error.to_string()))
    }

    /// Calculate a score's Taiko PP and its difficulty / accuracy breakdown.
    ///
    /// base_overall_difficulty is the value stored on the score's beatmap before score mods. is_convert is true
    /// when the score's source ruleset was not Taiko; this affects hidden and finger-count bonuses.
    pub fn calculate(
        score: &ScoreInfo,
        attributes: &TaikoDifficultyAttributes,
        base_overall_difficulty: f64,
        is_convert: bool,
    ) -> Result<TaikoPerformanceAttributes, TaikoPerformanceError> {
        if !base_overall_difficulty.is_finite() {
            return Err(TaikoPerformanceError::NonFiniteOverallDifficulty);
        }

        let clock_rate = calculate_rate_with_mods(&score.mods).map_err(|_| {
            let rate = score
                .mods
                .iter()
                .filter_map(Mod::rate_multiplier)
                .fold(1.0, |current, multiplier| current * multiplier);
            TaikoPerformanceError::InvalidRate(rate)
        })?;

        let count_great = score.statistics.get(HitResult::Great);
        let count_ok = score.statistics.get(HitResult::Ok);
        let count_meh = score.statistics.get(HitResult::Meh);
        let count_miss = score.statistics.get(HitResult::Miss).max(0);
        let total_hits = count_great + count_ok + count_meh + count_miss;

        let overall_difficulty =
            apply_overall_difficulty_mods(base_overall_difficulty, &score.mods);
        let great_hit_window = great_window(overall_difficulty) / clock_rate;
        let estimated_unstable_rate = if count_great == 0 || great_hit_window <= 0.0 {
            None
        } else {
            Some(
                deviation_upper_bound(
                    total_hits as f64,
                    count_great as f64 / total_hits as f64,
                    great_hit_window,
                ) * 10.0,
            )
        };

        let total_difficult_hits = total_hits as f64 * attributes.consistency_factor;
        let has_hidden = score
            .mods
            .iter()
            .any(|modifier| matches!(modifier, Mod::Hidden));
        let has_flashlight = score
            .mods
            .iter()
            .any(|modifier| matches!(modifier, Mod::Flashlight));
        let has_easy = score
            .mods
            .iter()
            .any(|modifier| matches!(modifier, Mod::Easy));
        let is_classic = score
            .mods
            .iter()
            .any(|modifier| matches!(modifier, Mod::Classic));

        let difficulty = difficulty_value(
            attributes,
            estimated_unstable_rate,
            great_hit_window,
            total_difficult_hits,
            total_hits,
            count_miss,
            has_hidden,
            has_flashlight,
            has_easy,
            is_classic,
            is_convert,
        ) * 1.08;
        let accuracy = accuracy_value(
            attributes,
            estimated_unstable_rate,
            great_hit_window,
            total_difficult_hits,
            total_hits,
            has_hidden,
            has_flashlight,
            is_convert,
        ) * 1.1;

        Ok(TaikoPerformanceAttributes {
            total: difficulty + accuracy,
            difficulty,
            accuracy,
            estimated_unstable_rate,
        })
    }
}

impl PerformanceCalculator for TaikoPerformanceCalculator {
    type Difficulty = TaikoDifficultyAttributes;
    type Attributes = TaikoPerformanceAttributes;

    fn calculate(
        &self,
        score: &ScoreInfo,
        difficulty: &Self::Difficulty,
    ) -> Result<Self::Attributes, CalculationError> {
        Self::calculate_score(score, difficulty)
    }
}

fn apply_overall_difficulty_mods(base: f64, mods: &[Mod]) -> f64 {
    let mut overall_difficulty = base as f32;
    for modifier in mods {
        match modifier {
            Mod::Easy => overall_difficulty *= 0.5_f32,
            Mod::HardRock => overall_difficulty = (overall_difficulty * 1.4_f32).min(10.0_f32),
            Mod::DifficultyAdjust(settings) => {
                if let Some(value) = settings.overall_difficulty {
                    overall_difficulty = value as f32;
                }
            }
            _ => {}
        }
    }
    overall_difficulty as f64
}

fn great_window(overall_difficulty: f64) -> f64 {
    difficulty_range(overall_difficulty, 50.0, 35.0, 20.0).floor() - 0.5
}

fn difficulty_range(value: f64, min: f64, middle: f64, max: f64) -> f64 {
    if value > 5.0 {
        middle + (max - middle) * ((value - 5.0) / 5.0)
    } else if value < 5.0 {
        middle + (middle - min) * ((value - 5.0) / 5.0)
    } else {
        middle
    }
}

fn deviation_upper_bound(total_hits: f64, accuracy: f64, great_hit_window: f64) -> f64 {
    const Z: f64 = 2.326_347_874_04;
    let n = total_hits;
    let p = accuracy;
    let p_lower_bound = (n * p + Z * Z / 2.0) / (n + Z * Z)
        - Z / (n + Z * Z) * (n * p * (1.0 - p) + Z * Z / 4.0).sqrt();
    great_hit_window / (SQRT_2 * erf_inv(p_lower_bound))
}

fn difficulty_value(
    attributes: &TaikoDifficultyAttributes,
    estimated_unstable_rate: Option<f64>,
    great_hit_window: f64,
    total_difficult_hits: f64,
    total_hits: i32,
    count_miss: i32,
    has_hidden: bool,
    has_flashlight: bool,
    has_easy: bool,
    is_classic: bool,
    is_convert: bool,
) -> f64 {
    let Some(unstable_rate) = estimated_unstable_rate else {
        return 0.0;
    };
    if total_difficult_hits == 0.0 {
        return 0.0;
    }

    let expected_unstable_rate =
        deviation_upper_bound(total_hits as f64, 1.0, great_hit_window) * 10.0;
    let maximum_unstable_rate =
        deviation_upper_bound(total_hits as f64, 0.8, great_hit_window) * 10.0;
    let rhythm_factor = reverse_lerp(
        attributes.rhythm_difficulty / attributes.star_rating,
        0.15,
        0.4,
    );
    let rhythm_penalty = 1.0
        - logistic(
            unstable_rate,
            (expected_unstable_rate + maximum_unstable_rate) / 2.0,
            10.0 / (maximum_unstable_rate - expected_unstable_rate),
            0.25 * rhythm_factor.powi(3),
        );

    let base_difficulty =
        5.0 * csharp_max(attributes.star_rating * rhythm_penalty / 0.110, 1.0) - 4.0;
    let mut value = csharp_min(
        base_difficulty.powi(3) / 69_052.51,
        base_difficulty.powf(2.25) / 1250.0,
    );
    value *= 1.0 + 0.10 * csharp_max(attributes.star_rating - 10.0, 0.0);

    let length_bonus = 1.0 + 0.25 * total_difficult_hits / (total_difficult_hits + 4000.0);
    value *= length_bonus;
    let miss_penalty = 0.97 + 0.03 * total_difficult_hits / (total_difficult_hits + 1500.0);
    value *= miss_penalty.powi(count_miss);

    if has_hidden {
        let mut hidden_bonus: f64 = if is_convert { 0.025 } else { 0.1 };
        if !has_flashlight {
            if !is_classic {
                hidden_bonus *= 0.2;
            }
            if has_easy && is_classic {
                hidden_bonus *= 0.5;
            }
        }
        value *= 1.0 + hidden_bonus;
    }

    if has_flashlight {
        value *= csharp_max(
            1.050 - csharp_min(attributes.mono_stamina_factor / 50.0, 1.0) * length_bonus,
            1.0,
        );
    }

    let mono_accuracy_exponent = 2.0 + attributes.mono_stamina_factor;
    let mono_accuracy_shift = 500.0 - 100.0 * (attributes.mono_stamina_factor * 3.0);
    value * erf(mono_accuracy_shift / (SQRT_2 * unstable_rate)).powf(mono_accuracy_exponent)
}

fn accuracy_value(
    attributes: &TaikoDifficultyAttributes,
    estimated_unstable_rate: Option<f64>,
    great_hit_window: f64,
    total_difficult_hits: f64,
    total_hits: i32,
    has_hidden: bool,
    has_flashlight: bool,
    is_convert: bool,
) -> f64 {
    let Some(unstable_rate) = estimated_unstable_rate else {
        return 0.0;
    };
    if great_hit_window <= 0.0 {
        return 0.0;
    }

    let mut value = 470.0 * 0.9885_f64.powf(unstable_rate);
    value *= 1.0 + (50.0 / unstable_rate).powi(2) * attributes.star_rating.powf(2.8) / 600.0;
    if has_hidden && !is_convert {
        value *= 1.075;
    }
    value *= 1.0 + 0.3 * total_difficult_hits / (total_difficult_hits + 4000.0);
    let memory_length_bonus = csharp_min((total_hits as f64 / 1500.0).powf(0.3), 1.15);
    if has_flashlight && has_hidden && !is_convert {
        value *= csharp_max(1.05 * memory_length_bonus, 1.0);
    }
    value
}

fn logistic(x: f64, midpoint_offset: f64, multiplier: f64, max_value: f64) -> f64 {
    max_value / (1.0 + (multiplier * (midpoint_offset - x)).exp())
}

fn reverse_lerp(value: f64, start: f64, end: f64) -> f64 {
    ((value - start) / (end - start)).clamp(0.0, 1.0)
}

fn csharp_max(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        f64::NAN
    } else {
        left.max(right)
    }
}

fn csharp_min(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        f64::NAN
    } else {
        left.min(right)
    }
}

fn erf(x: f64) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    if x == f64::INFINITY {
        return 1.0;
    }
    if x == f64::NEG_INFINITY {
        return -1.0;
    }
    if x.is_nan() {
        return f64::NAN;
    }
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let tau = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let value = 1.0 - tau * (-x * x).exp();
    if x >= 0.0 { value } else { -value }
}

fn erf_inv(value: f64) -> f64 {
    if value <= -1.0 {
        return f64::NEG_INFINITY;
    }
    if value >= 1.0 {
        return f64::INFINITY;
    }
    if value == 0.0 {
        return 0.0;
    }
    const A: f64 = 0.147;
    let sign = value.signum();
    let x = value.abs();
    let ln = (1.0 - x * x).ln();
    let t1 = 2.0 / (std::f64::consts::PI * A) + ln / 2.0;
    let t2 = ln / A;
    let base = (t1 * t1 - t2).sqrt() - t1;
    let correction = if x >= 0.85 {
        ((x - 0.85) / 0.293).powi(8)
    } else {
        0.0
    };
    sign * (base.sqrt() + correction)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::DifficultyAdjustment;

    fn difficulty(
        star: f64,
        max_combo: i32,
        consistency: f64,
        mono_factor: f64,
        rhythm: f64,
    ) -> TaikoDifficultyAttributes {
        TaikoDifficultyAttributes {
            star_rating: star,
            max_combo,
            consistency_factor: consistency,
            mono_stamina_factor: mono_factor,
            rhythm_difficulty: rhythm,
            ..Default::default()
        }
    }

    fn score(great: i32, ok: i32, meh: i32, miss: i32, mods: Vec<Mod>) -> ScoreInfo {
        let mut score = ScoreInfo {
            mods,
            ..Default::default()
        };
        score.statistics.set(HitResult::Great, great);
        score.statistics.set(HitResult::Ok, ok);
        score.statistics.set(HitResult::Meh, meh);
        score.statistics.set(HitResult::Miss, miss);
        score
    }

    #[test]
    fn no_great_judgements_disable_both_pp_components_and_ur() {
        let result = TaikoPerformanceCalculator::calculate(
            &score(0, 3, 2, 1, vec![]),
            &difficulty(5.0, 100, 0.8, 1.0, 2.0),
            5.0,
            false,
        )
        .unwrap();
        assert_eq!(result.estimated_unstable_rate, None);
        assert_eq!(result.difficulty, 0.0);
        assert_eq!(result.accuracy, 0.0);
        assert_eq!(result.total, 0.0);
    }

    #[test]
    fn one_great_reference_deviation_and_accuracy_are_deterministic() {
        let result = TaikoPerformanceCalculator::calculate(
            &score(1, 0, 0, 0, vec![]),
            &difficulty(5.0, 1, 1.0, 0.0, 0.0),
            5.0,
            false,
        )
        .unwrap();
        let p_lower = (1.0 + 2.326_347_874_04_f64.powi(2) / 2.0)
            / (1.0 + 2.326_347_874_04_f64.powi(2))
            - 2.326_347_874_04 / (1.0 + 2.326_347_874_04_f64.powi(2))
                * (2.326_347_874_04_f64.powi(2) / 4.0).sqrt();
        let expected_ur = 34.5 / (SQRT_2 * erf_inv(p_lower)) * 10.0;
        assert!((result.estimated_unstable_rate.unwrap() - expected_ur).abs() < 1e-12);
        let accuracy_base = 470.0 * 0.9885_f64.powf(expected_ur);
        let accuracy_star_bonus = 1.0 + (50.0 / expected_ur).powi(2) * 5.0_f64.powf(2.8) / 600.0;
        let accuracy_length_bonus = 1.0 + 0.3 / 4001.0;
        let expected_accuracy = accuracy_base * accuracy_star_bonus * accuracy_length_bonus * 1.1;
        assert!((result.accuracy - expected_accuracy).abs() < 1e-10);
        let base_difficulty = 5.0_f64 * (5.0_f64 / 0.110).max(1.0) - 4.0;
        let pre_mono_difficulty = (base_difficulty.powi(3) / 69_052.51)
            .min(base_difficulty.powf(2.25) / 1250.0)
            * (1.0 + 0.25 / 4001.0);
        let mono_accuracy = erf(500.0 / (SQRT_2 * expected_ur)).powi(2);
        let expected_difficulty = pre_mono_difficulty * mono_accuracy * 1.08;
        assert!((result.difficulty - expected_difficulty).abs() < 1e-10);
        assert!((result.total - result.accuracy - result.difficulty).abs() < 1e-12);
        assert!(
            (result.total - 8.387_754_144_954_819).abs() < 1e-9,
            "unexpected one-Great Taiko PP: {}",
            result.total
        );
    }

    #[test]
    fn tick_statistics_are_ignored_and_negative_misses_are_clamped() {
        let mut with_unrelated_stats = score(8, 2, 1, -3, vec![]);
        with_unrelated_stats
            .statistics
            .set(HitResult::LargeTickMiss, 99);
        let attributes = difficulty(4.0, 50, 0.5, 1.0, 1.0);
        let actual =
            TaikoPerformanceCalculator::calculate(&with_unrelated_stats, &attributes, 5.0, false)
                .unwrap();
        let expected = TaikoPerformanceCalculator::calculate(
            &score(8, 2, 1, 0, vec![]),
            &attributes,
            5.0,
            false,
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn easy_hardrock_difficulty_adjust_and_clock_rate_change_the_ur() {
        let base_score = score(16, 2, 1, 1, vec![]);
        let attributes = difficulty(5.0, 20, 0.8, 1.0, 2.0);
        let normal =
            TaikoPerformanceCalculator::calculate(&base_score, &attributes, 5.0, false).unwrap();
        let easy = TaikoPerformanceCalculator::calculate(
            &score(16, 2, 1, 1, vec![Mod::Easy]),
            &attributes,
            5.0,
            false,
        )
        .unwrap();
        let hardrock = TaikoPerformanceCalculator::calculate(
            &score(16, 2, 1, 1, vec![Mod::HardRock]),
            &attributes,
            5.0,
            false,
        )
        .unwrap();
        let adjust = TaikoPerformanceCalculator::calculate(
            &score(
                16,
                2,
                1,
                1,
                vec![Mod::DifficultyAdjust(DifficultyAdjustment {
                    overall_difficulty: Some(7.3),
                    ..Default::default()
                })],
            ),
            &attributes,
            5.0,
            false,
        )
        .unwrap();
        let double_time = TaikoPerformanceCalculator::calculate(
            &score(16, 2, 1, 1, vec![Mod::DoubleTime]),
            &attributes,
            5.0,
            false,
        )
        .unwrap();
        assert_ne!(normal.estimated_unstable_rate, easy.estimated_unstable_rate);
        assert_ne!(
            normal.estimated_unstable_rate,
            hardrock.estimated_unstable_rate
        );
        assert_ne!(
            normal.estimated_unstable_rate,
            adjust.estimated_unstable_rate
        );
        assert_ne!(
            normal.estimated_unstable_rate,
            double_time.estimated_unstable_rate
        );
    }

    #[test]
    fn hidden_flashlight_classic_and_convert_bonus_gates() {
        let attributes = difficulty(8.0, 1000, 0.9, 4.0, 2.0);
        let base = TaikoPerformanceCalculator::calculate(
            &score(2000, 0, 0, 0, vec![]),
            &attributes,
            5.0,
            false,
        )
        .unwrap();
        let hidden = TaikoPerformanceCalculator::calculate(
            &score(2000, 0, 0, 0, vec![Mod::Hidden]),
            &attributes,
            5.0,
            false,
        )
        .unwrap();
        let hidden_classic = TaikoPerformanceCalculator::calculate(
            &score(2000, 0, 0, 0, vec![Mod::Hidden, Mod::Classic]),
            &attributes,
            5.0,
            false,
        )
        .unwrap();
        let hidden_flashlight = TaikoPerformanceCalculator::calculate(
            &score(2000, 0, 0, 0, vec![Mod::Hidden, Mod::Flashlight]),
            &attributes,
            5.0,
            false,
        )
        .unwrap();
        let converted_hidden = TaikoPerformanceCalculator::calculate(
            &score(2000, 0, 0, 0, vec![Mod::Hidden]),
            &attributes,
            5.0,
            true,
        )
        .unwrap();
        assert!(hidden.difficulty > base.difficulty);
        assert!(hidden_classic.difficulty > hidden.difficulty);
        assert!(hidden_flashlight.difficulty > hidden.difficulty);
        assert!(hidden.accuracy > base.accuracy);
        assert!(converted_hidden.difficulty < hidden.difficulty);
    }

    #[test]
    fn nofail_has_no_taiko_pp_penalty_and_one_miss_does() {
        let attributes = difficulty(5.0, 100, 1.0, 0.0, 1.0);
        let regular = TaikoPerformanceCalculator::calculate(
            &score(99, 0, 0, 1, vec![]),
            &attributes,
            5.0,
            false,
        )
        .unwrap();
        let nofail = TaikoPerformanceCalculator::calculate(
            &score(99, 0, 0, 1, vec![Mod::NoFail]),
            &attributes,
            5.0,
            false,
        )
        .unwrap();
        let perfect = TaikoPerformanceCalculator::calculate(
            &score(100, 0, 0, 0, vec![]),
            &attributes,
            5.0,
            false,
        )
        .unwrap();
        assert_eq!(regular, nofail);
        assert!(regular.total < perfect.total);
    }

    #[test]
    fn invalid_rate_and_overall_difficulty_are_reported() {
        assert!(matches!(
            TaikoPerformanceCalculator::calculate(
                &score(1, 0, 0, 0, vec![Mod::RateAdjust { rate: 0.0 }]),
                &difficulty(1.0, 1, 1.0, 0.0, 0.0),
                5.0,
                false
            ),
            Err(TaikoPerformanceError::InvalidRate(_))
        ));
        assert_eq!(
            TaikoPerformanceCalculator::calculate(
                &score(1, 0, 0, 0, vec![]),
                &difficulty(1.0, 1, 1.0, 0.0, 0.0),
                f64::NAN,
                false
            ),
            Err(TaikoPerformanceError::NonFiniteOverallDifficulty)
        );
    }

    #[test]
    fn erf_pair_matches_lazer_approximation_reference() {
        assert!((erf(1.0) - 0.842_700_689_747_589_9).abs() < 1e-15);
        assert!((erf_inv(0.842_700_689_747_589_9) - 1.0).abs() < 0.002);
        assert_eq!(erf_inv(1.0), f64::INFINITY);
        assert_eq!(erf_inv(-1.0), f64::NEG_INFINITY);
    }
}
