//! Perfect-play PPP performance formulas from Ez2Lazer.
//!
//! These formulas compute the per-beatmap PP+ value from a chart skill vector and mode-specific chart metadata.
//! They are distinct from official score PP and do not accept score judgements, misses, or combo.

use crate::skills::{
    EzPPPCatchSkillAxis, EzPPPManiaSkillAxis, EzPPPOSUSkillMetric, EzPPPTaikoSkillAxis, SkillVector,
};

const Z_99: f64 = 2.32634787404;

/// Perfect-play PPP performance calculators for the four supported rulesets.
pub struct EzPppPerformance;

impl EzPppPerformance {
    /// osu! performance from the chart skill vector.
    ///
    /// `great_hit_window_ms` is already adjusted for the active clock rate and difficulty mods.
    pub fn osu(
        skills: &SkillVector<EzPPPOSUSkillMetric>,
        total_hits: i32,
        circle_count: i32,
        great_hit_window_ms: f64,
        hidden: bool,
        flashlight: bool,
    ) -> f64 {
        if total_hits <= 0 || !great_hit_window_ms.is_finite() || great_hit_window_ms <= 0.0 {
            return -1.0;
        }

        let overall_difficulty = (80.0 - great_hit_window_ms) / 6.0;
        let deviation = accuracy_deviation(
            circle_count,
            total_hits,
            great_hit_window_ms,
            overall_difficulty,
        );
        if !deviation.is_finite() || deviation <= 0.0 {
            return -1.0;
        }

        let mut aim_weight = 0.995_f64.powf(deviation) * 1.04;
        if flashlight {
            aim_weight *= 1.0 + (total_hits as f64 / 2000.0).atan();
        }

        let speed_weight = 0.985_f64.powf(deviation) * 1.12;
        let mut accuracy_weight = 1.2 * ((circle_count + 400) as f64 / 1050.0).tanh();
        if hidden {
            accuracy_weight *= 1.02;
        }
        if flashlight {
            accuracy_weight *= 1.04;
        }

        let aim_value =
            difficulty_to_performance(finite_or_zero(skills.get(EzPPPOSUSkillMetric::Aim)))
                * aim_weight;
        let speed_value =
            difficulty_to_performance(finite_or_zero(skills.get(EzPPPOSUSkillMetric::Speed)))
                * speed_weight;
        let stamina_value =
            difficulty_to_performance(finite_or_zero(skills.get(EzPPPOSUSkillMetric::Stamina)))
                * speed_weight;
        let accuracy_value = 560.0
            * 0.85_f64.powf(deviation)
            * finite_or_zero(skills.get(EzPPPOSUSkillMetric::AccuracyDifficulty)).max(0.0)
            * accuracy_weight;

        let total = aim_value.powf(1.1)
            + speed_value.max(stamina_value).powf(1.1)
            + accuracy_value.powf(1.1);
        1.12 * total.powf(1.0 / 1.1)
    }

    /// osu!taiko performance for a perfect play.
    ///
    /// Hit windows are great/ok judgement windows already adjusted for the active clock rate. `is_convert` is true
    /// when the source beatmap is converted to taiko rather than native taiko.
    pub fn taiko(
        skills: &SkillVector<EzPPPTaikoSkillAxis>,
        total_hits: i32,
        great_hit_window_ms: f64,
        ok_hit_window_ms: f64,
        hidden: bool,
        easy: bool,
        hard_rock: bool,
        flashlight: bool,
        is_convert: bool,
    ) -> f64 {
        let star_rating = skills.get(EzPPPTaikoSkillAxis::StarRating);
        if total_hits <= 0 || !star_rating.is_finite() || star_rating <= 0.0 {
            return -1.0;
        }

        let deviation = taiko_deviation(total_hits, great_hit_window_ms, ok_hit_window_ms);
        if !deviation.is_finite() || deviation <= 0.0 {
            return -1.0;
        }

        let urgency = 10.0 * deviation;
        let mut multiplier: f64 = 1.13;
        if hidden {
            multiplier *= 1.075;
        }
        if easy {
            multiplier *= 0.975;
        }

        let length_bonus = 1.0 + 0.1 * (total_hits as f64 / 1500.0).min(1.0);
        let mut difficulty = (5.0 * (star_rating / 0.115).max(1.0) - 4.0).powf(2.25) / 1150.0;
        difficulty *= length_bonus;
        difficulty *= if easy { 0.985 } else { 1.0 };
        difficulty *= if hidden && !is_convert { 1.025 } else { 1.0 };
        difficulty *= if hard_rock { 1.10 } else { 1.0 };
        difficulty *= if flashlight {
            1.0 + 0.05 * length_bonus
        } else {
            1.0
        };
        difficulty *= erf(400.0 / (2.0_f64.sqrt() * urgency)).powi(2);

        let mut accuracy = (70.0 / urgency).powf(1.1) * star_rating.powf(0.4) * 100.0;
        if flashlight && hidden && !is_convert {
            accuracy *= (1.05 * (total_hits as f64 / 1500.0).powf(0.3).min(1.15)).max(1.0);
        }

        (difficulty.max(0.0).powf(1.1) + accuracy.max(0.0).powf(1.1)).powf(1.0 / 1.1) * multiplier
    }

    /// osu!catch performance for a perfect play.
    pub fn catch_chart(
        skills: &SkillVector<EzPPPCatchSkillAxis>,
        approach_rate: f64,
        total_hits: i32,
        combo_objects: i32,
        hidden: bool,
        flashlight: bool,
        no_fail: bool,
    ) -> f64 {
        let star_rating = skills.get(EzPPPCatchSkillAxis::StarRating);
        if total_hits <= 0 || combo_objects <= 0 || !star_rating.is_finite() || star_rating <= 0.0 {
            return -1.0;
        }

        let base_value = (5.0 * (star_rating / 0.0049).max(1.0) - 4.0).powi(2) / 100_000.0;
        let mut length_bonus = 0.95 + 0.3 * (combo_objects as f64 / 2500.0).min(1.0);
        if combo_objects > 2500 {
            length_bonus += (combo_objects as f64 / 2500.0).log10() * 0.475;
        }

        let approach_rate_bonus = if approach_rate > 10.0 {
            1.0 + 0.1 * (approach_rate - 9.0) + 0.1 * (approach_rate - 10.0)
        } else if approach_rate > 9.0 {
            1.0 + 0.1 * (approach_rate - 9.0)
        } else if approach_rate < 8.0 {
            1.0 + 0.025 * (8.0 - approach_rate)
        } else {
            1.0
        };

        let mut total = base_value * length_bonus;
        if hidden {
            let hidden_bonus = if approach_rate <= 10.0 {
                1.05 + 0.075 * (10.0 - approach_rate)
            } else {
                1.01 + 0.04 * (11.0 - approach_rate.min(11.0))
            };
            total *= hidden_bonus;
        }
        if flashlight {
            total *= 1.0 + 0.35 * length_bonus;
        }
        if no_fail {
            total *= 1.0;
        }
        total * approach_rate_bonus
    }

    /// osu!mania performance for a perfect play.
    pub fn mania(
        skills: &SkillVector<EzPPPManiaSkillAxis>,
        total_hits: i32,
        no_fail: bool,
        easy: bool,
    ) -> f64 {
        let star_rating = skills.get(EzPPPManiaSkillAxis::StarRating);
        if total_hits <= 0 || !star_rating.is_finite() || star_rating <= 0.0 {
            return -1.0;
        }

        let mut value = 8.0
            * (star_rating - 0.15).max(0.05).powf(2.2)
            * (1.0 + 0.1 * (total_hits as f64 / 1500.0).min(1.0));
        if no_fail {
            value *= 0.75;
        }
        if easy {
            value *= 0.5;
        }
        value
    }
}

fn accuracy_deviation(
    circle_count: i32,
    _total_hits: i32,
    great_hit_window_ms: f64,
    overall_difficulty: f64,
) -> f64 {
    if circle_count <= 0 {
        return 200.0 - 10.0 * overall_difficulty;
    }

    let lower = 0.2_f64.powf(1.0 / circle_count as f64);
    let quantile = lower + (1.0 - lower) / 2.0;
    great_hit_window_ms / inverse_normal_cdf(quantile)
}

fn taiko_deviation(total_hits: i32, great_hit_window_ms: f64, ok_hit_window_ms: f64) -> f64 {
    if !great_hit_window_ms.is_finite()
        || !ok_hit_window_ms.is_finite()
        || great_hit_window_ms <= 0.0
        || ok_hit_window_ms <= 0.0
    {
        return f64::NAN;
    }

    let n = total_hits as f64;
    let z_squared = Z_99 * Z_99;
    let lower =
        (n + z_squared / 2.0) / (n + z_squared) - Z_99 / (n + z_squared) * (z_squared / 4.0).sqrt();
    let inverse = inverse_erf(lower);
    if !inverse.is_finite() || inverse <= 0.0 {
        return f64::NAN;
    }

    let great = great_hit_window_ms / (2.0_f64.sqrt() * inverse);
    let ok = ok_hit_window_ms / (2.0_f64.sqrt() * inverse);
    great.min(ok)
}

/// Acklam's rational approximation to the inverse standard normal CDF, matching EzPPPMath.
pub fn inverse_normal_cdf(p: f64) -> f64 {
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }

    const P_LOW: f64 = 0.02425;
    const A: [f64; 6] = [
        -39.69683028665376,
        220.9460984245205,
        -275.9285104469687,
        138.3577518672690,
        -30.66479806614716,
        2.506628277459239,
    ];
    const B: [f64; 5] = [
        -54.47609879822406,
        161.5858368580409,
        -155.6989798598866,
        66.80131188771972,
        -13.28068155288572,
    ];
    const C: [f64; 6] = [
        -0.007784894002430293,
        -0.3223964580411365,
        -2.400758277161838,
        -2.549732539343734,
        4.374664141464968,
        2.938163982698783,
    ];
    const D: [f64; 4] = [
        0.007784695709041462,
        0.3224671290700398,
        2.445134137142996,
        3.754408661907416,
    ];

    if p < P_LOW {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p > 1.0 - P_LOW {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else {
        let t = p - 0.5;
        let r = t * t;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * t
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    }
}

/// Inverse error function using the same normal-CDF relationship as EzPPPMath.
pub fn inverse_erf(value: f64) -> f64 {
    if value <= -1.0 {
        return f64::NEG_INFINITY;
    }
    if value >= 1.0 {
        return f64::INFINITY;
    }
    inverse_normal_cdf((value + 1.0) / 2.0) / 2.0_f64.sqrt()
}

/// Abramowitz and Stegun 7.1.26 approximation used by the C# PP+ calculator.
pub fn erf(value: f64) -> f64 {
    let sign = if value < 0.0 { -1.0 } else { 1.0 };
    let absolute = value.abs();
    let t = 1.0 / (1.0 + 0.3275911 * absolute);
    let polynomial = ((((1.061405429 * t - 1.453152027) * t + 1.421413741) * t - 0.284496736) * t
        + 0.254829592)
        * t;
    sign * (1.0 - polynomial * (-absolute * absolute).exp())
}

fn difficulty_to_performance(difficulty: f64) -> f64 {
    3.9 * difficulty * difficulty * difficulty
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn osu_skills(
        aim: f64,
        speed: f64,
        stamina: f64,
        accuracy: f64,
    ) -> SkillVector<EzPPPOSUSkillMetric> {
        SkillVector::from_fn(|axis| match axis {
            EzPPPOSUSkillMetric::Aim => aim,
            EzPPPOSUSkillMetric::Speed => speed,
            EzPPPOSUSkillMetric::Stamina => stamina,
            EzPPPOSUSkillMetric::AccuracyDifficulty => accuracy,
            _ => 0.0,
        })
    }

    fn taiko_skills(star: f64) -> SkillVector<EzPPPTaikoSkillAxis> {
        SkillVector::from_fn(|axis| {
            if axis == EzPPPTaikoSkillAxis::StarRating {
                star
            } else {
                0.0
            }
        })
    }

    fn catch_skills(star: f64) -> SkillVector<EzPPPCatchSkillAxis> {
        SkillVector::from_fn(|axis| {
            if axis == EzPPPCatchSkillAxis::StarRating {
                star
            } else {
                0.0
            }
        })
    }

    fn mania_skills(star: f64) -> SkillVector<EzPPPManiaSkillAxis> {
        SkillVector::from_fn(|axis| {
            if axis == EzPPPManiaSkillAxis::StarRating {
                star
            } else {
                0.0
            }
        })
    }

    #[test]
    fn inverse_normal_and_erf_match_documented_quantiles() {
        assert!(inverse_normal_cdf(0.5).abs() < 1e-14);
        assert!((inverse_normal_cdf(0.975) - 1.959963984540054).abs() < 1e-8);
        assert!((inverse_normal_cdf(0.025) + 1.959963984540054).abs() < 1e-8);
        assert!((inverse_normal_cdf(0.001) + 3.090232306167813).abs() < 1e-8);
        assert!((inverse_erf(0.8427007929497149) - 1.0).abs() < 1e-7);
        assert!((erf(1.0) - 0.8427007929497149).abs() < 1.5e-7);
    }

    #[test]
    fn mania_matches_documented_perfect_play_expression_and_mods() {
        let skills = mania_skills(5.0);
        let expected = 8.0 * 4.85_f64.powf(2.2) * (1.0 + 0.1 * (1000.0 / 1500.0));
        assert!((EzPppPerformance::mania(&skills, 1000, false, false) - expected).abs() < 1e-10);
        assert!(
            (EzPppPerformance::mania(&skills, 1000, true, false) - expected * 0.75).abs() < 1e-10
        );
        assert!(
            (EzPppPerformance::mania(&skills, 1000, false, true) - expected * 0.5).abs() < 1e-10
        );
    }

    #[test]
    fn catch_perfect_play_scales_with_rating_and_length() {
        let small =
            EzPppPerformance::catch_chart(&catch_skills(4.0), 9.2, 500, 500, false, false, false);
        let larger =
            EzPppPerformance::catch_chart(&catch_skills(6.0), 9.2, 500, 500, false, false, false);
        let longer =
            EzPppPerformance::catch_chart(&catch_skills(4.0), 9.2, 1200, 1200, false, false, false);
        assert!(small > 0.0 && larger > small && longer > small);
    }

    #[test]
    fn taiko_perfect_play_scales_with_rating_and_easy_mod() {
        let low = EzPppPerformance::taiko(
            &taiko_skills(3.0),
            800,
            25.0,
            50.0,
            false,
            false,
            false,
            false,
            false,
        );
        let high = EzPppPerformance::taiko(
            &taiko_skills(6.0),
            800,
            25.0,
            50.0,
            false,
            false,
            false,
            false,
            false,
        );
        let easy = EzPppPerformance::taiko(
            &taiko_skills(6.0),
            800,
            25.0,
            50.0,
            false,
            true,
            false,
            false,
            false,
        );
        assert!(low > 0.0 && high > low && easy < high);
    }

    #[test]
    fn osu_perfect_play_scales_with_aim_and_rejects_empty_chart() {
        let baseline = EzPppPerformance::osu(
            &osu_skills(4.0, 3.0, 2.0, 1.0),
            800,
            400,
            26.0,
            false,
            false,
        );
        let stronger_aim = EzPppPerformance::osu(
            &osu_skills(6.0, 3.0, 2.0, 1.0),
            800,
            400,
            26.0,
            false,
            false,
        );
        assert!(baseline > 0.0 && stronger_aim > baseline);
        assert_eq!(
            EzPppPerformance::osu(&osu_skills(4.0, 3.0, 2.0, 1.0), 0, 0, 26.0, false, false),
            -1.0
        );
    }

    #[test]
    fn invalid_mode_inputs_report_unavailable() {
        assert_eq!(
            EzPppPerformance::mania(&mania_skills(2.0), 0, false, false),
            -1.0
        );
        assert_eq!(
            EzPppPerformance::taiko(
                &taiko_skills(f64::NAN),
                100,
                25.0,
                50.0,
                false,
                false,
                false,
                false,
                false
            ),
            -1.0
        );
        assert_eq!(
            EzPppPerformance::catch_chart(&catch_skills(2.0), 5.0, 10, 0, false, false, false),
            -1.0
        );
    }
}
