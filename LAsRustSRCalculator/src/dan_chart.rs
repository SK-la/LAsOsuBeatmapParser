//! Pure chart-side Dan derivation and per-chart skillset filing.
//!
//! Score history, player clear windows, persistence, and presentation resources are deliberately
//! outside this module. Chart features are inputs because CSI and LeoBlack are separate passes.

use crate::dan_filing::{DanBucketVector, DanChartFeatures, classify_skillset_buckets};
use crate::dan_lookup::{DanSide, DanValueSource, try_lookup};
use crate::skills::{EzMinaSkillAxis, SkillAxis, SkillVector};

const REFORM_MEANS_LEVELS: usize = 20;
const LN_PRIMARY_MIN_RATIO: f64 = 0.45;
const LN_PRIMARY_7K_MIN_RATIO: f64 = 0.375;

const STREAM_MEANS: [f64; 20] = [
    3.1, 3.5, 3.9, 4.3, 4.7, 5.05, 5.35, 5.6, 5.78, 5.92, 6.12, 6.5, 6.92, 7.42, 8.08, 8.8, 9.65,
    10.42, 11.2, 12.0,
];
const JACK_MEANS: [f64; 20] = [
    3.15, 3.55, 3.95, 4.35, 4.75, 5.15, 5.45, 5.7, 5.92, 6.1, 6.35, 6.75, 7.15, 7.65, 8.25, 8.85,
    9.55, 10.25, 11.0, 11.8,
];
const JUMPSTREAM_MEANS: [f64; 20] = [
    3.15, 3.55, 3.95, 4.35, 4.75, 5.1, 5.4, 5.66, 5.86, 6.02, 6.35, 6.72, 7.08, 7.55, 8.15, 8.85,
    9.65, 10.38, 11.14, 11.95,
];
const HANDSTREAM_MEANS: [f64; 20] = [
    3.2, 3.6, 4.0, 4.4, 4.8, 5.15, 5.45, 5.72, 5.92, 6.08, 6.6, 6.9, 6.96, 7.72, 8.48, 9.18, 9.98,
    10.72, 11.46, 12.2,
];
const STAMINA_MEANS: [f64; 20] = [
    3.2, 3.6, 4.0, 4.4, 4.8, 5.15, 5.45, 5.72, 5.92, 6.08, 6.3, 6.7, 7.12, 7.62, 8.28, 8.98, 9.78,
    10.52, 11.26, 12.0,
];
const CHORDJACK_MEANS: [f64; 20] = [
    3.2, 3.6, 4.0, 4.42, 4.82, 5.18, 5.48, 5.75, 5.95, 6.12, 6.35, 6.75, 7.15, 7.65, 8.25, 8.85,
    9.55, 10.25, 11.0, 11.8,
];
const TECH_MEANS: [f64; 20] = [
    3.25, 3.65, 4.05, 4.48, 4.88, 5.25, 5.55, 5.82, 6.02, 6.18, 6.42, 6.82, 7.22, 7.72, 8.35, 9.02,
    9.8, 10.52, 11.26, 12.0,
];

/// User-configurable gate used only for the extra persisted LN chart half.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DanChartConfig {
    /// A chart can receive an LN side when its hold object count is strictly above this value,
    /// or when its hold ratio reaches the keymode's primary LN threshold.
    pub ln_chart_min_hold_objects: usize,
}

/// One ladder estimate and the chart-side buckets stamped with its label.
#[derive(Clone, Debug, PartialEq)]
pub struct DanChartSideResult {
    pub side: DanSide,
    pub raw_dan: f64,
    pub display_label: String,
    pub value_source: DanValueSource,
    pub bucket_ids: DanBucketVector,
}

/// Dan derivation for a single chart. RC and LN are independent optional halves.
#[derive(Clone, Debug, PartialEq)]
pub struct DanChartResult {
    pub key_count: usize,
    pub hold_ratio: f64,
    pub hold_count: Option<usize>,
    pub overall_msd: f64,
    pub dominant_axis: EzMinaSkillAxis,
    /// Ordered as RC, LN to provide a compact, stable interop layout.
    pub sides: [Option<DanChartSideResult>; 2],
}

impl DanChartResult {
    pub fn for_side(&self, side: DanSide) -> Option<&DanChartSideResult> {
        self.sides[side_index(side)].as_ref()
    }
}

/// Inputs for pure chart-side Dan derivation. Mina axes and chart metadata are supplied directly.
#[derive(Clone, Copy, Debug)]
pub struct DanChartInput<'a> {
    pub key_count: usize,
    pub hold_ratio: f64,
    pub hold_count: Option<usize>,
    pub sunny_sr: Option<f64>,
    pub mina_axes: &'a SkillVector<EzMinaSkillAxis>,
    pub chart_features: Option<&'a DanChartFeatures<'a>>,
}

/// Derives both available chart Dan sides and their pure chart-level skillset labels.
///
/// An unsupported tableless keymode can still produce a valid result with no rated side when it
/// has no Sunny SR; its Mina overall and dominant axis remain available to the caller.
pub fn derive_chart_dan(
    input: DanChartInput<'_>,
    config: DanChartConfig,
) -> Option<DanChartResult> {
    if input.key_count == 0 {
        return None;
    }
    let overall = input.mina_axes.get(EzMinaSkillAxis::Overall);
    if !overall.is_finite() || overall <= 0.0 {
        return None;
    }

    let dominant = dominant_mina_axis(input.mina_axes);
    let mut sides = [None, None];
    for side in [DanSide::Rc, DanSide::Ln] {
        if side == DanSide::Ln
            && !allows_persisted_ln_half(
                input.key_count,
                input.hold_ratio,
                input.hold_count,
                config.ln_chart_min_hold_objects,
            )
        {
            continue;
        }

        let Some((raw_dan, display_label, value_source)) =
            resolve_side(input.key_count, side, overall, dominant, input.sunny_sr)
        else {
            continue;
        };
        let bucket_ids = classify_skillset_buckets(
            input.key_count,
            side,
            input.mina_axes,
            input.chart_features,
            input
                .chart_features
                .and_then(|features| features.length_seconds),
            1.0,
        );
        sides[side_index(side)] = Some(DanChartSideResult {
            side,
            raw_dan,
            display_label,
            value_source,
            bucket_ids,
        });
    }

    Some(DanChartResult {
        key_count: input.key_count,
        hold_ratio: input.hold_ratio,
        hold_count: input.hold_count,
        overall_msd: overall,
        dominant_axis: dominant,
        sides,
    })
}

/// The chart's primary identity threshold used by clear filing and side selection.
pub const fn ln_primary_min_ratio_for(key_count: usize) -> f64 {
    if key_count == 7 {
        LN_PRIMARY_7K_MIN_RATIO
    } else {
        LN_PRIMARY_MIN_RATIO
    }
}

pub const fn primary_side(key_count: usize, hold_ratio: f64) -> DanSide {
    if hold_ratio >= ln_primary_min_ratio_for(key_count) {
        DanSide::Ln
    } else {
        DanSide::Rc
    }
}

/// Whether the persisted LN half is available under the explicit caller-provided chart gate.
pub fn allows_persisted_ln_half(
    key_count: usize,
    hold_ratio: f64,
    hold_count: Option<usize>,
    ln_chart_min_hold_objects: usize,
) -> bool {
    hold_count.is_some_and(|count| count > ln_chart_min_hold_objects)
        || hold_ratio >= ln_primary_min_ratio_for(key_count)
}

/// Highest Mina radar axis, preserving the C# radar ordering for ties.
pub fn dominant_mina_axis(axes: &SkillVector<EzMinaSkillAxis>) -> EzMinaSkillAxis {
    let mut best = EzMinaSkillAxis::Stream;
    let mut best_value = -1.0;
    for axis in EzMinaSkillAxis::radar_axes() {
        let value = axes.get(axis);
        if !value.is_finite() || value <= best_value {
            continue;
        }
        best = axis;
        best_value = value;
    }
    best
}

/// Maps an overall Mina value through the axis-specific calibrated means table.
pub fn sr_to_raw_dan(sr: f64, axis: EzMinaSkillAxis) -> f64 {
    if !sr.is_finite() || sr <= 0.0 {
        return 1.0;
    }
    let calibration_axis = if axis == EzMinaSkillAxis::Jumpstream {
        EzMinaSkillAxis::Handstream
    } else {
        axis
    };
    raw_dan_from_means(
        calibrate_sr_for_axis(sr, calibration_axis),
        means_for(calibration_axis),
    )
}

/// Parses the raw Dan value using the same zero/one-based ladder and tier boundaries as EzLazer.
pub fn ladder_label(key_count: usize, side: DanSide, raw_dan: f64) -> Option<String> {
    let (labels, one_based): (Vec<String>, bool) = match (key_count, side) {
        (4, DanSide::Ln) => ((1..=17).map(|value| value.to_string()).collect(), true),
        (4, DanSide::Rc) => (
            (1..=10)
                .map(|value| value.to_string())
                .chain(
                    [
                        "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta",
                        "iota", "kappa",
                    ]
                    .into_iter()
                    .map(str::to_owned),
                )
                .collect(),
            true,
        ),
        (6, _) => (
            (0..=9)
                .map(|value| value.to_string())
                .chain(
                    ["terra", "celestial", "mystery", "nihility", "finish"]
                        .into_iter()
                        .map(str::to_owned),
                )
                .collect(),
            false,
        ),
        (7, _) => (
            (0..=10)
                .map(|value| value.to_string())
                .chain(
                    ["gamma", "azimuth", "zenith", "stellium"]
                        .into_iter()
                        .map(str::to_owned),
                )
                .collect(),
            false,
        ),
        _ => return None,
    };
    if !raw_dan.is_finite() {
        return None;
    }
    let level = if one_based {
        raw_dan.round_ties_even().clamp(1.0, labels.len() as f64) as i64
    } else {
        raw_dan
            .round_ties_even()
            .clamp(0.0, labels.len() as f64 - 1.0) as i64
    };
    let index = if one_based { level - 1 } else { level } as usize;
    let suffix = match raw_dan - level as f64 {
        offset if offset <= -0.45 => "--",
        offset if offset <= -0.25 => "-",
        offset if offset < 0.1 => "",
        offset if offset < 0.26 => "+",
        _ => "++",
    };
    Some(format!("{}{suffix}", labels[index]))
}

fn resolve_side(
    key_count: usize,
    side: DanSide,
    overall_msd: f64,
    dominant_axis: EzMinaSkillAxis,
    sunny_sr: Option<f64>,
) -> Option<(f64, String, DanValueSource)> {
    if let Some(sr) = sunny_sr.filter(|value| value.is_finite() && *value > 0.0) {
        if let Some(result) = try_lookup(key_count, side, sr) {
            return Some((
                result.raw_dan,
                result.display_label,
                DanValueSource::SunnyTable,
            ));
        }
        if !matches!(key_count, 4 | 6 | 7) {
            if let Some(result) = try_lookup(4, side, sr) {
                return Some((
                    result.raw_dan,
                    result.display_label,
                    DanValueSource::BorrowedSunnyTable,
                ));
            }
        }
    }

    if !matches!(key_count, 4 | 6 | 7) {
        return None;
    }
    let raw_dan = sr_to_raw_dan(overall_msd, dominant_axis);
    let display_label = ladder_label(key_count, side, raw_dan)?;
    Some((raw_dan, display_label, DanValueSource::MinaMeans))
}

fn side_index(side: DanSide) -> usize {
    match side {
        DanSide::Rc => 0,
        DanSide::Ln => 1,
    }
}

fn means_for(axis: EzMinaSkillAxis) -> &'static [f64; REFORM_MEANS_LEVELS] {
    match axis {
        EzMinaSkillAxis::JackSpeed => &JACK_MEANS,
        EzMinaSkillAxis::Jumpstream => &JUMPSTREAM_MEANS,
        EzMinaSkillAxis::Handstream => &HANDSTREAM_MEANS,
        EzMinaSkillAxis::Stamina => &STAMINA_MEANS,
        EzMinaSkillAxis::Chordjack => &CHORDJACK_MEANS,
        EzMinaSkillAxis::Technical => &TECH_MEANS,
        _ => &STREAM_MEANS,
    }
}

fn calibrate_sr_for_axis(sr: f64, axis: EzMinaSkillAxis) -> f64 {
    let (slope, offset, gate_start, gate_width) = match axis {
        EzMinaSkillAxis::JackSpeed => (0.74, 1.3, 7.4, 0.3),
        EzMinaSkillAxis::Stream => (0.92, -0.4, 7.0, 0.9),
        EzMinaSkillAxis::Handstream => (1.16, -2.0, 7.4, 0.2),
        EzMinaSkillAxis::Stamina => (1.12, -1.5, 7.3, 0.55),
        EzMinaSkillAxis::Chordjack => (1.0, 0.0, 7.15, 0.85),
        EzMinaSkillAxis::Technical => (0.95, -0.9, 7.6, 0.9),
        EzMinaSkillAxis::Jumpstream | EzMinaSkillAxis::Overall => (0.92, -0.4, 7.0, 0.9),
    };
    let target = sr * slope + offset;
    let gate = ((sr - gate_start) / gate_width).clamp(0.0, 1.0);
    sr + (target - sr) * gate
}

fn raw_dan_from_means(value: f64, means: &[f64; REFORM_MEANS_LEVELS]) -> f64 {
    if value < boundary_lower(means, 0) {
        return 1.0;
    }
    let last = means.len() - 1;
    if value >= boundary_upper(means, last) {
        return 20.0;
    }
    for index in 0..means.len() {
        let lower = boundary_lower(means, index);
        let upper = boundary_upper(means, index);
        if value >= lower && value < upper {
            let t = (value - lower) / (upper - lower).max(0.001);
            return index as f64 + 1.0 + t - 0.5;
        }
    }
    1.0
}

fn boundary_lower(means: &[f64; REFORM_MEANS_LEVELS], index: usize) -> f64 {
    if index == 0 {
        means[0] - (means[1] - means[0]) / 2.0
    } else {
        (means[index - 1] + means[index]) / 2.0
    }
}

fn boundary_upper(means: &[f64; REFORM_MEANS_LEVELS], index: usize) -> f64 {
    if index == means.len() - 1 {
        means[index] + (means[index] - means[index - 1]) / 2.0
    } else {
        (means[index] + means[index + 1]) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axes(overall: f64) -> SkillVector<EzMinaSkillAxis> {
        SkillVector::from_fn(|axis| match axis {
            EzMinaSkillAxis::Overall => overall,
            EzMinaSkillAxis::Stream => 18.0,
            EzMinaSkillAxis::Technical => 20.0,
            _ => 5.0,
        })
    }

    #[test]
    fn csharp_sunny_table_fixture_six_key_rc_703_is_seven_minus() {
        let axes = axes(20.5);
        let chart = derive_chart_dan(
            DanChartInput {
                key_count: 6,
                hold_ratio: 0.1,
                hold_count: Some(0),
                sunny_sr: Some(7.03),
                mina_axes: &axes,
                chart_features: None,
            },
            DanChartConfig {
                ln_chart_min_hold_objects: 0,
            },
        )
        .unwrap();
        let rc = chart.for_side(DanSide::Rc).unwrap();
        assert_eq!(rc.display_label, "7-");
        assert!((rc.raw_dan - 6.8).abs() <= 1e-12);
        assert_eq!(rc.value_source, DanValueSource::SunnyTable);
        assert!(chart.for_side(DanSide::Ln).is_none());
    }

    #[test]
    fn tableless_keymode_borrows_same_side_four_key_sunny_table() {
        let axes = axes(20.0);
        let chart = derive_chart_dan(
            DanChartInput {
                key_count: 8,
                hold_ratio: 0.1,
                hold_count: Some(0),
                sunny_sr: Some(6.0),
                mina_axes: &axes,
                chart_features: None,
            },
            DanChartConfig {
                ln_chart_min_hold_objects: 0,
            },
        )
        .unwrap();
        let expected = try_lookup(4, DanSide::Rc, 6.0).unwrap();
        let rc = chart.for_side(DanSide::Rc).unwrap();
        assert_eq!(rc.raw_dan, expected.raw_dan);
        assert_eq!(rc.display_label, expected.display_label);
        assert_eq!(rc.value_source, DanValueSource::BorrowedSunnyTable);
    }

    #[test]
    fn tableless_keymode_without_sunny_rating_keeps_axes_without_inventing_dan() {
        let axes = axes(20.0);
        let chart = derive_chart_dan(
            DanChartInput {
                key_count: 8,
                hold_ratio: 0.1,
                hold_count: None,
                sunny_sr: None,
                mina_axes: &axes,
                chart_features: None,
            },
            DanChartConfig {
                ln_chart_min_hold_objects: 20,
            },
        )
        .unwrap();
        assert_eq!(chart.overall_msd, 20.0);
        assert!(chart.for_side(DanSide::Rc).is_none());
        assert!(chart.for_side(DanSide::Ln).is_none());
    }

    #[test]
    fn ln_half_gate_is_strict_for_count_and_ratio_based_for_seven_key() {
        assert!(!allows_persisted_ln_half(4, 0.1, Some(24), 24));
        assert!(allows_persisted_ln_half(4, 0.1, Some(25), 24));
        assert!(allows_persisted_ln_half(7, 0.375, Some(0), 24));
        assert_eq!(primary_side(7, 0.374), DanSide::Rc);
        assert_eq!(primary_side(7, 0.375), DanSide::Ln);
    }

    #[test]
    fn fallback_uses_overall_mina_and_dominant_axis_means() {
        let mut values = SkillVector::zeros();
        values.set(EzMinaSkillAxis::Overall, 5.92);
        values.set(EzMinaSkillAxis::Stream, 18.0);
        let estimate = derive_chart_dan(
            DanChartInput {
                key_count: 4,
                hold_ratio: 0.1,
                hold_count: None,
                sunny_sr: None,
                mina_axes: &values,
                chart_features: None,
            },
            DanChartConfig {
                ln_chart_min_hold_objects: 24,
            },
        )
        .unwrap();
        let rc = estimate.for_side(DanSide::Rc).unwrap();
        assert_eq!(rc.value_source, DanValueSource::MinaMeans);
        let expected = 10.0 + (5.92 - 5.85) / (6.02 - 5.85) - 0.5;
        assert!((rc.raw_dan - expected).abs() < 1e-12);
        assert_eq!(estimate.dominant_axis, EzMinaSkillAxis::Stream);
    }
}
