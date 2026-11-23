//! Versioned binary ABI for direct memory input.

use crate::attributes::{
    AnyDifficultyAttributes, CatchPerformanceAttributes, DifficultyAttributes,
    ManiaPerformanceAttributes, OsuPerformanceAttributes, TaikoPerformanceAttributes,
};
use crate::beatmap::{GameMode, OsuBeatmap};
use crate::calculator::{CalculationError, calculate_difficulty};
use crate::catch_performance::CatchPerformanceCalculator;
use crate::csi_features::{EzChartFeatureExtractor, EzManiaChartInput};
use crate::csi_patterns::{EzPatternAnalysis, EzPatternAnalyzer, LnPatternAxis, RcPatternAxis};
use crate::mania_converter::ManiaBeatmapConverter;
use crate::mania_performance::ManiaPerformanceCalculator;
use crate::mina::MinaEngine;
use crate::mods::{DifficultyAdjustment, Mod};
use crate::normalization::normalize;
use crate::osu_performance::{OsuPerformanceCalculator, OsuPerformanceContext};
use crate::score::{HitResult, ScoreInfo, ScoreStatistics};
use crate::skills::{EzMinaSkillAxis, SkillAxis};
use crate::taiko_performance::TaikoPerformanceCalculator;

pub const OK: i32 = 0;
pub const INVALID_INPUT: i32 = -1;
pub const PARSE_FAILED: i32 = -2;
pub const UNSUPPORTED: i32 = -3;
pub const CALCULATION_FAILED: i32 = -4;
pub const ENGINE_FAILED: i32 = -5;
pub const MAX_CSI_PATTERN_VALUES_V1: usize = 32;
pub const CSI_METRIC_COUNT_V1: usize = 51;

pub const MOD_NO_FAIL: u64 = 1 << 0;
pub const MOD_EASY: u64 = 1 << 1;
pub const MOD_TOUCH_DEVICE: u64 = 1 << 2;
pub const MOD_HIDDEN: u64 = 1 << 3;
pub const MOD_HARD_ROCK: u64 = 1 << 4;
pub const MOD_SUDDEN_DEATH: u64 = 1 << 5;
pub const MOD_PERFECT: u64 = 1 << 6;
pub const MOD_DOUBLE_TIME: u64 = 1 << 7;
pub const MOD_NIGHTCORE: u64 = 1 << 8;
pub const MOD_HALF_TIME: u64 = 1 << 9;
pub const MOD_DAYCORE: u64 = 1 << 10;
pub const MOD_FLASHLIGHT: u64 = 1 << 11;
pub const MOD_RELAX: u64 = 1 << 12;
pub const MOD_AUTOPILOT: u64 = 1 << 13;
pub const MOD_SPUN_OUT: u64 = 1 << 14;
pub const MOD_AUTOPLAY: u64 = 1 << 15;
pub const MOD_SCORE_V2: u64 = 1 << 16;
pub const MOD_MIRROR: u64 = 1 << 17;
pub const MOD_CLASSIC: u64 = 1 << 18;

const SUPPORTED_SCORE_MODS: u64 = (1 << 19) - 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EzChartDifficultyV1 {
    pub star_rating: f64,
    pub max_combo: i32,
    pub ruleset_id: i32,
}

/// Mina axis values in stable order: overall, stream, jumpstream, handstream, stamina, jack, chordjack, tech.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EzMinaAxesV1 {
    pub axes: [f64; 8],
}

/// One CSI pattern result. Pattern IDs are stable ABI values: RC axes use their `RcPatternAxis::ALL`
/// index (0..14), LN axes use `0x100 + LnPatternAxis::ALL` index, and the aggregate `ln` pattern uses `0x200`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EzCsiPatternValueV1 {
    pub pattern_id: u32,
    pub score: f64,
    pub confidence: f64,
}

/// CSI chart analysis result in a fixed, versioned binary layout.
///
/// `metrics` order is `EzChartFeatureMetrics` declaration order: key_count, note_count, duration_ms,
/// hold_ratio, chord_ratio, two_note_chord_ratio, peak_nps_1s, peak_nps_5s, nps_5s_p50,
/// nps_5s_p90, nps_5s_p95, sustained_nps_10s, sustained_nps_30s, sustained_nps_60s,
/// active_nps, long_gap_ratio, long_gap_count, jack_pressure, stream_pressure,
/// jumpstream_pressure, chordjack_pressure, chord_column_overlap_ratio,
/// adjacent_column_rehit_share, two_back_column_rehit_share, two_back_column_rehit_excess,
/// tech_pressure, row_burst_pressure, fast_row_ratio, row_interval_entropy, off_grid_row_share,
/// pattern_variety, row_pattern_entropy, row_pattern_variety, repeated_row_pattern_ratio,
/// alternating_row_pattern_ratio, row_pattern_change_rate, row_motif_repeat_ratio,
/// rhythm_motif_repeat_ratio, adjacent_motif_repeat_ratio, strain_spikiness,
/// sustained_pressure_ratio, anchor_pressure, ln_release_pressure, ln_density,
/// ln_overlap_pressure, ln_chord_pressure, ln_hold_duration_avg, ln_hold_duration_p90,
/// chord_size_change_rate, direction_change_rate, stamina_pressure.
///
/// `selected_patterns` contains the final display selection; `all_patterns` contains every candidate
/// sorted by descending score and stable label tie-break. Unused entries are zero-filled. `warnings`
/// is a bit mask where bit 0 means the source chart contains too little note data.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EzCsiAnalysisV1 {
    pub key_count: u32,
    pub note_count: u32,
    pub duration_ms: f64,
    pub metrics: [f64; CSI_METRIC_COUNT_V1],
    pub selected_pattern_count: u32,
    pub all_pattern_count: u32,
    /// Stable pattern ID, or `u32::MAX` if the analyzer produced no primary pattern.
    pub primary_pattern_id: u32,
    pub warnings: u32,
    pub selected_patterns: [EzCsiPatternValueV1; MAX_CSI_PATTERN_VALUES_V1],
    pub all_patterns: [EzCsiPatternValueV1; MAX_CSI_PATTERN_VALUES_V1],
}

impl Default for EzCsiAnalysisV1 {
    fn default() -> Self {
        Self {
            key_count: 0,
            note_count: 0,
            duration_ms: 0.0,
            metrics: [0.0; CSI_METRIC_COUNT_V1],
            selected_pattern_count: 0,
            all_pattern_count: 0,
            primary_pattern_id: u32::MAX,
            warnings: 0,
            selected_patterns: [EzCsiPatternValueV1::default(); MAX_CSI_PATTERN_VALUES_V1],
            all_patterns: [EzCsiPatternValueV1::default(); MAX_CSI_PATTERN_VALUES_V1],
        }
    }
}

/// Fixed-layout score data for [`ez_score_performance_from_bytes_v1`].
///
/// `statistics` uses [`HitResult::ALL`] order: Perfect, Great, Good, Ok, Meh, Miss, Geki,
/// Katu, LargeTickHit, LargeTickMiss, SmallTickHit, SmallTickMiss, SliderTailHit,
/// SliderTailMiss, IgnoreHit, IgnoreMiss, StrongGreat, StrongOk. `classic_slider_accuracy`
/// is 0 to use the Classic mod default, 1 to force classic slider accuracy, or 2 to disable it.
/// `difficulty_adjustment_mask` selects optional `difficulty_adjustment_values` in HP, CS, OD,
/// AR, slider multiplier, slider tick rate order.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EzScoreInputV1 {
    pub statistics: [i32; HitResult::COUNT],
    pub max_combo: i32,
    pub total_score: i64,
    pub legacy_total_score: i64,
    pub accuracy: f64,
    pub classic_slider_accuracy: u32,
    pub difficulty_adjustment_mask: u32,
    pub difficulty_adjustment_values: [f64; 6],
}

impl Default for EzScoreInputV1 {
    fn default() -> Self {
        Self {
            statistics: [0; HitResult::COUNT],
            max_combo: 0,
            total_score: 0,
            legacy_total_score: 0,
            accuracy: 0.0,
            classic_slider_accuracy: 0,
            difficulty_adjustment_mask: 0,
            difficulty_adjustment_values: [0.0; 6],
        }
    }
}

/// Fixed-layout official score performance result. Fields not used by the selected ruleset are NaN.
///
/// `difficulty_star_rating` and `max_combo` are the calculated chart values. For osu!standard,
/// `aim`, `speed`, `accuracy_pp`, `flashlight`, and `reading` are its component PP values; the
/// auxiliary fields are effective miss count, speed deviation, combo-estimated misses,
/// score-estimated misses, aim slider breaks, and speed slider breaks. For Taiko, `difficulty`
/// and `accuracy_pp` are PP components and `estimated_unstable_rate` is the Taiko UR estimate.
/// Mania reports `difficulty`; Catch reports only `total`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EzScorePerformanceV1 {
    pub ruleset_id: i32,
    pub max_combo: i32,
    pub difficulty_star_rating: f64,
    pub total: f64,
    pub difficulty: f64,
    pub aim: f64,
    pub speed: f64,
    pub accuracy_pp: f64,
    pub flashlight: f64,
    pub reading: f64,
    pub effective_miss_count: f64,
    pub speed_deviation: f64,
    pub combo_based_estimated_miss_count: f64,
    pub score_based_estimated_miss_count: f64,
    pub aim_estimated_slider_breaks: f64,
    pub speed_estimated_slider_breaks: f64,
    pub estimated_unstable_rate: f64,
}

impl Default for EzScorePerformanceV1 {
    fn default() -> Self {
        Self {
            ruleset_id: -1,
            max_combo: 0,
            difficulty_star_rating: f64::NAN,
            total: f64::NAN,
            difficulty: f64::NAN,
            aim: f64::NAN,
            speed: f64::NAN,
            accuracy_pp: f64::NAN,
            flashlight: f64::NAN,
            reading: f64::NAN,
            effective_miss_count: f64::NAN,
            speed_deviation: f64::NAN,
            combo_based_estimated_miss_count: f64::NAN,
            score_based_estimated_miss_count: f64::NAN,
            aim_estimated_slider_breaks: f64::NAN,
            speed_estimated_slider_breaks: f64::NAN,
            estimated_unstable_rate: f64::NAN,
        }
    }
}

/// Calculates the native ruleset's star rating and maximum combo.
///
/// A clock rate of 1 is No Mod. The output is untouched on failure.
///
/// # Safety
/// The caller must provide `length` initialized readable bytes at `input` and a writable,
/// aligned result at `out`. The buffers must not overlap or be concurrently modified.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ez_chart_difficulty_from_bytes_v1(
    input: *const u8,
    length: usize,
    clock_rate: f64,
    out: *mut EzChartDifficultyV1,
) -> i32 {
    if input.is_null()
        || out.is_null()
        || length == 0
        || length > isize::MAX as usize
        || !clock_rate.is_finite()
        || !(0.01..=10.0).contains(&clock_rate)
    {
        return INVALID_INPUT;
    }
    let bytes = unsafe { std::slice::from_raw_parts(input, length) };
    let Ok(beatmap) = OsuBeatmap::parse_bytes(bytes) else {
        return PARSE_FAILED;
    };
    // Match the legacy decoder's hit-object time range before entering strain section loops.
    if beatmap.hit_objects.iter().any(|object| {
        object.start_time.abs() > f64::from(i32::MAX)
            || object.end_time().abs() > f64::from(i32::MAX)
    }) {
        return INVALID_INPUT;
    }
    let rate_mod = [Mod::RateAdjust { rate: clock_rate }];
    let mods = if clock_rate == 1.0 {
        &[][..]
    } else {
        &rate_mod[..]
    };
    let result = match calculate_difficulty(&beatmap, mods) {
        Ok(result) => result,
        Err(CalculationError::UnsupportedObject { .. }) => return UNSUPPORTED,
        Err(_) => return CALCULATION_FAILED,
    };
    if !result.star_rating().is_finite() {
        return CALCULATION_FAILED;
    }
    unsafe {
        out.write(EzChartDifficultyV1 {
            star_rating: result.star_rating(),
            max_combo: result.max_combo(),
            ruleset_id: result.ruleset() as i32,
        });
    }
    OK
}

/// Calculates official score PP directly from a native-ruleset `.osu` byte buffer.
///
/// `mod_flags` uses [`MOD_NO_FAIL`] through [`MOD_CLASSIC`] bit IDs. `rate` is multiplied by any
/// rate-changing mod bits. Difficulty adjustment values are optional and selected by the input
/// mask in HP, CS, OD, AR, slider multiplier, slider tick rate order. `accuracy` is a 0..=1
/// fraction. The output is untouched on every failure.
///
/// # Safety
/// `input` must point to `length` initialized readable bytes, `score` to an aligned initialized
/// [`EzScoreInputV1`], and `out` to an aligned writable [`EzScorePerformanceV1`]. The three buffers
/// must not overlap or be concurrently modified.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ez_score_performance_from_bytes_v1(
    input: *const u8,
    length: usize,
    rate: f64,
    mod_flags: u64,
    score: *const EzScoreInputV1,
    out: *mut EzScorePerformanceV1,
) -> i32 {
    if input.is_null()
        || score.is_null()
        || out.is_null()
        || length == 0
        || length > isize::MAX as usize
        || !rate.is_finite()
        || !(0.01..=10.0).contains(&rate)
        || mod_flags & !SUPPORTED_SCORE_MODS != 0
    {
        return INVALID_INPUT;
    }

    let score_input = unsafe { score.read() };
    if score_input.max_combo < 0
        || score_input.total_score < 0
        || score_input.legacy_total_score < 0
        || !score_input.accuracy.is_finite()
        || !(0.0..=1.0).contains(&score_input.accuracy)
        || score_input.classic_slider_accuracy > 2
        || score_input.difficulty_adjustment_mask & !0x3f != 0
        || score_input.statistics.iter().any(|count| *count < 0)
    {
        return INVALID_INPUT;
    }

    let bytes = unsafe { std::slice::from_raw_parts(input, length) };
    let beatmap = match OsuBeatmap::parse_bytes(bytes) {
        Ok(beatmap) => beatmap,
        Err(_) => return PARSE_FAILED,
    };

    let Some(mods) = mods_with_adjustment(
        mod_flags,
        rate,
        score_input.difficulty_adjustment_mask,
        score_input.difficulty_adjustment_values,
    ) else {
        return INVALID_INPUT;
    };

    let attrs = match calculate_difficulty(&beatmap, &mods) {
        Ok(attrs) => attrs,
        Err(CalculationError::UnsupportedObject { .. }) => return UNSUPPORTED,
        Err(_) => return CALCULATION_FAILED,
    };
    let mut score_info = ScoreInfo {
        ruleset: Some(beatmap.mode),
        beatmap_difficulty: Some(beatmap.difficulty.clone()),
        mods,
        max_combo: score_input.max_combo,
        total_score: score_input.total_score,
        legacy_total_score: score_input.legacy_total_score,
        accuracy: score_input.accuracy,
        ..ScoreInfo::default()
    };
    let mut statistics = ScoreStatistics::default();
    for (result, count) in HitResult::ALL.into_iter().zip(score_input.statistics) {
        statistics.set(result, count);
    }
    score_info.statistics = statistics;

    let mut result = EzScorePerformanceV1 {
        ruleset_id: attrs.ruleset() as i32,
        max_combo: attrs.max_combo(),
        difficulty_star_rating: attrs.star_rating(),
        ..EzScorePerformanceV1::default()
    };
    let calculated = match &attrs {
        AnyDifficultyAttributes::Osu(difficulty) => {
            let classic_slider_accuracy = match score_input.classic_slider_accuracy {
                0 => score_info
                    .mods
                    .iter()
                    .any(|modifier| matches!(modifier, Mod::Classic)),
                1 => true,
                2 => false,
                _ => unreachable!(),
            };
            let Ok(pp) = OsuPerformanceCalculator::calculate(
                &score_info,
                difficulty,
                OsuPerformanceContext {
                    classic_slider_accuracy,
                },
            ) else {
                return CALCULATION_FAILED;
            };
            write_osu_performance(&mut result, pp)
        }
        AnyDifficultyAttributes::Taiko(difficulty) => {
            let Ok(pp) = TaikoPerformanceCalculator::calculate_score(&score_info, difficulty)
            else {
                return CALCULATION_FAILED;
            };
            write_taiko_performance(&mut result, pp)
        }
        AnyDifficultyAttributes::Catch(difficulty) => {
            let Ok(pp) = CatchPerformanceCalculator::calculate_score(&score_info, difficulty)
            else {
                return CALCULATION_FAILED;
            };
            write_catch_performance(&mut result, pp)
        }
        AnyDifficultyAttributes::Mania(difficulty) => {
            let pp = ManiaPerformanceCalculator::calculate(&score_info, difficulty);
            write_mania_performance(&mut result, pp)
        }
    };
    if !calculated || !result.total.is_finite() {
        return CALCULATION_FAILED;
    }

    unsafe { out.write(result) };
    OK
}

fn mods_from_score_flags(flags: u64) -> Vec<Mod> {
    let mut mods = Vec::new();
    for (flag, modifier) in [
        (MOD_NO_FAIL, Mod::NoFail),
        (MOD_EASY, Mod::Easy),
        (MOD_TOUCH_DEVICE, Mod::TouchDevice),
        (MOD_HIDDEN, Mod::Hidden),
        (MOD_HARD_ROCK, Mod::HardRock),
        (MOD_SUDDEN_DEATH, Mod::SuddenDeath),
        (MOD_PERFECT, Mod::Perfect),
        (MOD_DOUBLE_TIME, Mod::DoubleTime),
        (MOD_NIGHTCORE, Mod::Nightcore),
        (MOD_HALF_TIME, Mod::HalfTime),
        (MOD_DAYCORE, Mod::Daycore),
        (MOD_FLASHLIGHT, Mod::Flashlight),
        (MOD_RELAX, Mod::Relax),
        (MOD_AUTOPILOT, Mod::Autopilot),
        (MOD_SPUN_OUT, Mod::SpunOut),
        (MOD_AUTOPLAY, Mod::Autoplay),
        (MOD_SCORE_V2, Mod::ScoreV2),
        (MOD_MIRROR, Mod::Mirror),
        (MOD_CLASSIC, Mod::Classic),
    ] {
        if flags & flag != 0 {
            mods.push(modifier);
        }
    }
    mods
}

fn write_osu_performance(result: &mut EzScorePerformanceV1, pp: OsuPerformanceAttributes) -> bool {
    result.total = pp.total;
    result.aim = pp.aim;
    result.speed = pp.speed;
    result.accuracy_pp = pp.accuracy;
    result.flashlight = pp.flashlight;
    result.reading = pp.reading;
    result.effective_miss_count = pp.effective_miss_count;
    result.speed_deviation = pp.speed_deviation.unwrap_or(f64::NAN);
    result.combo_based_estimated_miss_count = pp.combo_based_estimated_miss_count;
    result.score_based_estimated_miss_count =
        pp.score_based_estimated_miss_count.unwrap_or(f64::NAN);
    result.aim_estimated_slider_breaks = pp.aim_estimated_slider_breaks;
    result.speed_estimated_slider_breaks = pp.speed_estimated_slider_breaks;
    result.total.is_finite()
        && result.aim.is_finite()
        && result.speed.is_finite()
        && result.accuracy_pp.is_finite()
        && result.flashlight.is_finite()
        && result.reading.is_finite()
}

fn write_taiko_performance(
    result: &mut EzScorePerformanceV1,
    pp: TaikoPerformanceAttributes,
) -> bool {
    result.total = pp.total;
    result.difficulty = pp.difficulty;
    result.accuracy_pp = pp.accuracy;
    result.estimated_unstable_rate = pp.estimated_unstable_rate.unwrap_or(f64::NAN);
    result.total.is_finite() && result.difficulty.is_finite() && result.accuracy_pp.is_finite()
}

fn write_catch_performance(
    result: &mut EzScorePerformanceV1,
    pp: CatchPerformanceAttributes,
) -> bool {
    result.total = pp.total;
    result.total.is_finite()
}

fn write_mania_performance(
    result: &mut EzScorePerformanceV1,
    pp: ManiaPerformanceAttributes,
) -> bool {
    result.total = pp.total;
    result.difficulty = pp.difficulty;
    result.total.is_finite() && result.difficulty.is_finite()
}

fn mods_with_adjustment(
    flags: u64,
    rate: f64,
    adjustment_mask: u32,
    adjustment_values: [f64; 6],
) -> Option<Vec<Mod>> {
    if adjustment_mask & !0x3f != 0 {
        return None;
    }
    let mut mods = mods_from_score_flags(flags);
    if rate != 1.0 {
        mods.push(Mod::RateAdjust { rate });
    }
    if adjustment_mask != 0 {
        let mut value = DifficultyAdjustment::default();
        let slots = [
            &mut value.hp_drain_rate,
            &mut value.circle_size,
            &mut value.overall_difficulty,
            &mut value.approach_rate,
            &mut value.slider_multiplier,
            &mut value.slider_tick_rate,
        ];
        for (index, slot) in slots.into_iter().enumerate() {
            if adjustment_mask & (1 << index) != 0 {
                if !adjustment_values[index].is_finite() {
                    return None;
                }
                *slot = Some(adjustment_values[index]);
            }
        }
        mods.push(Mod::DifficultyAdjust(value));
    }
    Some(mods)
}

/// Calculates the chart-side Mina skill axes from a native Mania `.osu` byte buffer.
///
/// # Safety
/// `input` must reference `length` initialized readable bytes and `out` must be an aligned writable
/// `EzMinaAxesV1`. The buffers must not overlap or be concurrently modified.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ez_mina_chart_axes_from_bytes_v1(
    input: *const u8,
    length: usize,
    rate: f32,
    out: *mut EzMinaAxesV1,
) -> i32 {
    if input.is_null() || out.is_null() || length == 0 || length > isize::MAX as usize {
        return INVALID_INPUT;
    }
    let bytes = unsafe { std::slice::from_raw_parts(input, length) };
    let beatmap = match OsuBeatmap::parse_bytes(bytes) {
        Ok(beatmap) => beatmap,
        Err(_) => return PARSE_FAILED,
    };
    if beatmap.mode != crate::beatmap::GameMode::Mania {
        return UNSUPPORTED;
    }
    let normalized = match normalize(&beatmap) {
        Ok(map) => map,
        Err(_) => return PARSE_FAILED,
    };
    let (notes, keys) = match ManiaBeatmapConverter::convert(&normalized, &[]) {
        Ok(result) => result,
        Err(_) => return CALCULATION_FAILED,
    };
    let engine = match MinaEngine::new() {
        Ok(engine) => engine,
        Err(_) => return ENGINE_FAILED,
    };
    let vector = match engine.calculate_chart(&notes, keys, rate) {
        Ok(vector) => vector,
        Err(_) => return ENGINE_FAILED,
    };
    let axes = std::array::from_fn(|index| vector.get(EzMinaSkillAxis::ALL[index]));
    unsafe {
        out.write(EzMinaAxesV1 { axes });
    }
    OK
}

/// Calculates Mania CSI features and RC/LN pattern classification from `.osu` bytes.
///
/// The caller supplies the representative beatmap BPM because osu! files store timing points, not
/// the metadata BPM value used by Ez2Lazer's chart analyzer. Rate 1.0 is No Mod. The output is
/// untouched on failure. Stable numeric pattern IDs and metric ordering are documented on
/// [`EzCsiPatternValueV1`] and [`EzCsiAnalysisV1`].
///
/// # Safety
/// `input` must reference `length` initialized readable bytes and `out` must be an aligned writable
/// `EzCsiAnalysisV1`. The buffers must not overlap or be concurrently modified.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ez_csi_chart_analysis_from_bytes_v1(
    input: *const u8,
    length: usize,
    bpm: f64,
    rate: f64,
    out: *mut EzCsiAnalysisV1,
) -> i32 {
    if input.is_null()
        || out.is_null()
        || length == 0
        || length > isize::MAX as usize
        || !bpm.is_finite()
        || bpm <= 0.0
        || !rate.is_finite()
        || rate <= 0.0
    {
        return INVALID_INPUT;
    }
    let bytes = unsafe { std::slice::from_raw_parts(input, length) };
    let beatmap = match OsuBeatmap::parse_bytes(bytes) {
        Ok(beatmap) => beatmap,
        Err(_) => return PARSE_FAILED,
    };
    if beatmap.mode != GameMode::Mania {
        return UNSUPPORTED;
    }
    let normalized = match normalize(&beatmap) {
        Ok(map) => map,
        Err(_) => return PARSE_FAILED,
    };
    let (notes, key_count) = match ManiaBeatmapConverter::convert(&normalized, &[]) {
        Ok(result) => result,
        Err(_) => return CALCULATION_FAILED,
    };
    let timing_points: Vec<_> = normalized
        .timing_points
        .iter()
        .filter(|point| point.uninherited)
        .map(|point| (point.time, point.beat_length))
        .collect();
    let total_length_ms = notes
        .iter()
        .map(|note| note.end_time_ms)
        .fold(0.0_f64, f64::max);
    let input_map = EzManiaChartInput {
        key_count,
        bpm,
        total_length_ms,
        notes: &notes,
        timing_points: &timing_points,
    };
    let features = match EzChartFeatureExtractor::extract(input_map, rate) {
        Ok(features) => features,
        Err(_) => return CALCULATION_FAILED,
    };
    let analysis = EzPatternAnalyzer::analyze_features(input_map, rate, &features);
    let result = match csi_analysis_v1(&analysis) {
        Some(result) => result,
        None => return CALCULATION_FAILED,
    };
    unsafe { out.write(result) };
    OK
}

fn csi_analysis_v1(analysis: &EzPatternAnalysis) -> Option<EzCsiAnalysisV1> {
    let metrics = &analysis.metrics;
    let metric_values = [
        metrics.key_count as f64,
        metrics.note_count as f64,
        metrics.duration_ms,
        metrics.hold_ratio,
        metrics.chord_ratio,
        metrics.two_note_chord_ratio,
        metrics.peak_nps_1s,
        metrics.peak_nps_5s,
        metrics.nps_5s_p50,
        metrics.nps_5s_p90,
        metrics.nps_5s_p95,
        metrics.sustained_nps_10s,
        metrics.sustained_nps_30s,
        metrics.sustained_nps_60s,
        metrics.active_nps,
        metrics.long_gap_ratio,
        metrics.long_gap_count as f64,
        metrics.jack_pressure,
        metrics.stream_pressure,
        metrics.jumpstream_pressure,
        metrics.chordjack_pressure,
        metrics.chord_column_overlap_ratio,
        metrics.adjacent_column_rehit_share,
        metrics.two_back_column_rehit_share,
        metrics.two_back_column_rehit_excess,
        metrics.tech_pressure,
        metrics.row_burst_pressure,
        metrics.fast_row_ratio,
        metrics.row_interval_entropy,
        metrics.off_grid_row_share,
        metrics.pattern_variety,
        metrics.row_pattern_entropy,
        metrics.row_pattern_variety,
        metrics.repeated_row_pattern_ratio,
        metrics.alternating_row_pattern_ratio,
        metrics.row_pattern_change_rate,
        metrics.row_motif_repeat_ratio,
        metrics.rhythm_motif_repeat_ratio,
        metrics.adjacent_motif_repeat_ratio,
        metrics.strain_spikiness,
        metrics.sustained_pressure_ratio,
        metrics.anchor_pressure,
        metrics.ln_release_pressure,
        metrics.ln_density,
        metrics.ln_overlap_pressure,
        metrics.ln_chord_pressure,
        metrics.ln_hold_duration_avg,
        metrics.ln_hold_duration_p90,
        metrics.chord_size_change_rate,
        metrics.direction_change_rate,
        metrics.stamina_pressure,
    ];
    if metric_values.iter().any(|value| !value.is_finite())
        || analysis.all_patterns.len() > MAX_CSI_PATTERN_VALUES_V1
        || analysis.patterns.len() > MAX_CSI_PATTERN_VALUES_V1
    {
        return None;
    }
    let mut result = EzCsiAnalysisV1 {
        key_count: u32::try_from(analysis.key_count).ok()?,
        note_count: u32::try_from(metrics.note_count).ok()?,
        duration_ms: metrics.duration_ms,
        metrics: metric_values,
        selected_pattern_count: u32::try_from(analysis.patterns.len()).ok()?,
        all_pattern_count: u32::try_from(analysis.all_patterns.len()).ok()?,
        primary_pattern_id: analysis
            .primary
            .as_ref()
            .and_then(|pattern| csi_pattern_id(pattern.id))
            .unwrap_or(u32::MAX),
        warnings: u32::from(!analysis.warnings.is_empty()),
        ..EzCsiAnalysisV1::default()
    };
    for (target, pattern) in result.selected_patterns.iter_mut().zip(&analysis.patterns) {
        *target = csi_pattern_value(pattern.id, pattern.score, pattern.confidence)?;
    }
    for (target, pattern) in result.all_patterns.iter_mut().zip(&analysis.all_patterns) {
        *target = csi_pattern_value(pattern.id, pattern.score, pattern.confidence)?;
    }
    Some(result)
}

fn csi_pattern_value(id: &str, score: f64, confidence: f64) -> Option<EzCsiPatternValueV1> {
    Some(EzCsiPatternValueV1 {
        pattern_id: csi_pattern_id(id)?,
        score,
        confidence,
    })
}

fn csi_pattern_id(id: &str) -> Option<u32> {
    if id == "ln" {
        return Some(0x200);
    }
    if let Some(axis) = RcPatternAxis::try_parse(id) {
        return u32::try_from(
            RcPatternAxis::ALL
                .iter()
                .position(|candidate| *candidate == axis)?,
        )
        .ok();
    }
    let axis = LnPatternAxis::try_parse(id)?;
    u32::try_from(
        0x100_usize
            + LnPatternAxis::ALL
                .iter()
                .position(|candidate| *candidate == axis)?,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_mania_bytes_and_preserves_output_on_parse_failure() {
        let mut output = EzChartDifficultyV1 {
            star_rating: -1.0,
            max_combo: -1,
            ruleset_id: -1,
        };
        let fixture = include_bytes!("../tests/fixtures/mania-diffcalc-test.osu");
        let status = unsafe {
            ez_chart_difficulty_from_bytes_v1(fixture.as_ptr(), fixture.len(), 1.5, &mut output)
        };
        assert_eq!(status, OK);
        assert_eq!(output.ruleset_id, 3);
        assert_eq!(output.max_combo, 242);
        assert!((output.star_rating / 2.797245912537965 - 1.0).abs() <= 2e-5);

        let before = output;
        let invalid = b"not an osu file";
        let status = unsafe {
            ez_chart_difficulty_from_bytes_v1(invalid.as_ptr(), invalid.len(), 1.0, &mut output)
        };
        assert_eq!(status, PARSE_FAILED);
        assert_eq!(output, before);
    }

    #[test]
    fn mina_binary_abi_matches_vendored_chart_axis_reference() {
        let mut bytes = String::from(
            "osu file format v14\n[General]\nMode:3\n[Difficulty]\nCircleSize:4\n[HitObjects]\n",
        );
        for row in 0..64 {
            let column = row % 4;
            let x = (column * 128 + 64) as i32;
            let time = row * 50;
            bytes.push_str(&format!("{x},192,{time},1,0\n"));
        }
        let mut result = EzMinaAxesV1::default();
        let status = unsafe {
            ez_mina_chart_axes_from_bytes_v1(bytes.as_ptr(), bytes.len(), 1.0, &mut result)
        };
        assert_eq!(status, OK);
        assert!((result.axes[0] - 13.2229395).abs() < 0.25);
        assert!((result.axes[1] - 12.41642).abs() < 0.25);
        assert!((result.axes[7] - 12.248576).abs() < 0.25);
    }

    #[test]
    fn csi_binary_abi_exposes_fixed_metric_and_pattern_layout() {
        let fixture = include_bytes!("../tests/fixtures/mania-diffcalc-test.osu");
        let mut result = EzCsiAnalysisV1::default();
        let status = unsafe {
            ez_csi_chart_analysis_from_bytes_v1(
                fixture.as_ptr(),
                fixture.len(),
                120.0,
                1.0,
                &mut result,
            )
        };
        assert_eq!(status, OK);
        assert_eq!(result.key_count, 4);
        assert_eq!(result.metrics[0], f64::from(result.key_count));
        assert_eq!(result.metrics[1], f64::from(result.note_count));
        assert!(result.duration_ms.is_finite());
        assert!(result.metrics.iter().all(|value| value.is_finite()));
        assert!(result.all_pattern_count as usize <= MAX_CSI_PATTERN_VALUES_V1);
        assert!(result.selected_pattern_count as usize <= MAX_CSI_PATTERN_VALUES_V1);
        assert_eq!(result.all_patterns[0].pattern_id, result.primary_pattern_id);

        let before = result;
        let invalid_rate = unsafe {
            ez_csi_chart_analysis_from_bytes_v1(
                fixture.as_ptr(),
                fixture.len(),
                120.0,
                0.0,
                &mut result,
            )
        };
        assert_eq!(invalid_rate, INVALID_INPUT);
        assert_eq!(result, before);
    }

    #[test]
    fn csi_binary_abi_rejects_other_rulesets_without_touching_output() {
        let bytes =
            b"osu file format v14\n[General]\nMode:0\n[Difficulty]\nCircleSize:4\n[HitObjects]\n";
        let mut result = EzCsiAnalysisV1::default();
        result.key_count = u32::MAX;
        let before = result;
        let status = unsafe {
            ez_csi_chart_analysis_from_bytes_v1(
                bytes.as_ptr(),
                bytes.len(),
                120.0,
                1.0,
                &mut result,
            )
        };
        assert_eq!(status, UNSUPPORTED);
        assert_eq!(result, before);
    }

    #[test]
    fn score_binary_abi_calculates_all_four_native_rulesets() {
        let cases: [(&[u8], i32); 4] = [
            (
                include_bytes!("../tests/fixtures/osu-diffcalc-test.osu"),
                GameMode::Standard as i32,
            ),
            (
                include_bytes!("../tests/fixtures/taiko-diffcalc-test.osu"),
                GameMode::Taiko as i32,
            ),
            (
                include_bytes!("../tests/fixtures/catch-diffcalc-test.osu"),
                GameMode::Catch as i32,
            ),
            (
                include_bytes!("../tests/fixtures/mania-diffcalc-test.osu"),
                GameMode::Mania as i32,
            ),
        ];

        for (bytes, ruleset_id) in cases {
            let mut score = EzScoreInputV1 {
                max_combo: 1000,
                total_score: 1_000_000,
                legacy_total_score: 1_000_000,
                accuracy: 1.0,
                ..EzScoreInputV1::default()
            };
            score.statistics[HitResult::Great as usize] = 64;
            score.statistics[HitResult::Perfect as usize] = 64;
            let mut result = EzScorePerformanceV1::default();

            let status = unsafe {
                ez_score_performance_from_bytes_v1(
                    bytes.as_ptr(),
                    bytes.len(),
                    1.0,
                    0,
                    &score,
                    &mut result,
                )
            };

            assert_eq!(status, OK, "expected ruleset {ruleset_id}");
            assert_eq!(result.ruleset_id, ruleset_id);
            assert!(result.difficulty_star_rating.is_finite());
            assert!(result.total.is_finite() && result.total >= 0.0);
        }
    }

    #[test]
    fn score_binary_abi_applies_mods_rate_and_difficulty_adjustments() {
        let bytes = include_bytes!("../tests/fixtures/osu-diffcalc-test.osu");
        let mut score = EzScoreInputV1 {
            max_combo: 100,
            accuracy: 0.98,
            difficulty_adjustment_mask: 1 << 2,
            difficulty_adjustment_values: [0.0, 0.0, 7.0, 0.0, 0.0, 0.0],
            ..EzScoreInputV1::default()
        };
        score.statistics[HitResult::Great as usize] = 64;
        let mut result = EzScorePerformanceV1::default();
        let status = unsafe {
            ez_score_performance_from_bytes_v1(
                bytes.as_ptr(),
                bytes.len(),
                1.1,
                MOD_DOUBLE_TIME | MOD_HIDDEN,
                &score,
                &mut result,
            )
        };
        assert_eq!(status, OK);
        assert!(result.total.is_finite());
    }

    #[test]
    fn score_binary_abi_rejects_invalid_data_without_touching_output() {
        let bytes = include_bytes!("../tests/fixtures/osu-diffcalc-test.osu");
        let mut score = EzScoreInputV1 {
            accuracy: f64::NAN,
            ..EzScoreInputV1::default()
        };
        let mut output = EzScorePerformanceV1 {
            ruleset_id: 42,
            total: 123.0,
            ..EzScorePerformanceV1::default()
        };
        let status = unsafe {
            ez_score_performance_from_bytes_v1(
                bytes.as_ptr(),
                bytes.len(),
                1.0,
                0,
                &score,
                &mut output,
            )
        };
        assert_eq!(status, INVALID_INPUT);
        assert_eq!(output.ruleset_id, 42);
        assert_eq!(output.total, 123.0);

        score.accuracy = 1.0;
        let status = unsafe {
            ez_score_performance_from_bytes_v1(
                bytes.as_ptr(),
                bytes.len(),
                1.0,
                1 << 63,
                &score,
                &mut output,
            )
        };
        assert_eq!(status, INVALID_INPUT);
        assert_eq!(output.ruleset_id, 42);
        assert_eq!(output.total, 123.0);
    }
}
