//! Versioned binary ABI for direct memory input.

use crate::attributes::DifficultyAttributes;
use crate::beatmap::OsuBeatmap;
use crate::calculator::{CalculationError, calculate_difficulty};
use crate::mania_converter::ManiaBeatmapConverter;
use crate::mina::MinaEngine;
use crate::mods::Mod;
use crate::normalization::normalize;
use crate::skills::{EzMinaSkillAxis, SkillAxis};

pub const OK: i32 = 0;
pub const INVALID_INPUT: i32 = -1;
pub const PARSE_FAILED: i32 = -2;
pub const UNSUPPORTED: i32 = -3;
pub const CALCULATION_FAILED: i32 = -4;
pub const ENGINE_FAILED: i32 = -5;

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
}
