//! Official osu!catch score performance calculation.
//!
//! The caller supplies the already-normalized beatmap difficulty from `ScoreInfo.BeatmapInfo.Difficulty`.
//! This mirrors the lazer score-dependent PP calculation and keeps score data separate from chart difficulty.

use crate::attributes::{CatchDifficultyAttributes, CatchPerformanceAttributes};
use crate::beatmap::DifficultySettings;
use crate::calculator::{CalculationError, PerformanceCalculator};
use crate::mods::{Mod, ModError, calculate_rate_with_mods};
use crate::score::{HitResult, ScoreInfo};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CatchPerformanceError {
    InvalidRate(f64),
    NonFiniteDifficulty,
}

impl std::fmt::Display for CatchPerformanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRate(rate) => write!(f, "invalid performance playback rate {rate}"),
            Self::NonFiniteDifficulty => f.write_str("beatmap difficulty must be finite"),
        }
    }
}

impl std::error::Error for CatchPerformanceError {}

impl From<ModError> for CatchPerformanceError {
    fn from(value: ModError) -> Self {
        match value {
            ModError::InvalidRate(rate) => Self::InvalidRate(rate),
        }
    }
}

/// Calculates PP for one Catch score using its beatmap difficulty and Catch difficulty attributes.
pub struct CatchPerformanceCalculator;

impl CatchPerformanceCalculator {
    pub fn calculate_score(
        score: &ScoreInfo,
        attributes: &CatchDifficultyAttributes,
    ) -> Result<CatchPerformanceAttributes, CalculationError> {
        let difficulty = score.beatmap_difficulty.as_ref().ok_or_else(|| {
            CalculationError::InvalidBeatmap(
                "Catch performance requires beatmap difficulty in ScoreInfo".to_owned(),
            )
        })?;
        Self::calculate(score, attributes, difficulty)
            .map_err(|error| CalculationError::InvalidBeatmap(error.to_string()))
    }

    pub fn calculate(
        score: &ScoreInfo,
        difficulty_attributes: &CatchDifficultyAttributes,
        base_difficulty: &DifficultySettings,
    ) -> Result<CatchPerformanceAttributes, CatchPerformanceError> {
        let mut num_300 = score.statistics.get(HitResult::Great);
        let mut num_100 = score.statistics.get(HitResult::LargeTickHit);
        let mut num_50 = score.statistics.get(HitResult::SmallTickHit);
        let mut num_katu = score.statistics.get(HitResult::SmallTickMiss);
        let num_miss = score
            .statistics
            .get(HitResult::Miss)
            .saturating_add(score.statistics.get(HitResult::LargeTickMiss))
            .max(0);

        num_300 = num_300.max(0);
        num_100 = num_100.max(0);
        num_50 = num_50.max(0);
        num_katu = num_katu.max(0);

        let mut approach_rate = difficulty_value(base_difficulty.approach_rate);

        if [
            difficulty_value(base_difficulty.hp_drain_rate),
            difficulty_value(base_difficulty.circle_size),
            difficulty_value(base_difficulty.overall_difficulty),
            approach_rate,
        ]
        .iter()
        .any(|value| !value.is_finite())
        {
            return Err(CatchPerformanceError::NonFiniteDifficulty);
        }

        for modifier in &score.mods {
            match modifier {
                Mod::Easy => {
                    approach_rate = ((approach_rate as f32) * 0.5_f32) as f64;
                }
                Mod::HardRock => {
                    approach_rate = (((approach_rate as f32) * 1.4_f32).min(10.0_f32)) as f64;
                }
                Mod::DifficultyAdjust(settings) => {
                    if let Some(value) = settings.approach_rate {
                        approach_rate = value as f32 as f64;
                    }
                }
                _ => {}
            }
        }

        let max_combo = difficulty_attributes.max_combo.max(0);
        let score_max_combo = score.max_combo.clamp(0, max_combo);

        let mut value = (5.0 * (difficulty_attributes.star_rating / 0.0049).max(1.0) - 4.0)
            .powf(2.0)
            / 100_000.0;

        let num_total_hits = num_50 + num_100 + num_300 + num_miss + num_katu;
        let num_combo_hits = num_miss + num_100 + num_300;
        let length_bonus = 0.95
            + 0.3 * (num_combo_hits as f64 / 2500.0).min(1.0)
            + if num_combo_hits > 2500 {
                (num_combo_hits as f64 / 2500.0).log10() * 0.475
            } else {
                0.0
            };
        value *= length_bonus;
        value *= 0.97_f64.powf(num_miss as f64);

        if max_combo > 0 {
            value *= ((score_max_combo as f64).powf(0.35) / (max_combo as f64).powf(0.35)).min(1.0);
        }

        let clock_rate = calculate_rate_with_mods(&score.mods)?;
        let preempt = difficulty_range(approach_rate, 1800.0, 1200.0, 450.0) / clock_rate;
        let effective_approach_rate = if preempt > 1200.0 {
            -(preempt - 1800.0) / 120.0
        } else {
            -(preempt - 1200.0) / 150.0 + 5.0
        };

        let mut approach_rate_factor = 1.0;
        if effective_approach_rate > 9.0 {
            approach_rate_factor += 0.1 * (effective_approach_rate - 9.0);
        }
        if effective_approach_rate > 10.0 {
            approach_rate_factor += 0.1 * (effective_approach_rate - 10.0);
        } else if effective_approach_rate < 8.0 {
            approach_rate_factor += 0.025 * (8.0 - effective_approach_rate);
        }
        value *= approach_rate_factor;

        if score
            .mods
            .iter()
            .any(|modifier| matches!(modifier, Mod::Hidden))
        {
            if effective_approach_rate <= 10.0 {
                value *= 1.05 + 0.075 * (10.0 - effective_approach_rate);
            } else {
                value *= 1.01 + 0.04 * (11.0 - effective_approach_rate.min(11.0));
            }
        }

        if score
            .mods
            .iter()
            .any(|modifier| matches!(modifier, Mod::Flashlight))
        {
            value *= 1.35 * length_bonus;
        }

        let accuracy = if num_total_hits == 0 {
            0.0
        } else {
            ((num_50 + num_100 + num_300) as f64 / num_total_hits as f64).clamp(0.0, 1.0)
        };
        value *= accuracy.powf(5.5);

        if score
            .mods
            .iter()
            .any(|modifier| matches!(modifier, Mod::NoFail))
        {
            value *= (1.0 - 0.02 * num_miss as f64).max(0.90);
        }

        Ok(CatchPerformanceAttributes { total: value })
    }
}

impl PerformanceCalculator for CatchPerformanceCalculator {
    type Difficulty = CatchDifficultyAttributes;
    type Attributes = CatchPerformanceAttributes;

    fn calculate(
        &self,
        score: &ScoreInfo,
        difficulty: &Self::Difficulty,
    ) -> Result<Self::Attributes, CalculationError> {
        Self::calculate_score(score, difficulty)
    }
}

fn difficulty_value(value: Option<f64>) -> f64 {
    value.unwrap_or(5.0) as f32 as f64
}

fn difficulty_range(value: f64, min: f64, mid: f64, max: f64) -> f64 {
    let normalized = (value - 5.0) / 5.0;
    if value > 5.0 {
        mid + (max - mid) * normalized
    } else if value < 5.0 {
        mid + (mid - min) * normalized
    } else {
        mid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attributes(star_rating: f64, max_combo: i32) -> CatchDifficultyAttributes {
        CatchDifficultyAttributes {
            star_rating,
            max_combo,
            ..Default::default()
        }
    }

    fn difficulty() -> DifficultySettings {
        DifficultySettings {
            hp_drain_rate: Some(5.0),
            circle_size: Some(5.0),
            overall_difficulty: Some(5.0),
            approach_rate: Some(5.0),
            slider_multiplier: Some(1.4),
            slider_tick_rate: Some(1.0),
        }
    }

    fn score(great: i32, combo: i32, mods: Vec<Mod>) -> ScoreInfo {
        let mut result = ScoreInfo {
            mods,
            max_combo: combo,
            ..Default::default()
        };
        result.statistics.set(HitResult::Great, great);
        result
    }

    #[test]
    fn one_perfect_catch_hit_matches_reference_formula_golden() {
        let result = CatchPerformanceCalculator::calculate(
            &score(1, 1, vec![]),
            &attributes(0.0049, 1),
            &difficulty(),
        )
        .unwrap();
        assert!((result.total - 0.000_010_213_79).abs() < 1e-16);
    }

    #[test]
    fn empty_and_failed_scores_have_no_accuracy_value() {
        let empty = CatchPerformanceCalculator::calculate(
            &ScoreInfo::default(),
            &attributes(4.0, 100),
            &difficulty(),
        )
        .unwrap();
        assert_eq!(empty.total, 0.0);

        let mut failed = ScoreInfo::default();
        failed.statistics.set(HitResult::Miss, 5);
        let result =
            CatchPerformanceCalculator::calculate(&failed, &attributes(4.0, 100), &difficulty())
                .unwrap();
        assert_eq!(result.total, 0.0);
    }

    #[test]
    fn catch_tick_judgements_map_into_legacy_result_counts() {
        let mut result = ScoreInfo {
            max_combo: 4,
            ..Default::default()
        };
        result.statistics.set(HitResult::Great, 1);
        result.statistics.set(HitResult::LargeTickHit, 1);
        result.statistics.set(HitResult::SmallTickHit, 1);
        result.statistics.set(HitResult::SmallTickMiss, 1);
        let result =
            CatchPerformanceCalculator::calculate(&result, &attributes(3.0, 4), &difficulty())
                .unwrap();
        assert!(result.total > 0.0);
        assert!(
            result.total
                < CatchPerformanceCalculator::calculate(
                    &score(4, 4, vec![]),
                    &attributes(3.0, 4),
                    &difficulty(),
                )
                .unwrap()
                .total
        );
    }

    #[test]
    fn miss_combo_hd_fl_and_nf_apply_reference_modifiers() {
        let perfect = CatchPerformanceCalculator::calculate(
            &score(100, 100, vec![]),
            &attributes(5.0, 100),
            &difficulty(),
        )
        .unwrap()
        .total;
        let miss = CatchPerformanceCalculator::calculate(
            &score(99, 99, vec![]),
            &attributes(5.0, 100),
            &difficulty(),
        )
        .unwrap()
        .total;
        let hd = CatchPerformanceCalculator::calculate(
            &score(100, 100, vec![Mod::Hidden]),
            &attributes(5.0, 100),
            &difficulty(),
        )
        .unwrap()
        .total;
        let fl = CatchPerformanceCalculator::calculate(
            &score(100, 100, vec![Mod::Flashlight]),
            &attributes(5.0, 100),
            &difficulty(),
        )
        .unwrap()
        .total;
        let mut failed_regular = score(99, 99, vec![]);
        failed_regular.statistics.set(HitResult::Miss, 1);
        let regular_failure = CatchPerformanceCalculator::calculate(
            &failed_regular,
            &attributes(5.0, 100),
            &difficulty(),
        )
        .unwrap()
        .total;
        failed_regular.mods.push(Mod::NoFail);
        let nf = CatchPerformanceCalculator::calculate(
            &failed_regular,
            &attributes(5.0, 100),
            &difficulty(),
        )
        .unwrap()
        .total;

        assert!(miss < perfect);
        assert!(hd > perfect);
        assert!(fl > perfect);
        assert!(nf < regular_failure);
    }

    #[test]
    fn rate_mod_changes_ar_used_for_pp() {
        let base = CatchPerformanceCalculator::calculate(
            &score(100, 100, vec![]),
            &attributes(5.0, 100),
            &difficulty(),
        )
        .unwrap()
        .total;
        let dt = CatchPerformanceCalculator::calculate(
            &score(100, 100, vec![Mod::DoubleTime]),
            &attributes(5.0, 100),
            &difficulty(),
        )
        .unwrap()
        .total;
        assert_ne!(base, dt);
    }

    #[test]
    fn invalid_rate_is_reported() {
        let error = CatchPerformanceCalculator::calculate(
            &score(1, 1, vec![Mod::RateAdjust { rate: 0.0 }]),
            &attributes(1.0, 1),
            &difficulty(),
        )
        .unwrap_err();
        assert!(matches!(error, CatchPerformanceError::InvalidRate(0.0)));
    }
}
