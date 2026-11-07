//! Checks the C ABI contract and the properties the host depends on: the notes it passes are the notes rated,
//! the cross matrix is the caller's, and every component is a real share of the rating.

use crate::cross_matrix::CrossMatrixProvider;
use crate::ffi::*;
use crate::sr::{SrConfig, SRCalculator, COMPONENT_COUNT};

fn stream(keys: i32, rows: i32) -> Vec<EzSrNote> {
    let mut notes = Vec::new();
    let mut time = 500;

    for row in 0..rows {
        notes.push(EzSrNote { col: row % keys, head: time, tail: -1 });
        time += 120;
    }

    notes
}

fn hold_chart(keys: i32, rows: i32) -> Vec<EzSrNote> {
    let mut notes = Vec::new();
    let mut time = 500;

    for row in 0..rows {
        notes.push(EzSrNote { col: row % keys, head: time, tail: time + 300 });
        time += 140;
    }

    notes
}

/// Dense chorded chart: exercises the high-end rescale, which only engages above the threshold.
fn dense_chart(keys: i32, rows: i32) -> Vec<EzSrNote> {
    let mut notes = Vec::new();
    let mut time = 500;
    let mut seed: i64 = 12_345;

    for _row in 0..rows {
        for _note in 0..keys {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let col = (seed >> 16).rem_euclid(keys as i64) as i32;
            notes.push(EzSrNote { col, head: time, tail: time + 120 });
        }

        time += 90;
    }

    notes
}

fn call(notes: &[EzSrNote], keys: i32, od: f64) -> (i32, EzSrResult) {
    let cross = CrossMatrixProvider::get_matrix(keys).expect("supported key count");
    let mut out = EzSrResult { sr: 0.0, components: [0.0; COMPONENT_COUNT], component_count: 0 };

    let code = unsafe {
        ez_sr_calculate(
            notes.as_ptr(),
            notes.len(),
            keys,
            od,
            cross.as_ptr(),
            EzSrConfig { rescale_threshold: 8.0, ln_multiplier: 5.5 },
            &mut out,
        )
    };

    (code, out)
}

fn rate_adjust(notes: &[EzSrNote], rate: f64) -> Vec<EzSrNote> {
    notes
        .iter()
        .map(|n| EzSrNote {
            col: n.col,
            head: (n.head as f64 / rate).round() as i32,
            tail: if n.tail < 0 { -1 } else { (n.tail as f64 / rate).round() as i32 },
        })
        .collect()
}

#[test]
fn every_supported_key_count_rates() {
    for keys in 1..=18 {
        let notes = stream(keys, 64);
        let (code, out) = call(&notes, keys, 8.0);

        assert_eq!(code, EZ_SR_OK, "{keys}k returned {code}");
        assert!(out.sr > 0.0, "{keys}k produced a zero rating");
        assert_eq!(out.component_count as usize, COMPONENT_COUNT);
    }
}

#[test]
fn odd_key_counts_use_the_borrowed_tables() {
    // 11 / 13 / 15 / 17 are rated by the neighbour-inferred tables the host also stores; rejecting them would
    // make charts un-rateable that the host can rate.
    for keys in [11, 13, 15, 17] {
        let notes = stream(keys, 64);
        let (code, out) = call(&notes, keys, 8.0);

        assert_eq!(code, EZ_SR_OK, "{keys}k returned {code}");
        assert!(out.sr > 0.0, "{keys}k produced a zero rating");
    }
}

#[test]
fn unsupported_input_is_reported_without_touching_output() {
    let notes = stream(4, 32);
    let cross = CrossMatrixProvider::get_matrix(4).unwrap();
    let mut out = EzSrResult { sr: 7.0, components: [7.0; COMPONENT_COUNT], component_count: 9 };

    let code = unsafe {
        ez_sr_calculate(
            notes.as_ptr(),
            notes.len(),
            19,
            8.0,
            cross.as_ptr(),
            EzSrConfig { rescale_threshold: 8.0, ln_multiplier: 5.5 },
            &mut out,
        )
    };

    assert_eq!(code, EZ_SR_ERR_KEY_COUNT);
    assert_eq!(out.sr, 7.0, "a failed call must not write output");
}

#[test]
fn zero_length_hold_is_treated_as_a_tap() {
    let mut notes = stream(4, 32);

    for note in notes.iter_mut() {
        note.tail = note.head;
    }

    let zero_length = call(&notes, 4, 8.0);
    let taps = call(&stream(4, 32), 4, 8.0);

    assert_eq!(zero_length.0, EZ_SR_OK);
    assert!(
        (zero_length.1.sr - taps.1.sr).abs() < 1e-9,
        "a zero-length hold must rate exactly like a tap: {} vs {}",
        zero_length.1.sr,
        taps.1.sr
    );
}

#[test]
fn clock_rate_scales_the_rated_times() {
    let notes = stream(4, 128);

    let normal = call(&notes, 4, 8.0);
    let faster = call(&rate_adjust(&notes, 1.5), 4, 8.0);

    assert_eq!(normal.0, EZ_SR_OK);
    assert_eq!(faster.0, EZ_SR_OK);
    assert!(faster.1.sr > normal.1.sr, "1.5x should rate higher than 1.0x");
}

#[test]
fn the_cross_matrix_argument_is_actually_used() {
    let notes = stream(4, 64);
    let mut out = EzSrResult { sr: 0.0, components: [0.0; COMPONENT_COUNT], component_count: 0 };

    let normal = CrossMatrixProvider::get_matrix(4).unwrap().to_vec();
    let mut flat = normal.clone();
    for value in flat.iter_mut() {
        *value = 0.0;
    }

    let config = EzSrConfig { rescale_threshold: 8.0, ln_multiplier: 5.5 };

    let base = unsafe {
        ez_sr_calculate(notes.as_ptr(), notes.len(), 4, 8.0, normal.as_ptr(), config, &mut out)
    };
    assert_eq!(base, EZ_SR_OK);
    let base_sr = out.sr;

    let flat_code = unsafe {
        ez_sr_calculate(notes.as_ptr(), notes.len(), 4, 8.0, flat.as_ptr(), config, &mut out)
    };
    assert_eq!(flat_code, EZ_SR_OK);
    let flat_sr = out.sr;

    assert!(
        (flat_sr - base_sr).abs() > 1e-6,
        "changing the cross matrix must change the rating (got {flat_sr} vs {base_sr})"
    );
}

#[test]
fn negative_cross_coefficient_is_rejected() {
    let notes = stream(4, 32);
    let mut bad = CrossMatrixProvider::get_matrix(4).unwrap().to_vec();
    bad[2] = -0.5;

    let mut out = EzSrResult { sr: 0.0, components: [0.0; COMPONENT_COUNT], component_count: 0 };

    let code = unsafe {
        ez_sr_calculate(
            notes.as_ptr(),
            notes.len(),
            4,
            8.0,
            bad.as_ptr(),
            EzSrConfig { rescale_threshold: 8.0, ln_multiplier: 5.5 },
            &mut out,
        )
    };

    assert_eq!(code, EZ_SR_ERR_CROSS_MATRIX);
}

#[test]
fn components_are_bounded_by_the_rating() {
    for (keys, notes) in [(4, hold_chart(4, 200)), (7, stream(7, 300))] {
        let (code, out) = call(&notes, keys, 8.0);
        assert_eq!(code, EZ_SR_OK);

        let mut max_component = 0.0_f64;

        for value in out.components {
            assert!(value.is_finite(), "{keys}k component is not finite: {value}");
            assert!(value >= 0.0, "{keys}k component is negative: {value}");
            // Components are published rounded to two decimals after being clamped to the rating, so a component
            // may sit up to half a rounding step above it. The host's reference does the same.
            assert!(
                value <= out.sr + 0.005,
                "{keys}k component {value} exceeds the rating {} beyond a rounding step",
                out.sr
            );
            max_component = max_component.max(value);
        }

        assert!(
            (max_component - out.sr).abs() < 0.005,
            "{keys}k largest component {max_component} should sit at the rating {} after normalisation",
            out.sr
        );
    }
}

#[test]
fn tunables_change_the_rating() {
    let notes = hold_chart(4, 200);
    let cross = CrossMatrixProvider::get_matrix(4).unwrap();

    let mut ez = EzSrResult { sr: 0.0, components: [0.0; COMPONENT_COUNT], component_count: 0 };
    let mut legacy = EzSrResult { sr: 0.0, components: [0.0; COMPONENT_COUNT], component_count: 0 };

    unsafe {
        ez_sr_calculate(
            notes.as_ptr(), notes.len(), 4, 8.0, cross.as_ptr(),
            EzSrConfig { rescale_threshold: 8.0, ln_multiplier: 5.5 }, &mut ez,
        );
        ez_sr_calculate(
            notes.as_ptr(), notes.len(), 4, 8.0, cross.as_ptr(),
            EzSrConfig { rescale_threshold: 9.0, ln_multiplier: 6.0 }, &mut legacy,
        );
    }

    assert!(
        (ez.sr - legacy.sr).abs() > 1e-9,
        "the tunables must reach the core (got {} vs {})",
        ez.sr,
        legacy.sr
    );
}

/// The SR Adjust mod exposes both coefficients. Each must move the rating on its own, in the direction the mod's
/// usage implies: a higher threshold compresses less of the rating, a higher LN multiplier rates holds harder.
#[test]
fn each_coefficient_is_independent_and_monotonic() {
    // Must rate above the highest threshold the mod exposes, otherwise the rescale never engages and the
    // threshold assertion would be vacuous.
    let notes = dense_chart(4, 900);
    let cross = CrossMatrixProvider::get_matrix(4).unwrap();

    let rate_with = |threshold: f64, ln: f64| -> f64 {
        let mut out = EzSrResult { sr: 0.0, components: [0.0; COMPONENT_COUNT], component_count: 0 };
        let code = unsafe {
            ez_sr_calculate(
                notes.as_ptr(), notes.len(), 4, 8.0, cross.as_ptr(),
                EzSrConfig { rescale_threshold: threshold, ln_multiplier: ln }, &mut out,
            )
        };
        assert_eq!(code, EZ_SR_OK);
        out.sr
    };

    let threshold_low = rate_with(5.0, 5.5);
    let threshold_high = rate_with(10.0, 5.5);

    assert!(
        threshold_low > 10.0,
        "fixture rates {threshold_low}, below the highest threshold; the rescale would not be exercised"
    );
    assert!(
        threshold_high > threshold_low,
        "raising the threshold must raise the rating: {threshold_low} -> {threshold_high}"
    );

    // LN multiplier only: the threshold is held at a value below the raw rating, so both calls rescale the same way.
    let ln_low = rate_with(5.0, 2.0);
    let ln_high = rate_with(5.0, 8.0);
    assert!(
        ln_high > ln_low,
        "raising the LN multiplier must raise the rating: {ln_low} -> {ln_high}"
    );

    // The two are separate inputs: equal moves must not be interchangeable.
    assert!(
        (threshold_high - threshold_low - (ln_high - ln_low)).abs() > 1e-9,
        "the two coefficients should not be interchangeable"
    );
}

/// The range the mod can produce, so a coefficient is never silently clamped away.
#[test]
fn mod_range_is_honoured() {
    let notes = dense_chart(7, 600);
    let cross = CrossMatrixProvider::get_matrix(7).unwrap();

    for threshold in [5.0, 6.0, 7.0, 8.0, 9.0, 10.0] {
        for ln in [2.0, 4.5, 5.5, 6.0, 8.0] {
            let mut out = EzSrResult { sr: 0.0, components: [0.0; COMPONENT_COUNT], component_count: 0 };
            let code = unsafe {
                ez_sr_calculate(
                    notes.as_ptr(), notes.len(), 7, 8.0, cross.as_ptr(),
                    EzSrConfig { rescale_threshold: threshold, ln_multiplier: ln }, &mut out,
                )
            };

            assert_eq!(code, EZ_SR_OK, "threshold={threshold} ln={ln} returned {code}");
            assert!(out.sr.is_finite() && out.sr > 0.0, "threshold={threshold} ln={ln} produced {}", out.sr);
        }
    }
}

/// A non-finite or non-positive coefficient falls back to the host's default rather than poisoning the result.
#[test]
fn invalid_coefficients_fall_back() {
    let notes = stream(4, 64);
    let cross = CrossMatrixProvider::get_matrix(4).unwrap();

    let rate_with = |config: EzSrConfig| -> f64 {
        let mut out = EzSrResult { sr: 0.0, components: [0.0; COMPONENT_COUNT], component_count: 0 };
        let code = unsafe {
            ez_sr_calculate(notes.as_ptr(), notes.len(), 4, 8.0, cross.as_ptr(), config, &mut out)
        };
        assert_eq!(code, EZ_SR_OK);
        out.sr
    };

    let baseline = rate_with(EzSrConfig { rescale_threshold: 8.0, ln_multiplier: 5.5 });
    let nan_ln = rate_with(EzSrConfig { rescale_threshold: 8.0, ln_multiplier: f64::NAN });
    let zero_threshold = rate_with(EzSrConfig { rescale_threshold: 0.0, ln_multiplier: 5.5 });

    assert_eq!(baseline.to_bits(), nan_ln.to_bits());
    assert_eq!(baseline.to_bits(), zero_threshold.to_bits());
}

#[test]
fn repeated_calls_are_stable() {
    let notes = hold_chart(7, 300);

    let first = call(&notes, 7, 8.0);
    let second = call(&notes, 7, 8.0);

    assert_eq!(first.0, EZ_SR_OK);
    assert_eq!(first.1.sr.to_bits(), second.1.sr.to_bits(), "the core must be deterministic");

    for i in 0..COMPONENT_COUNT {
        assert_eq!(
            first.1.components[i].to_bits(),
            second.1.components[i].to_bits(),
            "component {i} drifted between identical calls"
        );
    }
}

#[test]
fn empty_chart_rates_zero() {
    let (code, out) = call(&[], 4, 8.0);

    assert_eq!(code, EZ_SR_OK);
    assert_eq!(out.sr, 0.0);
    assert_eq!(out.component_count as usize, COMPONENT_COUNT);
}

#[test]
fn direct_entry_matches_the_abi() {
    let notes = stream(4, 96);
    let raw: Vec<(i32, i32, i32)> = notes.iter().map(|n| (n.col, n.head, n.tail)).collect();

    let direct = crate::ffi::calculate_notes(&raw, 4, 8.0, SrConfig::EZ_DEFAULT).expect("direct");
    let (code, out) = call(&notes, 4, 8.0);

    assert_eq!(code, EZ_SR_OK);
    assert_eq!(direct.sr.to_bits(), out.sr.to_bits());
}

#[test]
fn cross_matrix_is_readable_from_the_host() {
    for keys in 1..=18 {
        let len = ez_sr_cross_matrix_len(keys);
        assert_eq!(len, (keys + 1) as usize);
        assert!(!ez_sr_cross_matrix(keys).is_null());
    }

    assert_eq!(ez_sr_cross_matrix_len(0), 0);
    assert!(ez_sr_cross_matrix(19).is_null());
}

#[test]
fn parsed_file_entry_still_works() {
    // The legacy entry keeps its own defaults; this only asserts it does not regress structurally.
    let data = crate::parser::ParsedData {
        column_count: 4,
        columns: vec![0, 1, 2, 3],
        note_starts: vec![500, 620, 740, 860],
        note_ends: vec![0, 0, 0, 0],
        note_types: vec![1, 1, 1, 1],
        od: 8.0,
    };

    let sr = SRCalculator::calculate_sr_from_parsed_data(&data).expect("legacy entry");
    assert!(sr >= 0.0);
}

