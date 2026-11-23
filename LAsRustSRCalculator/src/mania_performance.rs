//! Official osu!mania score performance calculation.

use crate::attributes::ManiaDifficultyAttributes;
use crate::attributes::ManiaPerformanceAttributes;
use crate::calculator::{CalculationError, PerformanceCalculator};
use crate::mods::Mod;
use crate::score::{HitResult, ScoreInfo};

/// Calculates mania score PP from score judgements and difficulty attributes.
pub struct ManiaPerformanceCalculator;

impl ManiaPerformanceCalculator {
    pub fn calculate(
        score: &ScoreInfo,
        attributes: &ManiaDifficultyAttributes,
    ) -> ManiaPerformanceAttributes {
        let count_perfect = score.statistics.get(HitResult::Perfect);
        let count_great = score.statistics.get(HitResult::Great);
        let count_good = score.statistics.get(HitResult::Good);
        let count_ok = score.statistics.get(HitResult::Ok);
        let count_meh = score.statistics.get(HitResult::Meh);
        let count_miss = score.statistics.get(HitResult::Miss).max(0);
        let total_hits =
            f64::from(count_perfect + count_ok + count_great + count_good + count_meh + count_miss);

        let custom_accuracy = if total_hits == 0.0 {
            0.0
        } else {
            (f64::from(count_perfect) * 320.0
                + f64::from(count_great) * 300.0
                + f64::from(count_good) * 200.0
                + f64::from(count_ok) * 100.0
                + f64::from(count_meh) * 50.0)
                / (f64::from(total_hits) * 320.0)
        }
        .clamp(0.0, 1.0);

        let length_bonus = (total_hits / 1500.0).min(1.0);
        let difficulty = 8.0
            * (attributes.star_rating - 0.15).max(0.05).powf(2.2)
            * (5.0 * custom_accuracy - 4.0).max(0.0)
            * (1.0 + 0.1 * length_bonus);

        let no_fail_multiplier = if score
            .mods
            .iter()
            .any(|modifier| matches!(modifier, Mod::NoFail))
        {
            0.75
        } else {
            1.0
        };
        let easy_multiplier = if score
            .mods
            .iter()
            .any(|modifier| matches!(modifier, Mod::Easy))
        {
            0.5
        } else {
            1.0
        };
        let multiplier = no_fail_multiplier * easy_multiplier;

        ManiaPerformanceAttributes {
            difficulty,
            total: difficulty * multiplier,
        }
    }
}

impl PerformanceCalculator for ManiaPerformanceCalculator {
    type Difficulty = ManiaDifficultyAttributes;
    type Attributes = ManiaPerformanceAttributes;

    fn calculate(
        &self,
        score: &ScoreInfo,
        difficulty: &Self::Difficulty,
    ) -> Result<Self::Attributes, CalculationError> {
        Ok(ManiaPerformanceCalculator::calculate(score, difficulty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perfect_score(count: i32, mods: Vec<Mod>) -> ScoreInfo {
        let mut score = ScoreInfo {
            mods,
            ..Default::default()
        };
        score.statistics.set(HitResult::Perfect, count);
        score
    }

    fn attributes(star_rating: f64) -> ManiaDifficultyAttributes {
        ManiaDifficultyAttributes {
            star_rating,
            ..Default::default()
        }
    }

    #[test]
    fn empty_or_low_accuracy_scores_award_no_mania_pp() {
        let empty = ManiaPerformanceCalculator::calculate(&ScoreInfo::default(), &attributes(5.0));
        assert_eq!(empty.difficulty, 0.0);
        assert_eq!(empty.total, 0.0);

        let mut misses = ScoreInfo::default();
        misses.statistics.set(HitResult::Miss, 5);
        let no_accuracy = ManiaPerformanceCalculator::calculate(&misses, &attributes(5.0));
        assert_eq!(no_accuracy.difficulty, 0.0);
    }

    #[test]
    fn perfect_score_has_difficulty_pp_and_applies_mod_multipliers() {
        let base = ManiaPerformanceCalculator::calculate(
            &perfect_score(500, Vec::new()),
            &attributes(4.0),
        );
        let no_fail = ManiaPerformanceCalculator::calculate(
            &perfect_score(500, vec![Mod::NoFail]),
            &attributes(4.0),
        );
        let easy = ManiaPerformanceCalculator::calculate(
            &perfect_score(500, vec![Mod::Easy]),
            &attributes(4.0),
        );
        let both = ManiaPerformanceCalculator::calculate(
            &perfect_score(500, vec![Mod::NoFail, Mod::Easy]),
            &attributes(4.0),
        );

        assert!(base.difficulty > 0.0);
        assert!((base.total - base.difficulty).abs() < 1e-12);
        assert!((no_fail.total / base.total - 0.75).abs() < 1e-12);
        assert!((easy.total / base.total - 0.5).abs() < 1e-12);
        assert!((both.total / base.total - 0.375).abs() < 1e-12);
    }

    #[test]
    fn length_bonus_grows_continuously_and_caps_at_1500_hits() {
        let halfway = ManiaPerformanceCalculator::calculate(
            &perfect_score(750, Vec::new()),
            &attributes(4.0),
        );
        let at_threshold = ManiaPerformanceCalculator::calculate(
            &perfect_score(1500, Vec::new()),
            &attributes(4.0),
        );
        let beyond_threshold = ManiaPerformanceCalculator::calculate(
            &perfect_score(2000, Vec::new()),
            &attributes(4.0),
        );

        assert!((at_threshold.total / halfway.total - (1.1 / 1.05)).abs() < 1e-12);
        assert!((beyond_threshold.total / at_threshold.total - 1.0).abs() < 1e-12);
    }

    #[test]
    fn miss_counts_reduce_custom_accuracy() {
        let mut score = perfect_score(9, Vec::new());
        score.statistics.set(HitResult::Miss, 1);
        let mixed = ManiaPerformanceCalculator::calculate(&score, &attributes(4.0));
        let perfect =
            ManiaPerformanceCalculator::calculate(&perfect_score(10, Vec::new()), &attributes(4.0));

        assert!(mixed.total < perfect.total);
    }
}
