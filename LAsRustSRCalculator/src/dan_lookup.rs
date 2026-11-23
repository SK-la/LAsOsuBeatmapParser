//! Sunny SR interval-table Dan lookup used by Ez2Lazer's chart Dan calculation.

use crate::dan_tables::{DanInterval, LN4K, LN6K, LN7K, RC4K, RC6K, RC7K};
use crate::skills::{EzMinaSkillAxis, SkillAxis, SkillVector};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DanSide {
    Rc,
    Ln,
}

impl DanSide {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Rc => "rc",
            Self::Ln => "ln",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DanLookup {
    pub key_count: usize,
    pub side: DanSide,
    pub interval_name: String,
    pub display_label: String,
    pub raw_dan: f64,
    pub value_source: DanValueSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DanValueSource {
    SunnyTable,
    BorrowedSunnyTable,
    MinaMeans,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DanChartEstimate {
    pub key_count: usize,
    pub side: DanSide,
    pub raw_dan: f64,
    pub display_label: String,
    pub overall_msd: f64,
    pub dominant_axis: EzMinaSkillAxis,
    pub value_source: DanValueSource,
}

const TIERS: [(&str, &str, f64); 5] = [
    ("mid/low", "-", -0.2),
    ("mid/high", "+", 0.2),
    ("low", "--", -0.4),
    ("mid", "", 0.0),
    ("high", "++", 0.4),
];
const GREEK_TAILS: [&str; 18] = [
    "alpha",
    "beta",
    "gamma",
    "delta",
    "epsilon",
    "zeta",
    "eta",
    "theta",
    "iota",
    "kappa",
    "azimuth",
    "zenith",
    "stellium",
    "terra",
    "celestial",
    "mystery",
    "nihility",
    "finish",
];

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

#[derive(Clone, Copy)]
struct Calibration {
    slope: f64,
    offset: f64,
    gate_start: f64,
    gate_width: f64,
}

/// Looks up a Sunny SR value. Supports the default table sets shipped by Ez2Lazer: 4K/6K/7K.
pub fn try_lookup(key_count: usize, side: DanSide, sunny_sr: f64) -> Option<DanLookup> {
    if !matches!(key_count, 4 | 6 | 7) || !sunny_sr.is_finite() || sunny_sr <= 0.0 {
        return None;
    }
    let table = table_for(key_count, side);
    if table.is_empty() {
        return None;
    }
    let interval_name = if let Some(row) = table
        .iter()
        .find(|row| row.lower <= sunny_sr && sunny_sr <= row.upper)
    {
        row.name.to_owned()
    } else if sunny_sr < table[0].lower {
        format!("< {}", table[0].name)
    } else if sunny_sr > table[table.len() - 1].upper {
        format!("> {}", table[table.len() - 1].name)
    } else {
        // This matches the fallback for small gaps between adjacent published intervals.
        table[0].name.to_owned()
    };

    let (base, tier, marker) = split_interval(&interval_name)?;
    let level = resolve_level(table, base)?;
    let bare = bare_label(base);
    let (suffix, offset) = match marker {
        Some(Boundary::Below) => ("--", -0.5),
        Some(Boundary::Above) => ("++", 0.5),
        None => {
            let (_, suffix, offset) = TIERS.iter().find(|(candidate, _, _)| *candidate == tier)?;
            (*suffix, *offset)
        }
    };
    Some(DanLookup {
        key_count,
        side,
        interval_name,
        display_label: format!("{bare}{suffix}"),
        raw_dan: level as f64 + offset,
        value_source: DanValueSource::SunnyTable,
    })
}

/// Derives one chart-side Dan estimate from Mina axes and optional Sunny SR.
/// Sunny tables are authoritative; supported table modes fall back to Mina means, while tableless
/// modes borrow the 4K Sunny table only when a Sunny SR value is available.
pub fn from_mina_chart(
    key_count: usize,
    hold_ratio: f64,
    sunny_sr: Option<f64>,
    axes: &SkillVector<EzMinaSkillAxis>,
) -> Option<DanChartEstimate> {
    let overall = axes.get(EzMinaSkillAxis::Overall);
    if key_count == 0 || !overall.is_finite() || overall <= 0.0 {
        return None;
    }
    let mut dominant = EzMinaSkillAxis::Stream;
    let mut dominant_value = 0.0;
    for axis in EzMinaSkillAxis::radar_axes() {
        let value = axes.get(axis);
        if value.is_finite() && value > dominant_value {
            dominant = axis;
            dominant_value = value;
        }
    }
    let side = if hold_ratio >= if key_count == 7 { 0.375 } else { 0.45 } {
        DanSide::Ln
    } else {
        DanSide::Rc
    };
    if let Some(sr) = sunny_sr.filter(|sr| sr.is_finite() && *sr > 0.0) {
        if let Some(value) = try_lookup(key_count, side, sr) {
            return Some(DanChartEstimate {
                key_count,
                side,
                raw_dan: value.raw_dan,
                display_label: value.display_label,
                overall_msd: overall,
                dominant_axis: dominant,
                value_source: DanValueSource::SunnyTable,
            });
        }
        if !matches!(key_count, 4 | 6 | 7) {
            if let Some(mut borrowed) = try_lookup(4, side, sr) {
                borrowed.key_count = key_count;
                borrowed.value_source = DanValueSource::BorrowedSunnyTable;
                return Some(DanChartEstimate {
                    key_count,
                    side,
                    raw_dan: borrowed.raw_dan,
                    display_label: borrowed.display_label,
                    overall_msd: overall,
                    dominant_axis: dominant,
                    value_source: borrowed.value_source,
                });
            }
        }
    }
    if !matches!(key_count, 4 | 6 | 7) {
        return None;
    }
    let axis = if dominant == EzMinaSkillAxis::Jumpstream {
        EzMinaSkillAxis::Handstream
    } else {
        dominant
    };
    let calibrated = calibrate_axis(axes.get(axis), axis);
    let raw_dan = sr_to_raw_dan(calibrated, axis);
    Some(DanChartEstimate {
        key_count,
        side,
        raw_dan,
        display_label: ladder_label(key_count, side, raw_dan),
        overall_msd: overall,
        dominant_axis: dominant,
        value_source: DanValueSource::MinaMeans,
    })
}

fn sr_to_raw_dan(value: f64, axis: EzMinaSkillAxis) -> f64 {
    let means = means_for(axis);
    if value < means[0] - (means[1] - means[0]) / 2.0 {
        return 1.0;
    }
    let last = means.len() - 1;
    if value >= means[last] + (means[last] - means[last - 1]) / 2.0 {
        return 20.0;
    }
    for index in 0..means.len() {
        let lower = if index == 0 {
            means[0] - (means[1] - means[0]) / 2.0
        } else {
            (means[index - 1] + means[index]) / 2.0
        };
        let upper = if index == last {
            means[last] + (means[last] - means[last - 1]) / 2.0
        } else {
            (means[index] + means[index + 1]) / 2.0
        };
        if value >= lower && value < upper {
            let t = (value - lower) / (upper - lower).max(0.001);
            return index as f64 + 1.0 + t - 0.5;
        }
    }
    1.0
}

fn means_for(axis: EzMinaSkillAxis) -> &'static [f64; 20] {
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

fn calibration(axis: EzMinaSkillAxis) -> Calibration {
    match axis {
        EzMinaSkillAxis::JackSpeed => Calibration {
            slope: 0.74,
            offset: 1.3,
            gate_start: 7.4,
            gate_width: 0.3,
        },
        EzMinaSkillAxis::Stream => Calibration {
            slope: 0.92,
            offset: -0.4,
            gate_start: 7.0,
            gate_width: 0.9,
        },
        EzMinaSkillAxis::Jumpstream => Calibration {
            slope: 1.02,
            offset: -0.45,
            gate_start: 7.25,
            gate_width: 0.55,
        },
        EzMinaSkillAxis::Handstream => Calibration {
            slope: 1.16,
            offset: -2.0,
            gate_start: 7.4,
            gate_width: 0.2,
        },
        EzMinaSkillAxis::Stamina => Calibration {
            slope: 1.12,
            offset: -1.5,
            gate_start: 7.3,
            gate_width: 0.55,
        },
        EzMinaSkillAxis::Chordjack => Calibration {
            slope: 1.0,
            offset: 0.0,
            gate_start: 7.15,
            gate_width: 0.85,
        },
        EzMinaSkillAxis::Technical => Calibration {
            slope: 0.95,
            offset: -0.9,
            gate_start: 7.6,
            gate_width: 0.9,
        },
        EzMinaSkillAxis::Overall => Calibration {
            slope: 0.92,
            offset: -0.4,
            gate_start: 7.0,
            gate_width: 0.9,
        },
    }
}

fn calibrate_axis(sr: f64, axis: EzMinaSkillAxis) -> f64 {
    if !sr.is_finite() || sr <= 0.0 {
        return 0.0;
    }
    let value = calibration(axis);
    let target = sr * value.slope + value.offset;
    let gate = ((sr - value.gate_start) / value.gate_width).clamp(0.0, 1.0);
    sr + (target - sr) * gate
}

fn ladder_label(key_count: usize, side: DanSide, raw_dan: f64) -> String {
    let labels: Vec<String> = match (key_count, side) {
        (4, DanSide::Ln) => (1..=17).map(|value| value.to_string()).collect(),
        (4, DanSide::Rc) => (1..=10)
            .map(|value| value.to_string())
            .chain(GREEK_TAILS[..10].iter().map(|x| (*x).to_owned()))
            .collect(),
        (6, _) => (0..=9)
            .map(|value| value.to_string())
            .chain(
                ["terra", "celestial", "mystery", "nihility", "finish"]
                    .iter()
                    .map(|x| (*x).to_owned()),
            )
            .collect(),
        (7, _) => (0..=10)
            .map(|value| value.to_string())
            .chain(
                ["gamma", "azimuth", "zenith", "stellium"]
                    .iter()
                    .map(|x| (*x).to_owned()),
            )
            .collect(),
        _ => return String::new(),
    };
    let one_based = key_count == 4;
    let level = if one_based {
        raw_dan.round_ties_even().clamp(1.0, labels.len() as f64) as i64
    } else {
        raw_dan
            .round_ties_even()
            .clamp(0.0, labels.len() as f64 - 1.0) as i64
    };
    let index =
        if one_based { level - 1 } else { level }.clamp(0, labels.len() as i64 - 1) as usize;
    let delta = raw_dan - level as f64;
    let suffix = if delta <= -0.45 {
        "--"
    } else if delta <= -0.25 {
        "-"
    } else if delta < 0.1 {
        ""
    } else if delta < 0.26 {
        "+"
    } else {
        "++"
    };
    format!("{}{suffix}", labels[index])
}

fn table_for(key_count: usize, side: DanSide) -> &'static [DanInterval] {
    match (key_count, side) {
        (4, DanSide::Rc) => RC4K,
        (4, DanSide::Ln) => LN4K,
        (6, DanSide::Rc) => RC6K,
        (6, DanSide::Ln) => LN6K,
        (7, DanSide::Rc) => RC7K,
        (7, DanSide::Ln) => LN7K,
        _ => &[],
    }
}

#[derive(Clone, Copy)]
enum Boundary {
    Below,
    Above,
}

fn split_interval(value: &str) -> Option<(&str, &str, Option<Boundary>)> {
    let (without_marker, marker) = if let Some(body) = value.strip_prefix("< ") {
        (body, Some(Boundary::Below))
    } else if let Some(body) = value.strip_prefix("> ") {
        (body, Some(Boundary::Above))
    } else {
        (value, None)
    };
    for (tier, _, _) in TIERS {
        let suffix = format!(" {tier}");
        if let Some(base) = without_marker.strip_suffix(&suffix) {
            if base.trim().is_empty() {
                return None;
            }
            return Some((base, tier, marker));
        }
    }
    None
}

fn resolve_level(table: &[DanInterval], target_base: &str) -> Option<i32> {
    let mut seen = Vec::<String>::new();
    let mut level = 0_i32;
    for row in table {
        let base = split_interval(row.name)
            .map(|(base, _, _)| base)
            .unwrap_or(row.name);
        if seen.iter().any(|candidate| candidate == base) {
            continue;
        }
        if let Some(number) = trailing_number(base) {
            level = number;
        } else {
            level += 1;
        }
        seen.push(base.to_owned());
        if base == target_base {
            return Some(level);
        }
    }
    None
}

fn trailing_number(value: &str) -> Option<i32> {
    let split = value.rfind(' ')?;
    value[split + 1..]
        .parse::<i32>()
        .ok()
        .filter(|number| *number >= 0)
}

fn bare_label(base: &str) -> String {
    let mut value = base.trim();
    if value.len() >= 8 && value[..8].eq_ignore_ascii_case("Regular ") {
        value = value[8..].trim_start();
    } else if value.len() >= 3 && value[..3].eq_ignore_ascii_case("LN ") {
        value = value[3..].trim_start();
    }
    if let Some(index) = value
        .find(" LN ")
        .filter(|index| *index == value.find(' ').unwrap_or(usize::MAX))
    {
        value = value[index + 4..].trim_start();
    }
    if let Some(rest) = value.strip_prefix("Reform ").map(str::trim_start) {
        if rest.parse::<i32>().is_ok() {
            return rest.to_owned();
        }
    }
    if let Some(rest) = value.strip_prefix("Intro ").map(str::trim_start) {
        if rest.parse::<i32>().is_ok() {
            return format!("intro{rest}");
        }
    }
    if value.parse::<i32>().is_ok() {
        return value.to_owned();
    }
    let lowercase = value.to_ascii_lowercase();
    for tail in GREEK_TAILS {
        if lowercase == tail || lowercase.ends_with(&format!(" {tail}")) {
            return tail.to_owned();
        }
    }
    lowercase
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_interval_tiers_ids_and_boundary_clamps() {
        let low = try_lookup(4, DanSide::Rc, 1.55).unwrap();
        assert_eq!(low.interval_name, "Intro 1 low");
        assert_eq!(low.display_label, "intro1--");
        assert!((low.raw_dan - 0.6).abs() < 1e-12);

        let middle = try_lookup(4, DanSide::Rc, 1.80).unwrap();
        assert_eq!(middle.display_label, "intro1");
        assert!((middle.raw_dan - 1.0).abs() < 1e-12);

        let below = try_lookup(4, DanSide::Rc, 1.0).unwrap();
        assert_eq!(below.display_label, "intro1--");
        assert!((below.raw_dan - 0.5).abs() < 1e-12);

        let above = try_lookup(4, DanSide::Rc, 20.0).unwrap();
        assert_eq!(above.value_source, DanValueSource::SunnyTable);
        assert!(above.display_label.ends_with("++"));
    }

    #[test]
    fn unsupported_keys_side_values_and_non_finite_sunny_star_are_unavailable() {
        assert!(try_lookup(5, DanSide::Rc, 4.0).is_none());
        assert!(try_lookup(7, DanSide::Ln, 0.0).is_none());
        assert!(try_lookup(7, DanSide::Ln, f64::NAN).is_none());
    }

    #[test]
    fn mina_fallback_matches_axis_calibration_and_tableless_requires_sunny() {
        let axes = SkillVector::<EzMinaSkillAxis>::from_fn(|axis| match axis {
            EzMinaSkillAxis::Overall => 6.0,
            EzMinaSkillAxis::Stream => 5.92,
            _ => 5.0,
        });
        let estimate = from_mina_chart(4, 0.1, None, &axes).unwrap();
        assert_eq!(estimate.dominant_axis, EzMinaSkillAxis::Stream);
        assert_eq!(estimate.value_source, DanValueSource::MinaMeans);
        let expected = 9.0 + 1.0 + (5.92 - 5.85) / (6.02 - 5.85) - 0.5;
        assert!((estimate.raw_dan - expected).abs() < 1e-12);
        assert!(from_mina_chart(8, 0.1, None, &axes).is_none());
        assert!(from_mina_chart(8, 0.1, Some(4.0), &axes).is_some());
    }

    #[test]
    fn fallback_ladders_keep_their_distinct_zero_and_one_based_levels() {
        assert_eq!(ladder_label(4, DanSide::Ln, 0.5), "1--");
        assert_eq!(ladder_label(4, DanSide::Rc, 0.5), "1--");
        assert_eq!(ladder_label(6, DanSide::Rc, 0.0), "0");
        assert_eq!(ladder_label(7, DanSide::Ln, 10.0), "10");
    }
}
