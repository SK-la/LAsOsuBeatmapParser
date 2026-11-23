//! Fixed-layout C ABI for cross-ruleset beatmap difficulty from `.osu` bytes.

use crate::attributes::DifficultyAttributes;
use crate::beatmap::{GameMode, OsuBeatmap};
use crate::calculator::{CalculationError, DifficultyCalculator};
use crate::catch_converter::CatchDifficultyCalculator;
use crate::mania::ManiaDifficultyCalculator;
use crate::mods::Mod;
use crate::osu_converter::OsuDifficultyCalculator;
use crate::taiko_converter::TaikoBeatmapDifficultyCalculator;

pub const CONVERSION_OK: i32 = 0;
pub const CONVERSION_INVALID_INPUT: i32 = -1;
pub const CONVERSION_PARSE_FAILED: i32 = -2;
pub const CONVERSION_UNSUPPORTED: i32 = -3;
pub const CONVERSION_CALCULATION_FAILED: i32 = -4;

/// Version 1 chart difficulty result for a requested target ruleset.
///
/// Ruleset IDs match `.osu` / Ez2Lazer IDs: Standard=0, Taiko=1, Catch=2, Mania=3.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EzTargetChartDifficultyV1 {
    pub star_rating: f64,
    pub max_combo: i32,
    pub source_ruleset_id: i32,
    pub target_ruleset_id: i32,
}

/// Calculates chart difficulty for `target_ruleset_id` from an in-memory `.osu` byte buffer.
///
/// `clock_rate` is the playback-rate multiplier and 1.0 means no rate change. Cross-ruleset
/// support follows each ruleset converter's input contract. The result is left untouched for all
/// invalid, unsupported, parse, or calculation failures.
///
/// # Safety
/// `input` must point to `length` initialized readable bytes and `out` must be an aligned writable
/// [`EzTargetChartDifficultyV1`]. The buffers must not overlap or be concurrently modified.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ez_chart_difficulty_for_ruleset_from_bytes_v1(
    input: *const u8,
    length: usize,
    target_ruleset_id: i32,
    clock_rate: f64,
    out: *mut EzTargetChartDifficultyV1,
) -> i32 {
    if input.is_null()
        || out.is_null()
        || length == 0
        || length > isize::MAX as usize
        || !clock_rate.is_finite()
        || !(0.01..=10.0).contains(&clock_rate)
    {
        return CONVERSION_INVALID_INPUT;
    }

    let target = match GameMode::try_from(target_ruleset_id) {
        Ok(mode) => mode,
        Err(_) => return CONVERSION_INVALID_INPUT,
    };
    let bytes = unsafe { std::slice::from_raw_parts(input, length) };
    let beatmap = match OsuBeatmap::parse_bytes(bytes) {
        Ok(beatmap) => beatmap,
        Err(_) => return CONVERSION_PARSE_FAILED,
    };

    // Keep conversion behavior in the typed calculator implementations; the ABI only selects the
    // target ruleset and adapts the fixed-layout result.
    let rate_mod = [Mod::RateAdjust { rate: clock_rate }];
    let mods = if clock_rate == 1.0 {
        &[][..]
    } else {
        &rate_mod[..]
    };
    let result = match target {
        GameMode::Standard => OsuDifficultyCalculator
            .calculate(&beatmap, mods)
            .map(|attrs| EzTargetChartDifficultyV1 {
                star_rating: attrs.star_rating(),
                max_combo: attrs.max_combo(),
                source_ruleset_id: beatmap.mode as i32,
                target_ruleset_id: target as i32,
            }),
        GameMode::Taiko => TaikoBeatmapDifficultyCalculator
            .calculate(&beatmap, mods)
            .map(|attrs| EzTargetChartDifficultyV1 {
                star_rating: attrs.star_rating(),
                max_combo: attrs.max_combo(),
                source_ruleset_id: beatmap.mode as i32,
                target_ruleset_id: target as i32,
            }),
        GameMode::Catch => CatchDifficultyCalculator
            .calculate(&beatmap, mods)
            .map(|attrs| EzTargetChartDifficultyV1 {
                star_rating: attrs.star_rating(),
                max_combo: attrs.max_combo(),
                source_ruleset_id: beatmap.mode as i32,
                target_ruleset_id: target as i32,
            }),
        GameMode::Mania => ManiaDifficultyCalculator
            .calculate(&beatmap, mods)
            .map(|attrs| EzTargetChartDifficultyV1 {
                star_rating: attrs.star_rating(),
                max_combo: attrs.max_combo(),
                source_ruleset_id: beatmap.mode as i32,
                target_ruleset_id: target as i32,
            }),
    };

    match result {
        Ok(result) if result.star_rating.is_finite() && result.max_combo >= 0 => {
            unsafe { out.write(result) };
            CONVERSION_OK
        }
        Ok(_) => CONVERSION_CALCULATION_FAILED,
        Err(CalculationError::WrongRuleset { .. } | CalculationError::UnsupportedObject { .. }) => {
            CONVERSION_UNSUPPORTED
        }
        Err(_) => CONVERSION_CALCULATION_FAILED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calculate(bytes: &[u8], target: GameMode) -> (i32, EzTargetChartDifficultyV1) {
        let mut result = EzTargetChartDifficultyV1 {
            star_rating: -123.0,
            max_combo: -123,
            source_ruleset_id: -123,
            target_ruleset_id: -123,
        };
        let status = unsafe {
            ez_chart_difficulty_for_ruleset_from_bytes_v1(
                bytes.as_ptr(),
                bytes.len(),
                target as i32,
                1.0,
                &mut result,
            )
        };
        (status, result)
    }

    #[test]
    fn converts_standard_sources_to_mania_and_catch() {
        let standard = include_bytes!("../tests/fixtures/osu-diffcalc-test.osu");
        for target in [GameMode::Mania, GameMode::Catch] {
            let (status, result) = calculate(standard, target);
            assert_eq!(status, CONVERSION_OK);
            assert_eq!(result.source_ruleset_id, GameMode::Standard as i32);
            assert_eq!(result.target_ruleset_id, target as i32);
            assert!(result.star_rating.is_finite() && result.star_rating > 0.0);
            assert!(result.max_combo > 0);
        }
    }

    #[test]
    fn converts_catch_sources_to_standard_and_taiko_and_keeps_native_mode() {
        let catch = include_bytes!("../tests/fixtures/catch-diffcalc-test.osu");
        for target in [GameMode::Standard, GameMode::Taiko, GameMode::Catch] {
            let (status, result) = calculate(catch, target);
            assert_eq!(status, CONVERSION_OK);
            assert_eq!(result.source_ruleset_id, GameMode::Catch as i32);
            assert_eq!(result.target_ruleset_id, target as i32);
            assert!(result.star_rating.is_finite());
        }
    }

    #[test]
    fn rejects_unsupported_sources_and_leaves_output_untouched() {
        let taiko = b"osu file format v14\n[General]\nMode:1\n[Difficulty]\nCircleSize:4\n[HitObjects]\n64,192,1000,1,0\n";
        let mut output = EzTargetChartDifficultyV1 {
            star_rating: 7.0,
            max_combo: 123,
            source_ruleset_id: 42,
            target_ruleset_id: 43,
        };
        let before = output;
        let status = unsafe {
            ez_chart_difficulty_for_ruleset_from_bytes_v1(
                taiko.as_ptr(),
                taiko.len(),
                GameMode::Standard as i32,
                1.0,
                &mut output,
            )
        };
        assert_eq!(status, CONVERSION_UNSUPPORTED);
        assert_eq!(output, before);
    }

    #[test]
    fn validates_arguments_parse_errors_and_preserves_output() {
        let bytes = include_bytes!("../tests/fixtures/osu-diffcalc-test.osu");
        let mut output = EzTargetChartDifficultyV1 {
            star_rating: 7.0,
            max_combo: 123,
            source_ruleset_id: 42,
            target_ruleset_id: 43,
        };
        let before = output;

        let null_input = unsafe {
            ez_chart_difficulty_for_ruleset_from_bytes_v1(
                std::ptr::null(),
                bytes.len(),
                GameMode::Standard as i32,
                1.0,
                &mut output,
            )
        };
        assert_eq!(null_input, CONVERSION_INVALID_INPUT);
        assert_eq!(output, before);

        let invalid_rate = unsafe {
            ez_chart_difficulty_for_ruleset_from_bytes_v1(
                bytes.as_ptr(),
                bytes.len(),
                GameMode::Standard as i32,
                f64::NAN,
                &mut output,
            )
        };
        assert_eq!(invalid_rate, CONVERSION_INVALID_INPUT);
        assert_eq!(output, before);

        let invalid_target = unsafe {
            ez_chart_difficulty_for_ruleset_from_bytes_v1(
                bytes.as_ptr(),
                bytes.len(),
                5,
                1.0,
                &mut output,
            )
        };
        assert_eq!(invalid_target, CONVERSION_INVALID_INPUT);
        assert_eq!(output, before);

        let malformed = b"not a beatmap";
        let parse_error = unsafe {
            ez_chart_difficulty_for_ruleset_from_bytes_v1(
                malformed.as_ptr(),
                malformed.len(),
                GameMode::Standard as i32,
                1.0,
                &mut output,
            )
        };
        assert_eq!(parse_error, CONVERSION_PARSE_FAILED);
        assert_eq!(output, before);

        let null_output = unsafe {
            ez_chart_difficulty_for_ruleset_from_bytes_v1(
                bytes.as_ptr(),
                bytes.len(),
                GameMode::Standard as i32,
                1.0,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(null_output, CONVERSION_INVALID_INPUT);
    }
}
