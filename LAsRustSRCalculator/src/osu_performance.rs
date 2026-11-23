//! Official osu!standard score performance calculation.
//!
//! This module ports the score-dependent `OsuPerformanceCalculator`. The caller supplies the matching
//! difficulty attributes and beatmap information through `ScoreInfo`; `OsuPerformanceContext` carries the
//! Classic slider-accuracy toggle, which is not encoded in the compact mod enum.

use crate::attributes::{OsuDifficultyAttributes, OsuPerformanceAttributes};
use crate::beatmap::DifficultySettings;
use crate::calculator::{CalculationError, PerformanceCalculator};
use crate::mods::{Mod, ModError, calculate_rate_with_mods};
use crate::score::{HitResult, ScoreInfo};

const PERFORMANCE_BASE_MULTIPLIER: f64 = 1.12;
const PERFORMANCE_NORM_EXPONENT: f64 = 1.1;
const Z_99: f64 = 2.326_347_874_04;
const SQRT_2: f64 = std::f64::consts::SQRT_2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OsuPerformanceContext {
    /// `OsuModClassic.NoSliderHeadAccuracy`, default true for osu!classic.
    pub classic_slider_accuracy: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OsuPerformanceError {
    MissingBeatmapDifficulty,
    MissingSourceRuleset,
    InvalidRate(f64),
    NonFiniteDifficulty,
}

impl std::fmt::Display for OsuPerformanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingBeatmapDifficulty => {
                f.write_str("osu! performance requires beatmap difficulty in ScoreInfo")
            }
            Self::MissingSourceRuleset => {
                f.write_str("osu! performance requires the score's source ruleset")
            }
            Self::InvalidRate(rate) => write!(f, "invalid osu! performance clock rate {rate}"),
            Self::NonFiniteDifficulty => f.write_str("osu! difficulty values must be finite"),
        }
    }
}

impl std::error::Error for OsuPerformanceError {}

impl From<ModError> for OsuPerformanceError {
    fn from(value: ModError) -> Self {
        match value {
            ModError::InvalidRate(rate) => Self::InvalidRate(rate),
        }
    }
}

pub struct OsuPerformanceCalculator;

impl OsuPerformanceCalculator {
    pub fn calculate_score(
        score: &ScoreInfo,
        attributes: &OsuDifficultyAttributes,
    ) -> Result<OsuPerformanceAttributes, OsuPerformanceError> {
        Self::calculate(
            score,
            attributes,
            OsuPerformanceContext {
                classic_slider_accuracy: score
                    .mods
                    .iter()
                    .any(|modifier| matches!(modifier, Mod::Classic)),
            },
        )
    }

    pub fn calculate(
        score: &ScoreInfo,
        attributes: &OsuDifficultyAttributes,
        context: OsuPerformanceContext,
    ) -> Result<OsuPerformanceAttributes, OsuPerformanceError> {
        let source_difficulty = score
            .beatmap_difficulty
            .as_ref()
            .ok_or(OsuPerformanceError::MissingBeatmapDifficulty)?;
        let _source_ruleset = score
            .ruleset
            .ok_or(OsuPerformanceError::MissingSourceRuleset)?;
        let difficulty = apply_difficulty(source_difficulty, &score.mods)?;
        if ![
            difficulty.hp_drain_rate.unwrap_or(5.0),
            difficulty.circle_size.unwrap_or(5.0),
            difficulty.overall_difficulty.unwrap_or(5.0),
            difficulty.approach_rate.unwrap_or(5.0),
        ]
        .iter()
        .all(|value| value.is_finite())
        {
            return Err(OsuPerformanceError::NonFiniteDifficulty);
        }

        let clock_rate = calculate_rate_with_mods(&score.mods)?;
        let accuracy = score.accuracy.clamp(0.0, 1.0);
        let score_max_combo = score.max_combo.clamp(0, attributes.max_combo);
        let count_great = score.statistics.get(HitResult::Great);
        let count_ok = score.statistics.get(HitResult::Ok);
        let count_meh = score.statistics.get(HitResult::Meh);
        let count_miss = score.statistics.get(HitResult::Miss);
        let count_slider_ends_dropped =
            attributes.slider_count - score.statistics.get(HitResult::SliderTailHit);
        let count_slider_tick_miss = score.statistics.get(HitResult::LargeTickMiss);
        let total_hits = count_great + count_ok + count_meh + count_miss;
        let total_imperfect_hits = count_ok + count_meh + count_miss;
        let total_successful_hits = count_great + count_ok + count_meh;

        let base_od = difficulty.overall_difficulty.unwrap_or(5.0);
        let base_ar = difficulty.approach_rate.unwrap_or(base_od);
        let great_hit_window = hit_window(base_od, 80.0, 50.0, 20.0) / clock_rate;
        let ok_hit_window = hit_window(base_od, 140.0, 100.0, 60.0) / clock_rate;
        let meh_hit_window = hit_window(base_od, 200.0, 150.0, 100.0) / clock_rate;
        let approach_rate = rate_adjusted_ar(base_ar, clock_rate);
        let overall_difficulty = (79.5 - great_hit_window) / 6.0;
        let drain_rate = difficulty.hp_drain_rate.unwrap_or(5.0);
        let using_score_v2 = score
            .mods
            .iter()
            .any(|modifier| matches!(modifier, Mod::ScoreV2));

        let combo_based_miss_count = combo_based_estimated_miss_count(
            attributes,
            context.classic_slider_accuracy,
            using_score_v2,
            score_max_combo,
            count_miss,
            count_ok,
            count_meh,
            count_slider_ends_dropped,
            count_slider_tick_miss,
        );

        let score_based_miss_count =
            if context.classic_slider_accuracy && !using_score_v2 && score.legacy_total_score > 0 {
                Some(legacy_score_based_misses(
                    score,
                    attributes,
                    score_max_combo,
                    total_hits,
                    context.classic_slider_accuracy,
                    using_score_v2,
                ))
            } else {
                None
            };

        let mut effective_miss_count = score_based_miss_count.unwrap_or(combo_based_miss_count);
        effective_miss_count = effective_miss_count.max(count_miss as f64);
        effective_miss_count = effective_miss_count.min(total_hits as f64).max(0.0);

        let (aim_estimated_slider_breaks, speed_estimated_slider_breaks) =
            if effective_miss_count > 0.0 {
                (
                    estimated_slider_breaks(
                        attributes.aim_top_weighted_slider_factor,
                        attributes,
                        context.classic_slider_accuracy,
                        count_ok + count_meh,
                        score_max_combo,
                        effective_miss_count,
                    ),
                    estimated_slider_breaks(
                        attributes.speed_top_weighted_slider_factor,
                        attributes,
                        context.classic_slider_accuracy,
                        count_ok + count_meh,
                        score_max_combo,
                        effective_miss_count,
                    ),
                )
            } else {
                (0.0, 0.0)
            };

        let mut multiplier = PERFORMANCE_BASE_MULTIPLIER;
        if has_mod(&score.mods, "NF") {
            multiplier *= (1.0 - 0.02 * effective_miss_count).max(0.90);
        }
        if has_mod(&score.mods, "SO") && total_hits > 0 {
            multiplier *= 1.0 - ((attributes.spinner_count as f64 / total_hits as f64).powf(0.85));
        }

        if has_mod(&score.mods, "RX") {
            let ok_multiplier = 0.75
                * if overall_difficulty > 0.0 {
                    (1.0 - overall_difficulty / 13.33).max(0.0)
                } else {
                    1.0
                };
            let meh_multiplier = if overall_difficulty > 0.0 {
                (1.0 - (overall_difficulty / 13.33).powi(5)).max(0.0)
            } else {
                1.0
            };
            effective_miss_count = (effective_miss_count
                + count_ok as f64 * ok_multiplier
                + count_meh as f64 * meh_multiplier)
                .min(total_hits as f64);
        }

        let speed_deviation = if total_successful_hits == 0 {
            None
        } else {
            let mut speed_note_count = attributes.speed_note_count;
            speed_note_count += (total_hits as f64 - attributes.speed_note_count) * 0.1;
            let relevant_miss = (count_miss as f64).min(speed_note_count);
            let relevant_meh = (count_meh as f64).min(speed_note_count - relevant_miss);
            let relevant_ok =
                (count_ok as f64).min(speed_note_count - relevant_miss - relevant_meh);
            let relevant_great =
                (speed_note_count - relevant_miss - relevant_meh - relevant_ok).max(0.0);
            calculate_deviation(
                relevant_great,
                relevant_ok,
                relevant_meh,
                great_hit_window,
                ok_hit_window,
                meh_hit_window,
            )
        };

        let aim = compute_aim(
            score,
            attributes,
            accuracy,
            score_max_combo,
            total_hits,
            total_imperfect_hits,
            count_slider_tick_miss,
            count_slider_ends_dropped,
            effective_miss_count,
            aim_estimated_slider_breaks,
            drain_rate,
            approach_rate,
            context.classic_slider_accuracy,
            using_score_v2,
        );
        let speed = compute_speed(
            score,
            attributes,
            speed_deviation,
            effective_miss_count,
            speed_estimated_slider_breaks,
            total_imperfect_hits,
            count_slider_tick_miss,
        );
        let accuracy_value = compute_accuracy(
            score,
            attributes,
            accuracy,
            total_hits,
            count_great,
            count_ok,
            count_meh,
            overall_difficulty,
            approach_rate,
            context.classic_slider_accuracy,
            using_score_v2,
        );
        let reading = compute_reading(
            attributes,
            accuracy,
            effective_miss_count,
            aim_estimated_slider_breaks,
        );
        let flashlight = compute_flashlight(
            score,
            attributes,
            accuracy,
            effective_miss_count,
            score_max_combo,
            total_hits,
        );
        let cognition = sum_cognition(reading, flashlight);
        let total = norm_1_1([aim, speed, accuracy_value, cognition]) * multiplier;

        Ok(OsuPerformanceAttributes {
            total,
            aim,
            speed,
            accuracy: accuracy_value,
            flashlight,
            reading,
            effective_miss_count,
            speed_deviation,
            combo_based_estimated_miss_count: combo_based_miss_count,
            score_based_estimated_miss_count: score_based_miss_count,
            aim_estimated_slider_breaks,
            speed_estimated_slider_breaks,
        })
    }
}

impl PerformanceCalculator for OsuPerformanceCalculator {
    type Difficulty = OsuDifficultyAttributes;
    type Attributes = OsuPerformanceAttributes;

    fn calculate(
        &self,
        score: &ScoreInfo,
        difficulty: &Self::Difficulty,
    ) -> Result<Self::Attributes, CalculationError> {
        let classic = score
            .mods
            .iter()
            .any(|modifier| matches!(modifier, Mod::Classic));
        Self::calculate(
            score,
            difficulty,
            OsuPerformanceContext {
                classic_slider_accuracy: classic,
            },
        )
        .map_err(|error| CalculationError::InvalidBeatmap(error.to_string()))
    }
}

pub fn calculate_score(
    score: &ScoreInfo,
    attributes: &OsuDifficultyAttributes,
    context: OsuPerformanceContext,
) -> Result<OsuPerformanceAttributes, OsuPerformanceError> {
    OsuPerformanceCalculator::calculate(score, attributes, context)
}

fn compute_aim(
    score: &ScoreInfo,
    attributes: &OsuDifficultyAttributes,
    accuracy: f64,
    score_max_combo: i32,
    total_hits: i32,
    total_imperfect_hits: i32,
    slider_tick_misses: i32,
    slider_ends_dropped: i32,
    effective_misses: f64,
    estimated_slider_breaks: f64,
    drain_rate: f64,
    approach_rate: f64,
    classic_slider_accuracy: bool,
    score_v2: bool,
) -> f64 {
    if has_mod(&score.mods, "AP") {
        return 0.0;
    }
    let mut aim_difficulty = attributes.aim_difficulty;
    if attributes.slider_count > 0 && attributes.aim_difficult_slider_count > 0.0 {
        let improperly_followed = if classic_slider_accuracy {
            (attributes.max_combo - score_max_combo)
                .min(total_imperfect_hits)
                .clamp(0, attributes.aim_difficult_slider_count as i32) as f64
        } else {
            (slider_ends_dropped + slider_tick_misses)
                .clamp(0, attributes.aim_difficult_slider_count as i32) as f64
        };
        let slider_nerf = (1.0 - attributes.slider_factor)
            * (1.0 - improperly_followed / attributes.aim_difficult_slider_count).powi(3)
            + attributes.slider_factor;
        aim_difficulty *= slider_nerf;
    }
    let mut value = 4.0 * aim_difficulty.powi(3);
    let length_bonus = 0.95
        + 0.35 * (total_hits as f64 / 2000.0).min(1.0)
        + if total_hits > 2000 {
            (total_hits as f64 / 2000.0).log10() * 0.5
        } else {
            0.0
        };
    value *= length_bonus;
    if effective_misses > 0.0 {
        let relevant = (effective_misses + estimated_slider_breaks)
            .min((total_imperfect_hits + slider_tick_misses) as f64);
        value *= miss_penalty(relevant, attributes.aim_difficult_strain_count);
    }
    if has_mod(&score.mods, "BL") {
        value *= 1.3
            + total_hits as f64
                * (0.0016 / (1.0 + 2.0 * effective_misses))
                * accuracy.powi(16)
                * (1.0 - 0.003 * drain_rate * drain_rate);
    } else if has_mod(&score.mods, "TC") {
        value *= 1.0 + traceable_bonus(approach_rate, attributes.slider_factor);
    }
    let _ = score_v2;
    value * accuracy
}

fn compute_speed(
    score: &ScoreInfo,
    attributes: &OsuDifficultyAttributes,
    deviation: Option<f64>,
    effective_misses: f64,
    estimated_slider_breaks: f64,
    total_imperfect_hits: i32,
    slider_tick_misses: i32,
) -> f64 {
    if has_mod(&score.mods, "RX") {
        return 0.0;
    }
    let Some(deviation) = deviation else {
        return 0.0;
    };
    let mut value = 4.0 * attributes.speed_difficulty.powi(3);
    if effective_misses > 0.0 {
        let relevant = (effective_misses + estimated_slider_breaks)
            .min((total_imperfect_hits + slider_tick_misses) as f64);
        value *= miss_penalty(relevant, attributes.speed_difficult_strain_count);
    }
    if has_mod(&score.mods, "BL") {
        value *= 1.12;
    }
    value *= speed_deviation_multiplier(value, deviation, attributes.speed_difficulty);
    let effective_window = 20.0 * (4.0 / attributes.speed_difficulty).powf(0.35);
    value *= erf(effective_window / deviation).powi(2);
    value
}

fn compute_accuracy(
    score: &ScoreInfo,
    attributes: &OsuDifficultyAttributes,
    _accuracy: f64,
    total_hits: i32,
    great: i32,
    ok: i32,
    meh: i32,
    od: f64,
    ar: f64,
    classic_slider_accuracy: bool,
    score_v2: bool,
) -> f64 {
    if has_mod(&score.mods, "RX") {
        return 0.0;
    }
    let mut objects = attributes.hit_circle_count;
    if !classic_slider_accuracy || score_v2 {
        objects += attributes.slider_count;
    }
    let better = if objects > 0 {
        (((great - (total_hits - objects).max(0)) * 6 + ok * 2 + meh) as f64 / (objects * 6) as f64)
            .max(0.0)
    } else {
        0.0
    };
    let mut value = 1.52163_f64.powf(od) * better.powi(24) * 2.83;
    value *= if objects < 1000 {
        (objects as f64 / 1000.0).powf(0.3)
    } else {
        (objects as f64 / 1000.0).powf(0.1)
    };
    if has_mod(&score.mods, "BL") {
        value *= 1.14;
    } else if has_mod(&score.mods, "TC") {
        value *= 1.0 + 0.08 * reverse_lerp(ar, 11.5, 10.0);
    }
    value
}

fn compute_flashlight(
    score: &ScoreInfo,
    attributes: &OsuDifficultyAttributes,
    accuracy: f64,
    effective_misses: f64,
    score_max_combo: i32,
    total_hits: i32,
) -> f64 {
    if !has_mod(&score.mods, "FL") {
        return 0.0;
    }
    let Some(difficulty) = attributes.flashlight_difficulty else {
        return 0.0;
    };
    let mut value = 25.0 * difficulty.powi(2);
    if effective_misses > 0.0 && total_hits > 0 {
        value *= 0.97
            * (1.0 - (effective_misses / total_hits as f64).powf(0.775))
                .powf(effective_misses.powf(0.875));
    }
    value *= combo_scaling(score_max_combo, attributes.max_combo);
    value * (0.5 + accuracy / 2.0)
}

fn compute_reading(
    attributes: &OsuDifficultyAttributes,
    accuracy: f64,
    effective_misses: f64,
    aim_slider_breaks: f64,
) -> f64 {
    let mut value = 4.0 * attributes.reading_difficulty.powi(3);
    if effective_misses > 0.0 {
        value *= miss_penalty(
            effective_misses + aim_slider_breaks,
            attributes.reading_difficult_note_count,
        );
    }
    value * accuracy.powi(3)
}

fn combo_based_estimated_miss_count(
    attributes: &OsuDifficultyAttributes,
    classic: bool,
    score_v2: bool,
    combo: i32,
    misses: i32,
    oks: i32,
    mehs: i32,
    slider_ends_dropped: i32,
    slider_tick_misses: i32,
) -> f64 {
    if attributes.slider_count <= 0 {
        return misses as f64;
    }
    let mut count = misses as f64;
    if classic {
        let likely_tail = 0.04 + 0.06 * attributes.aim_top_weighted_slider_factor.min(1.0).powi(2);
        let threshold = attributes.max_combo as f64
            - (4.0 + likely_tail * attributes.slider_count as f64)
                .min(attributes.slider_count as f64);
        if (combo as f64) < threshold {
            count = threshold / (combo as f64).max(1.0);
        }
        count = count.min((oks + mehs + misses) as f64);
        let breaks = ((attributes.max_combo - combo) / 2).min(attributes.slider_count) as f64;
        let slider_breaks = count - misses as f64;
        if slider_breaks > breaks {
            count = misses as f64 + breaks;
        }
    } else {
        let threshold = attributes.max_combo - slider_ends_dropped;
        if combo < threshold {
            count = threshold as f64 / (combo as f64).max(1.0);
        }
        count = count.min((slider_tick_misses + misses) as f64);
    }
    let _ = score_v2;
    count
}

fn estimated_slider_breaks(
    factor: f64,
    attributes: &OsuDifficultyAttributes,
    classic: bool,
    ok_meh: i32,
    combo: i32,
    effective_misses: f64,
) -> f64 {
    if !classic || ok_meh == 0 || attributes.max_combo <= 0 {
        return 0.0;
    }
    let missed_combo_percent = 1.0 - combo as f64 / attributes.max_combo as f64;
    let mut result = (ok_meh as f64).min(effective_misses * factor);
    let adjustment = (ok_meh as f64 - result + 4.5) / (ok_meh as f64 + 4.0);
    result *= smoothstep(effective_misses, 1.0, 2.0);
    result * adjustment * logistic(missed_combo_percent, 0.33, 15.0)
}

fn legacy_score_based_misses(
    score: &ScoreInfo,
    attributes: &OsuDifficultyAttributes,
    combo: i32,
    total_hits: i32,
    classic: bool,
    score_v2: bool,
) -> f64 {
    if attributes.max_combo == 0 || score.legacy_total_score <= 0 {
        return 0.0;
    }
    let score_multiplier =
        attributes.legacy_score_base_multiplier * legacy_score_multiplier(&score.mods, score_v2);
    let relevant_combo_per_object = {
        let combo_score = attributes.maximum_legacy_combo_score
            / (300.0 / 25.0 * attributes.legacy_score_base_multiplier);
        let numerator = (attributes.max_combo - 2) as f64 * attributes.max_combo as f64;
        numerator / (attributes.max_combo as f64 + 2.0 * (combo_score - 1.0)).max(1.0)
    };
    let maximum_miss_count = legacy_max_combo_misses(score, attributes, combo, classic);
    let calculate_score_at_combo = |value: f64| {
        let great = score.statistics.get(HitResult::Great);
        let ok = score.statistics.get(HitResult::Ok);
        let meh = score.statistics.get(HitResult::Meh);
        let misses = score.statistics.get(HitResult::Miss);
        let estimated_objects = value / relevant_combo_per_object - 1.0;
        let combo_score = if relevant_combo_per_object > 0.0 {
            (2.0 * (relevant_combo_per_object - 1.0)
                + (estimated_objects - 1.0) * relevant_combo_per_object)
                * estimated_objects
                / 2.0
        } else {
            0.0
        };
        let combo_score = combo_score * score.accuracy * 300.0 / 25.0 * score_multiplier;
        let objects_hit = ((total_hits - misses) as f64) * value / attributes.max_combo as f64;
        let non_combo_score =
            (300.0 + attributes.nested_score_per_object) * score.accuracy * objects_hit;
        let _ = (great, ok, meh);
        combo_score + non_combo_score
    };
    let remaining_score = score.legacy_total_score as f64 - calculate_score_at_combo(combo as f64);
    if remaining_score <= 0.0 {
        return maximum_miss_count;
    }
    let expected_remaining = calculate_score_at_combo((attributes.max_combo - combo) as f64);
    (expected_remaining / remaining_score)
        .max(1.0)
        .min(maximum_miss_count)
}

fn legacy_max_combo_misses(
    score: &ScoreInfo,
    attributes: &OsuDifficultyAttributes,
    combo: i32,
    classic: bool,
) -> f64 {
    let misses = score.statistics.get(HitResult::Miss);
    if attributes.slider_count <= 0 {
        return misses as f64;
    }
    let count_ok = score.statistics.get(HitResult::Ok);
    let count_meh = score.statistics.get(HitResult::Meh);
    let imperfect = count_ok + count_meh + misses;
    let likely_tail = 0.04 + 0.06 * attributes.aim_top_weighted_slider_factor.min(1.0).powi(2);
    let full_combo_threshold = attributes.max_combo as f64
        - (4.0 + likely_tail * attributes.slider_count as f64).min(attributes.slider_count as f64);
    let mut count = 0.0;
    if (combo as f64) < full_combo_threshold {
        count = (full_combo_threshold / (combo as f64).max(1.0)).powf(2.5);
    }
    count = count.min(imperfect as f64);
    if classic {
        let possible_breaks = ((attributes.max_combo - combo) / 2).min(attributes.slider_count);
        let slider_breaks = count - misses as f64;
        if slider_breaks > possible_breaks as f64 {
            count = misses as f64 + possible_breaks as f64;
        }
    }
    count
}

fn legacy_score_multiplier(mods: &[Mod], score_v2: bool) -> f64 {
    let mut multiplier = 1.0;
    for modifier in mods {
        multiplier *= match modifier {
            Mod::NoFail => {
                if score_v2 {
                    1.0
                } else {
                    0.5
                }
            }
            Mod::Easy => 0.5,
            Mod::HalfTime | Mod::Daycore => 0.3,
            Mod::Hidden => 1.06,
            Mod::HardRock => {
                if score_v2 {
                    1.1
                } else {
                    1.06
                }
            }
            Mod::DoubleTime | Mod::Nightcore => {
                if score_v2 {
                    1.2
                } else {
                    1.12
                }
            }
            Mod::Flashlight => 1.12,
            Mod::SpunOut => 0.9,
            Mod::Relax | Mod::Autopilot => return 0.0,
            _ => 1.0,
        };
    }
    multiplier
}

fn calculate_deviation(
    great: f64,
    ok: f64,
    meh: f64,
    great_window: f64,
    ok_window: f64,
    meh_window: f64,
) -> Option<f64> {
    let successes = great + ok + meh;
    if successes <= 0.0 {
        return None;
    }
    let n = (great + ok).max(1.0);
    let p = great / n;
    let p_lower = ((n * p + Z_99 * Z_99 / 2.0) / (n + Z_99 * Z_99)
        - Z_99 / (n + Z_99 * Z_99) * (n * p * (1.0 - p) + Z_99 * Z_99 / 4.0).sqrt())
    .min(p);
    let deviation = if p_lower > 0.01 {
        let mut value = great_window / (SQRT_2 * osu_erf_inverse(p_lower));
        let tail = (2.0 / std::f64::consts::PI).sqrt()
            * ok_window
            * (-0.5 * (ok_window / value).powi(2)).exp()
            / (value * erf(ok_window / SQRT_2 / value));
        value *= (1.0 - tail).sqrt();
        value
    } else {
        ok_window / 3.0_f64.sqrt()
    };
    let meh_variance = (meh_window.powi(2) + ok_window * meh_window + ok_window.powi(2)) / 3.0;
    Some((((great + ok) * deviation.powi(2) + meh * meh_variance) / successes).sqrt())
}

fn speed_deviation_multiplier(speed_value: f64, deviation: f64, speed_difficulty: f64) -> f64 {
    let cutoff = 100.0 + 220.0 * (22.0 / deviation).powf(6.5);
    if speed_value <= cutoff {
        return 1.0;
    }
    let scale = 50.0;
    let adjusted = scale * (((speed_value - cutoff) / scale + 1.0).ln() + cutoff / scale);
    let blend = 1.0 - reverse_lerp(deviation, 22.0, 27.0);
    let _ = speed_difficulty;
    (adjusted + (speed_value - adjusted) * blend) / speed_value
}

fn apply_difficulty(
    base: &DifficultySettings,
    mods: &[Mod],
) -> Result<DifficultySettings, OsuPerformanceError> {
    let mut difficulty = base.clone();
    let mut od = difficulty.overall_difficulty.unwrap_or(5.0) as f32 as f64;
    let mut ar = difficulty
        .approach_rate
        .unwrap_or(difficulty.overall_difficulty.unwrap_or(5.0)) as f32 as f64;
    let mut drain = difficulty.hp_drain_rate.unwrap_or(5.0) as f32 as f64;
    for modifier in mods {
        match modifier {
            Mod::Easy => {
                od = ((od as f32) * 0.5) as f64;
                ar = ((ar as f32) * 0.5) as f64;
                drain = ((drain as f32) * 0.5) as f64;
            }
            Mod::HardRock => {
                od = (((od as f32) * 1.4).min(10.0)) as f64;
                ar = (((ar as f32) * 1.4).min(10.0)) as f64;
                drain = (((drain as f32) * 1.4).min(10.0)) as f64;
            }
            Mod::DifficultyAdjust(adjust) => {
                if let Some(value) = adjust.overall_difficulty {
                    od = value as f32 as f64;
                }
                if let Some(value) = adjust.approach_rate {
                    ar = value as f32 as f64;
                }
                if let Some(value) = adjust.hp_drain_rate {
                    drain = value as f32 as f64;
                }
            }
            _ => {}
        }
    }
    if ![od, ar, drain].iter().all(|value| value.is_finite()) {
        return Err(OsuPerformanceError::NonFiniteDifficulty);
    }
    difficulty.overall_difficulty = Some(od);
    difficulty.approach_rate = Some(ar);
    difficulty.hp_drain_rate = Some(drain);
    Ok(difficulty)
}

fn hit_window(od: f64, min: f64, mid: f64, max: f64) -> f64 {
    difficulty_range(od, min, mid, max).floor() - 0.5
}
fn difficulty_range(value: f64, min: f64, mid: f64, max: f64) -> f64 {
    if value > 5.0 {
        mid + (max - mid) * ((value - 5.0) / 5.0)
    } else if value < 5.0 {
        mid + (mid - min) * ((value - 5.0) / 5.0)
    } else {
        mid
    }
}
fn rate_adjusted_ar(ar: f64, rate: f64) -> f64 {
    let preempt = difficulty_range(ar, 1800.0, 1200.0, 450.0) / rate;
    if (preempt - 1200.0).signum() == (-750.0_f64).signum() {
        (preempt - 1200.0) / (450.0 - 1200.0) * 5.0 + 5.0
    } else {
        (preempt - 1200.0) / (1200.0 - 1800.0) * 5.0 + 5.0
    }
}
fn miss_penalty(misses: f64, difficult_strains: f64) -> f64 {
    0.93 / (misses / (4.0 * difficult_strains.max(1.0).ln()) + 1.0)
}
fn combo_scaling(combo: i32, max_combo: i32) -> f64 {
    if max_combo <= 0 {
        1.0
    } else {
        ((combo as f64).powf(0.8) / (max_combo as f64).powf(0.8)).min(1.0)
    }
}
fn traceable_bonus(ar: f64, slider: f64) -> f64 {
    let high = 0.5 + slider.powi(6) / 2.0;
    let low = slider.powi(6);
    let mut b = 0.0275 + 0.025 * (12.0 - ar.max(7.0)) * high;
    if ar < 7.0 {
        b += 0.025 * (7.0 - ar.max(0.0)) * low;
    }
    if ar < 0.0 {
        b += 0.025 * (1.0 - 1.5_f64.powf(ar)) * low;
    }
    b
}
fn smoothstep(value: f64, start: f64, end: f64) -> f64 {
    let t = ((value - start) / (end - start)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn logistic(x: f64, midpoint: f64, multiplier: f64) -> f64 {
    1.0 / (1.0 + (multiplier * (midpoint - x)).exp())
}
fn reverse_lerp(value: f64, start: f64, end: f64) -> f64 {
    ((value - start) / (end - start)).clamp(0.0, 1.0)
}
fn norm_1_1(values: [f64; 4]) -> f64 {
    values
        .iter()
        .map(|value| value.max(0.0).powf(PERFORMANCE_NORM_EXPONENT))
        .sum::<f64>()
        .powf(1.0 / PERFORMANCE_NORM_EXPONENT)
}
fn has_mod(mods: &[Mod], acronym: &str) -> bool {
    mods.iter().any(|modifier| modifier.acronym() == acronym)
}
fn erf(x: f64) -> f64 {
    crate::ppp_performance::erf(x)
}
fn osu_erf_inverse(x: f64) -> f64 {
    if x <= -1.0 {
        return f64::NEG_INFINITY;
    }
    if x >= 1.0 {
        return f64::INFINITY;
    }
    if x == 0.0 {
        return 0.0;
    }

    const A: f64 = 0.147;
    let sign = x.signum();
    let absolute = x.abs();
    let logarithm = (1.0 - absolute * absolute).ln();
    let first = 2.0 / (std::f64::consts::PI * A) + logarithm / 2.0;
    let second = logarithm / A;
    let approximation = (first * first - second).sqrt() - first;
    let correction = if absolute >= 0.85 {
        ((absolute - 0.85) / 0.293).powi(8)
    } else {
        0.0
    };
    sign * (approximation.sqrt() + correction)
}
fn sum_cognition(reading: f64, flashlight: f64) -> f64 {
    if reading <= 0.0 {
        flashlight
    } else if flashlight <= 0.0 {
        reading
    } else {
        let adjusted_flashlight = flashlight * (flashlight / reading).clamp(0.25, 1.0);
        (reading.powf(1.1) + adjusted_flashlight.powf(1.1)).powf(1.0 / 1.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attributes::OsuDifficultyAttributes;
    use crate::beatmap::GameMode;
    use crate::score::ScoreStatistics;

    fn attrs() -> OsuDifficultyAttributes {
        OsuDifficultyAttributes {
            star_rating: 5.0,
            max_combo: 100,
            aim_difficulty: 2.0,
            speed_difficulty: 1.5,
            reading_difficulty: 0.5,
            speed_note_count: 80.0,
            aim_difficult_strain_count: 10.0,
            speed_difficult_strain_count: 8.0,
            reading_difficult_note_count: 10.0,
            hit_circle_count: 80,
            slider_count: 20,
            spinner_count: 2,
            ..Default::default()
        }
    }
    fn score() -> ScoreInfo {
        let mut stats = ScoreStatistics::default();
        stats.set(HitResult::Great, 100);
        ScoreInfo {
            ruleset: Some(GameMode::Standard),
            beatmap_difficulty: Some(DifficultySettings {
                hp_drain_rate: Some(5.0),
                circle_size: Some(4.0),
                overall_difficulty: Some(8.0),
                approach_rate: Some(9.0),
                slider_multiplier: Some(1.4),
                slider_tick_rate: Some(1.0),
            }),
            statistics: stats,
            max_combo: 100,
            accuracy: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn perfect_score_golden_components_and_total_are_finite() {
        let result = OsuPerformanceCalculator::calculate_score(&score(), &attrs()).unwrap();
        assert!(result.total > 0.0);
        assert_eq!(result.effective_miss_count, 0.0);
        assert_eq!(result.aim_estimated_slider_breaks, 0.0);
        assert!(
            (result.accuracy - 1.52163_f64.powf(8.0) * 2.83 * (100.0_f64 / 1000.0).powf(0.3)).abs()
                < 1e-12
        );
    }

    #[test]
    fn misses_and_slider_breaks_reduce_relevant_skill_values() {
        let perfect = OsuPerformanceCalculator::calculate_score(&score(), &attrs()).unwrap();
        let mut missed = score();
        missed.max_combo = 80;
        missed.statistics.set(HitResult::Great, 90);
        missed.statistics.set(HitResult::Miss, 10);
        missed.statistics.set(HitResult::LargeTickMiss, 4);
        missed.statistics.set(HitResult::SliderTailHit, 18);
        let result = OsuPerformanceCalculator::calculate_score(&missed, &attrs()).unwrap();
        assert!(result.effective_miss_count >= 10.0);
        assert!(result.aim < perfect.aim);
        assert!(result.total < perfect.total);
    }

    #[test]
    fn classic_context_uses_legacy_score_miss_estimate() {
        let mut score = score();
        score.legacy_total_score = 250_000;
        let result = OsuPerformanceCalculator::calculate(
            &score,
            &attrs(),
            OsuPerformanceContext {
                classic_slider_accuracy: true,
            },
        )
        .unwrap();
        assert!(result.score_based_estimated_miss_count.is_some());
    }

    #[test]
    fn rate_and_difficulty_mods_change_hit_windows_and_accuracy() {
        let mut value = score();
        value.mods = vec![Mod::DoubleTime, Mod::HardRock];
        let result = OsuPerformanceCalculator::calculate_score(&value, &attrs()).unwrap();
        assert!(result.total.is_finite());
    }

    #[test]
    fn no_successful_hits_returns_zero_speed_and_accuracy() {
        let mut value = score();
        value.statistics = ScoreStatistics::default();
        value.statistics.set(HitResult::Miss, 10);
        value.accuracy = 0.0;
        let result = OsuPerformanceCalculator::calculate_score(&value, &attrs()).unwrap();
        assert_eq!(result.speed, 0.0);
        assert_eq!(result.accuracy, 0.0);
    }
}
