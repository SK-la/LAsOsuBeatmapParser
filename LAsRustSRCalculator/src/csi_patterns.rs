//! Pure pattern classification port of Ez2Lazer's CSI pattern analyzer.

use std::collections::{HashMap, HashSet};

use crate::csi_features::{
    EzChartFeatureExtraction, EzChartFeatureExtractor, EzChartFeatureMetrics, EzChartRow,
    EzManiaChartInput,
};
use crate::mania::ManiaNote;

pub const PATTERN_TAG_MIN_SCORE: f64 = 0.5;
pub const CHORDJACK_TAG_MIN_SCORE: f64 = 0.8;
pub const DELAY_TAG_MIN_SCORE: f64 = 0.25;
pub const CLUSTER_SHARE_MIN: f64 = 0.4;
pub const JACK_TECH_VETO_MIN_SCORE: f64 = 0.8;
pub const LN_PRIMARY_MIN_RATIO: f64 = 0.45;
pub const LN_PRIMARY_7K_MIN_RATIO: f64 = 0.375;
const INVERSE_WINDOW_MS: f64 = 8000.0;
const INVERSE_WINDOW_MIN_PAIRS: usize = 20;
const INVERSE_WINDOW_MIN_WINDOWS: usize = 3;
const INVERSE_WINDOW_RATIO: f64 = 0.65;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RcPatternAxis {
    Chordstream,
    Bracket,
    Delay,
    Stream,
    Jack,
    Tech,
    Ln,
    Chordjack,
    Speedjack,
    Handjack,
    Jumpstream,
    Handstream,
    Dumpstream,
    Quadstream,
}
impl RcPatternAxis {
    pub const ALL: [Self; 14] = [
        Self::Chordstream,
        Self::Bracket,
        Self::Delay,
        Self::Stream,
        Self::Jack,
        Self::Tech,
        Self::Ln,
        Self::Chordjack,
        Self::Speedjack,
        Self::Handjack,
        Self::Jumpstream,
        Self::Handstream,
        Self::Dumpstream,
        Self::Quadstream,
    ];
    pub const PLAYER_AXES: [Self; 6] = [
        Self::Chordstream,
        Self::Bracket,
        Self::Delay,
        Self::Stream,
        Self::Jack,
        Self::Tech,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Chordstream => "chordstream",
            Self::Bracket => "bracket",
            Self::Delay => "delay",
            Self::Stream => "stream",
            Self::Jack => "jack",
            Self::Tech => "tech",
            Self::Ln => "ln",
            Self::Chordjack => "chordjack",
            Self::Speedjack => "speedjack",
            Self::Handjack => "handjack",
            Self::Jumpstream => "jumpstream",
            Self::Handstream => "handstream",
            Self::Dumpstream => "dumpstream",
            Self::Quadstream => "quadstream",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Chordstream => "Chordstream",
            Self::Bracket => "Bracket",
            Self::Delay => "Delay",
            Self::Stream => "Stream",
            Self::Jack => "Jack",
            Self::Tech => "Tech",
            Self::Ln => "LN",
            Self::Chordjack => "Chordjack",
            Self::Speedjack => "Speedjack",
            Self::Handjack => "Handjack",
            Self::Jumpstream => "Jumpstream",
            Self::Handstream => "Handstream",
            Self::Dumpstream => "Dumpstream",
            Self::Quadstream => "Quadstream",
        }
    }
    pub fn try_parse(id: &str) -> Option<Self> {
        let bare = id.rsplit_once('.').map_or(id, |(_, s)| s);
        Self::ALL
            .into_iter()
            .find(|a| a.id().eq_ignore_ascii_case(bare))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LnPatternAxis {
    Density,
    Overlap,
    Release,
    Chord,
    HoldP90,
    General,
    Inverse,
    ReleaseSubtype,
    Tech,
}
impl LnPatternAxis {
    pub const ALL: [Self; 9] = [
        Self::Density,
        Self::Overlap,
        Self::Release,
        Self::Chord,
        Self::HoldP90,
        Self::General,
        Self::Inverse,
        Self::ReleaseSubtype,
        Self::Tech,
    ];
    pub const STRUCTURE: [Self; 5] = [
        Self::Density,
        Self::Overlap,
        Self::Release,
        Self::Chord,
        Self::HoldP90,
    ];
    pub const SUBTYPES: [Self; 4] = [
        Self::General,
        Self::Inverse,
        Self::ReleaseSubtype,
        Self::Tech,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Density => "ln_density",
            Self::Overlap => "ln_overlap",
            Self::Release => "ln_release",
            Self::Chord => "ln_chord",
            Self::HoldP90 => "ln_hold_p90",
            Self::General => "ln_general",
            Self::Inverse => "ln_inverse",
            Self::ReleaseSubtype => "ln_release_pattern",
            Self::Tech => "ln_tech",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Density => "Density",
            Self::Overlap => "Overlap",
            Self::Release => "Release",
            Self::Chord => "Chord",
            Self::HoldP90 => "Hold P90",
            Self::General => "General",
            Self::Inverse => "Inverse",
            Self::ReleaseSubtype => "Release",
            Self::Tech => "Tech",
        }
    }
    pub fn try_parse(id: &str) -> Option<Self> {
        let bare = id.rsplit_once('.').map_or(id, |(_, s)| s);
        Self::ALL
            .into_iter()
            .find(|a| a.id().eq_ignore_ascii_case(bare))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EzPatternHit {
    pub id: &'static str,
    pub label: &'static str,
    pub score: f64,
    pub confidence: f64,
    pub evidence: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EzPatternAnalysis {
    pub key_count: usize,
    pub primary: Option<EzPatternHit>,
    pub patterns: Vec<EzPatternHit>,
    pub all_patterns: Vec<EzPatternHit>,
    pub metrics: EzChartFeatureMetrics,
    pub warnings: Vec<&'static str>,
}

pub struct EzPatternAnalyzer;
impl EzPatternAnalyzer {
    pub fn analyze(map: EzManiaChartInput<'_>, rate: f64) -> EzPatternAnalysis {
        let features = EzChartFeatureExtractor::extract(map, rate).unwrap_or_default();
        Self::analyze_features(map, rate, &features)
    }

    pub fn analyze_features(
        map: EzManiaChartInput<'_>,
        rate: f64,
        features: &EzChartFeatureExtraction,
    ) -> EzPatternAnalysis {
        let m = &features.metrics;
        let stats = row_pattern_stats(&features.ordered_notes, &features.ordered_rows, m.key_count);
        let row_count = stats.row_count.max(1);
        let chord = m.chord_ratio;
        let two = ratio(stats.two, row_count);
        let three = ratio(stats.three, row_count);
        let three_plus = ratio(stats.three_plus, row_count);
        let four_plus = ratio(stats.four_plus, row_count);
        let repeated_chord = ratio(
            stats.repeated_chords,
            stats.chord_rows.saturating_sub(1).max(1),
        );
        let low_chord = clamp01((0.34 - chord) / 0.3);
        let wide = m.key_count >= 6;
        let stream_activity = pressure(m.stream_pressure, 1.5, 5.5).max(pressure(
            m.sustained_nps_10s,
            if wide { 7.0 } else { 12.0 },
            if wide { 18.0 } else { 27.0 },
        ));
        let chordstream = min_gate(&[
            pressure(
                chord,
                if wide { 0.14 } else { 0.24 },
                if wide { 0.5 } else { 0.58 },
            ),
            pressure(
                m.sustained_nps_10s,
                if wide { 6.0 } else { 11.0 },
                if wide { 17.0 } else { 25.0 },
            ),
        ]);
        let overlap = pressure(m.chord_column_overlap_ratio, 0.18, 0.4);
        let chordjack_base = min_gate(&[
            pressure(chord, 0.28, 0.64),
            pressure(m.chordjack_pressure, 70.0, 185.0),
            overlap,
        ])
        .max(min_gate(&[
            pressure(chord, 0.36, 0.72),
            pressure(m.jack_pressure, 80.0, 180.0),
            overlap,
        ]));
        let tech = min_gate(&[
            pressure(m.tech_pressure, 3.5, 8.5),
            pressure(m.row_pattern_change_rate, 0.34, 0.66),
        ])
        .max(min_gate(&[
            pressure(m.chord_size_change_rate, 0.25, 0.58),
            pressure(m.direction_change_rate, 0.35, 0.72),
            pressure(m.row_interval_entropy, 1.1, 2.4),
        ]));
        let confidence = clamp01(
            0.35 + (m.note_count as f64 / 2500.0).min(0.4)
                + (stats.row_count as f64 / 900.0).min(0.25),
        );
        let beat_length = if map.bpm.is_finite() && map.bpm > 0.0 {
            60000.0 / (map.bpm * rate)
        } else {
            0.0
        };
        let ln = ln_pattern_stats(features, beat_length);
        let ln_score = pressure(m.hold_ratio, 0.03, 0.32).max(
            min_gate(&[
                pressure(m.ln_density, 0.02, 0.18),
                pressure(m.ln_overlap_pressure, 0.4, 2.4),
            ])
            .max(min_gate(&[
                pressure(m.ln_release_pressure, 1.2, 5.5),
                pressure(m.hold_ratio, 0.015, 0.16),
            ]))
            .max(min_gate(&[
                pressure(m.ln_chord_pressure, 0.15, 0.65),
                pressure(m.hold_ratio, 0.02, 0.18),
            ])),
        );
        let subtype_keys = matches!(m.key_count, 4 | 7);
        let subtype_gate = if subtype_keys {
            pressure(ln_score, 0.18, 0.58)
        } else {
            0.0
        };
        let inverse_shape = (pressure(ln.inverse_release, 0.24, 0.62)
            * clamp01((0.16 - ln.mixed_rows) / 0.16))
        .max(if m.key_count == 7 {
            pressure(ln.inverse_window_coverage, 0.35, 0.75)
                * clamp01((0.45 - ln.mixed_rows) / 0.25)
        } else {
            0.0
        });
        let inverse = subtype_gate
            * min_gate(&[
                inverse_shape,
                pressure(m.ln_density, 0.12, 0.5),
                pressure(m.ln_overlap_pressure, 1.1, 3.1).max(pressure(
                    m.ln_hold_duration_p90,
                    260.0,
                    520.0,
                )),
            ]);
        let release_wall = min_gate(&[
            pressure(m.hold_ratio, 0.85, 0.97),
            pressure(ln.held_release, 0.45, 0.65),
            pressure(m.ln_release_pressure, 32.0, 46.0),
            clamp01((0.5 - ln.inverse_release) / 0.15),
        ]);
        let release = if m.key_count == 7 {
            subtype_gate
                * min_gate(&[
                    pressure(m.hold_ratio, 0.2, 0.46),
                    pressure(ln.active_release, 0.55, 0.88).max(release_wall),
                    pressure(m.ln_release_pressure, 2.5, 6.0),
                    min_gate(&[
                        pressure(ln.hold_p50, 150.0, 330.0),
                        pressure(ln.coordinated_release, 0.34, 0.68).max(pressure(
                            ln.held_release,
                            0.32,
                            0.62,
                        )),
                    ])
                    .max(
                        min_gate(&[
                            pressure(ln.held_release, 0.45, 0.7),
                            pressure(m.hold_ratio, 0.6, 0.85),
                            pressure(ln.active_release, 0.7, 0.9),
                        ])
                        .max(release_wall),
                    ),
                ])
        } else {
            0.0
        };
        let ln_tech_burst =
            pressure(m.fast_row_ratio, 0.18, 0.36).max(pressure(m.row_burst_pressure, 16.0, 26.0));
        let ln_tech_coord = if m.key_count == 4 {
            pressure(ln.tap_while_holding, 0.1, 0.25).max(pressure(ln.head_tail, 0.3, 0.55))
        } else {
            pressure(ln.tap_while_holding, 0.04, 0.11)
                .max(pressure(ln.head_tail, 0.52, 0.72))
                .max(pressure(m.chord_size_change_rate, 0.55, 0.78))
        };
        let ln_tech = subtype_gate
            * min_gate(&[
                ln_tech_burst,
                ln_tech_coord,
                if m.key_count == 4 {
                    pressure(m.hold_ratio, 0.2, 0.4)
                } else {
                    1.0
                },
                pressure(m.tech_pressure, 4.2, 8.4).max(pressure(
                    m.row_interval_entropy,
                    2.0,
                    2.45,
                )),
                clamp01((0.6 - ln.inverse_release) / 0.34),
                clamp01((0.66 - ln.release_only) / 0.22),
            ]);
        let ln_general_coverage = min_gate(&[
            pressure(m.hold_ratio, 0.35, 0.82),
            pressure(m.ln_chord_pressure, 0.32, 0.66),
            pressure(ln.head_tail, 0.35, 0.62),
        ])
        .max(min_gate(&[
            pressure(m.ln_density, 0.12, 0.42),
            pressure(m.ln_release_pressure, 8.0, 24.0),
            pressure(chord, 0.28, 0.62),
        ]));
        let ln_general = subtype_gate
            * ln_general_coverage
            * clamp01((0.7 - inverse.max(release).max(ln_tech)) / 0.4);

        let mut candidates = vec![hit(
            "ln",
            "LN",
            if m.key_count == 7 {
                ln_score * 0.62
            } else {
                ln_score
            },
            confidence,
        )];
        if subtype_keys {
            candidates.extend([
                hit("ln_general", "General", ln_general, confidence),
                hit("ln_inverse", "Inverse", inverse, confidence),
                hit("ln_tech", "Tech", ln_tech, confidence),
            ]);
            if m.key_count == 7 {
                candidates.push(hit("ln_release_pattern", "Release", release, confidence));
            }
        }
        if m.key_count == 4 {
            let jack = (pressure(m.jack_pressure, 75.0, 185.0) * (0.55 + low_chord * 0.35)).max(
                min_gate(&[
                    pressure(m.jack_pressure, 110.0, 200.0),
                    pressure(m.fast_row_ratio, 0.05, 0.28),
                ]),
            );
            candidates.extend([
                hit("jack", "Jack", jack, confidence),
                hit("chordjack", "Chordjack", chordjack_base, confidence),
                hit(
                    "speedjack",
                    "Speedjack",
                    chordjack_base
                        * min_gate(&[
                            pressure(two, 0.18, 0.42),
                            pressure(m.jack_pressure, 115.0, 205.0),
                            clamp01((0.68 - three_plus) / 0.35),
                        ]),
                    confidence,
                ),
                hit(
                    "handjack",
                    "Handjack",
                    chordjack_base
                        * min_gate(&[
                            pressure(three_plus, 0.08, 0.28),
                            pressure(stats.average_chord_size, 2.15, 3.1),
                            pressure(m.jack_pressure, 95.0, 180.0),
                        ]),
                    confidence,
                ),
                hit("tech", "Tech", tech, confidence),
                hit(
                    "stream",
                    "Stream",
                    low_chord * stream_activity * clamp01((150.0 - m.jack_pressure) / 120.0),
                    confidence,
                ),
                hit(
                    "dumpstream",
                    "Dumpstream",
                    low_chord
                        * stream_activity
                        * min_gate(&[
                            pressure(m.row_pattern_entropy, 1.8, 3.5),
                            pressure(m.row_interval_entropy, 1.2, 2.7),
                            clamp01((0.75 - m.rhythm_motif_repeat_ratio) / 0.45),
                        ]),
                    confidence,
                ),
                hit(
                    "jumpstream",
                    "Jumpstream",
                    min_gate(&[
                        pressure(two, 0.14, 0.36),
                        pressure(chord, 0.24, 0.56),
                        pressure(m.jumpstream_pressure, 8.0, 22.0),
                    ]),
                    confidence,
                ),
                hit(
                    "handstream",
                    "Handstream",
                    min_gate(&[
                        pressure(three, 0.06, 0.22),
                        pressure(chord, 0.32, 0.62),
                        pressure(m.sustained_nps_10s, 13.0, 27.0),
                        clamp01((175.0 - m.jack_pressure) / 120.0),
                    ]),
                    confidence,
                ),
                hit(
                    "quadstream",
                    "Quadstream",
                    min_gate(&[
                        pressure(four_plus, 0.015, 0.08),
                        pressure(chord, 0.36, 0.72),
                        pressure(m.sustained_nps_10s, 12.0, 25.0),
                    ]),
                    confidence,
                ),
            ]);
        } else if (6..=8).contains(&m.key_count) {
            let flow_gate = clamp01((0.3 - m.hold_ratio) / 0.22);
            let pattern_gate = clamp01((0.68 - m.hold_ratio) / 0.56);
            let bracket_gate = clamp01((0.62 - m.chord_column_overlap_ratio) / 0.17);
            let bracket_window = ratio(stats.bracket_window_rows, row_count);
            let wide_chordstream = chordstream.max(min_gate(&[
                pressure(chord, 0.2, 0.62),
                pressure(m.chord_size_change_rate, 0.18, 0.52),
            ]));
            let single = single_jack_stats(&features.ordered_notes, &features.ordered_rows);
            let repeat_gate = pressure(
                ratio(stats.multi_overlap_chord_pairs, stats.chord_pairs.max(1)),
                0.16,
                0.26,
            )
            .max(pressure(single.jack1_share, 0.16, 0.26))
            .max(pressure(
                stats.chord_rows as f64 / stats.chord_runs.max(1) as f64,
                4.0,
                8.0,
            ));
            let chordjack = pattern_gate
                * repeat_gate
                * chordjack_base.max(min_gate(&[
                    pressure(chord, 0.34, 0.72),
                    pressure(repeated_chord, 0.04, 0.22),
                ]));
            candidates.extend([
                hit(
                    "delay",
                    "Delay",
                    flow_gate
                        * clamp01((0.8 - chordjack) / 0.05)
                        * pressure(m.off_grid_row_share, 0.2, 0.5),
                    confidence,
                ),
                hit(
                    "jack",
                    "Jack",
                    pattern_gate
                        * pressure(single.jack1_share, 0.08, 0.28).max(pressure(
                            single.trill_run_share,
                            0.03,
                            0.12,
                        )),
                    confidence,
                ),
                hit("chordjack", "Chordjack", chordjack, confidence),
                hit(
                    "tech",
                    "Tech",
                    pattern_gate
                        * tech.max(
                            wide_chordstream
                                * min_gate(&[
                                    pressure(m.row_pattern_change_rate, 0.38, 0.72),
                                    pressure(m.fast_row_ratio, 0.08, 0.36),
                                ]),
                        ),
                    confidence,
                ),
                hit(
                    "bracket",
                    "Bracket",
                    pattern_gate
                        * bracket_gate
                        * min_gate(&[
                            pressure(chord, 0.28, 0.62),
                            pressure(bracket_window, 0.18, 0.38),
                        ]),
                    confidence,
                ),
                hit(
                    "chordstream",
                    "Chordstream",
                    pattern_gate * wide_chordstream * clamp01((165.0 - m.jack_pressure) / 130.0),
                    confidence,
                ),
            ]);
        } else {
            candidates.extend([
                hit("stream", "Stream", low_chord * stream_activity, confidence),
                hit("chordstream", "Chordstream", chordstream, confidence),
                hit("chordjack", "Chordjack", chordjack_base, confidence),
                hit("tech", "Tech", tech, confidence),
            ]);
        }
        candidates.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.label.cmp(b.label))
        });
        let all_patterns = candidates;
        let mut patterns: Vec<_> = all_patterns
            .iter()
            .filter(|p| p.score >= 0.2)
            .take(5)
            .cloned()
            .collect();
        if let Some(ln) = all_patterns.iter().find(|p| p.id == "ln") {
            if (ln.score > 0.0 || m.hold_ratio > 0.0) && patterns.iter().all(|p| p.id != "ln") {
                patterns.push(ln.clone());
            }
        }
        let overflow: Vec<_> = all_patterns
            .iter()
            .filter(|p| {
                is_ln_subtype(p.id) && p.score >= 0.2 && patterns.iter().all(|v| v.id != p.id)
            })
            .cloned()
            .collect();
        patterns.extend(overflow);
        let primary = patterns.first().or_else(|| all_patterns.first()).cloned();
        EzPatternAnalysis {
            key_count: m.key_count,
            primary,
            patterns,
            all_patterns,
            metrics: m.clone(),
            warnings: features.warnings.clone(),
        }
    }
}

#[derive(Default)]
struct RowStats {
    row_count: usize,
    chord_rows: usize,
    two: usize,
    three: usize,
    four_plus: usize,
    three_plus: usize,
    repeated_chords: usize,
    bracket_window_rows: usize,
    average_chord_size: f64,
    chord_pairs: usize,
    multi_overlap_chord_pairs: usize,
    chord_runs: usize,
}
#[derive(Default)]
struct SingleJack {
    jack1_share: f64,
    trill_run_share: f64,
}
#[derive(Default)]
struct LnStats {
    inverse_release: f64,
    release_only: f64,
    head_tail: f64,
    mixed_rows: f64,
    tap_while_holding: f64,
    active_release: f64,
    coordinated_release: f64,
    held_release: f64,
    hold_p50: f64,
    inverse_window_coverage: f64,
}
fn hit(id: &'static str, label: &'static str, value: f64, confidence: f64) -> EzPatternHit {
    EzPatternHit {
        id,
        label,
        score: rounded(value),
        confidence: rounded(value * confidence),
        evidence: "",
    }
}
fn rounded(value: f64) -> f64 {
    (clamp01(value) * 1000.0).round_ties_even() / 1000.0
}
fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}
fn pressure(value: f64, low: f64, high: f64) -> f64 {
    clamp01((value - low) / (high - low).max(0.001))
}
fn min_gate(values: &[f64]) -> f64 {
    values.iter().copied().reduce(f64::min).map_or(0.0, clamp01)
}
fn ratio(value: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        value as f64 / total as f64
    }
}
fn quantile(values: &[f64], q: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[((sorted.len() - 1) as f64 * q).floor() as usize]
}
fn is_ln_subtype(id: &str) -> bool {
    matches!(
        id,
        "ln_general" | "ln_inverse" | "ln_release_pattern" | "ln_tech"
    )
}
fn bit_count(mask: u32) -> usize {
    mask.count_ones() as usize
}
fn row_columns(notes: &[ManiaNote]) -> Vec<usize> {
    let mut columns: Vec<_> = notes.iter().map(|n| n.column).collect();
    columns.sort_unstable();
    columns.dedup();
    columns
}
fn is_roll_between(a: &[usize], b: &[usize]) -> bool {
    !a.is_empty() && !b.is_empty() && (a[0] > b[b.len() - 1] || a[a.len() - 1] < b[0])
}
fn shared(a: &[usize], b: &[usize]) -> usize {
    b.iter().filter(|c| a.contains(c)).count()
}

fn row_pattern_stats(notes: &[ManiaNote], rows: &[EzChartRow], key_count: usize) -> RowStats {
    let mut s = RowStats {
        row_count: rows.len(),
        ..Default::default()
    };
    let (mut prev_chord, mut prev_mask, mut prev_size, mut prev_time) =
        (None, 0u32, 0usize, f64::NEG_INFINITY);
    let (mut total_chord, mut before_prev, mut prev_cols) =
        (0usize, Vec::<usize>::new(), Vec::<usize>::new());
    for row in rows {
        let columns = row_columns(&notes[row.note_range.clone()]);
        let size = columns.len();
        let mask = columns.iter().fold(0u32, |m, c| m | (1u32 << c));
        if size >= 2 {
            s.chord_rows += 1;
            total_chord += size;
            s.repeated_chords += usize::from(prev_chord == Some(mask));
            prev_chord = Some(mask);
            s.chord_runs += usize::from(prev_size < 2);
            if prev_size >= 2 && row.time_ms - prev_time < 1000.0 {
                s.chord_pairs += 1;
                s.multi_overlap_chord_pairs += usize::from(bit_count(mask & prev_mask) >= 2);
            }
        } else {
            prev_chord = None;
        }
        s.two += usize::from(size == 2);
        s.three += usize::from(size == 3);
        s.four_plus += usize::from(size >= 4);
        s.three_plus += usize::from(size >= 3);
        if key_count >= 6
            && size >= 2
            && before_prev.len() >= 2
            && prev_cols.len() >= 2
            && !is_roll_between(&before_prev, &prev_cols)
            && !is_roll_between(&prev_cols, &columns)
            && shared(&before_prev, &prev_cols) == 0
            && shared(&prev_cols, &columns) == 0
        {
            s.bracket_window_rows += 1;
        }
        before_prev = prev_cols;
        prev_cols = columns;
        prev_mask = mask;
        prev_size = size;
        prev_time = row.time_ms;
    }
    s.average_chord_size = if s.chord_rows > 0 {
        total_chord as f64 / s.chord_rows as f64
    } else {
        0.0
    };
    s
}

fn single_jack_stats(notes: &[ManiaNote], rows: &[EzChartRow]) -> SingleJack {
    let masks: Vec<u32> = rows
        .iter()
        .map(|r| {
            notes[r.note_range.clone()]
                .iter()
                .fold(0u32, |m, n| m | (1u32 << n.column))
        })
        .collect();
    let times: Vec<f64> = rows.iter().map(|r| r.time_ms).collect();
    let (mut count, mut jack) = (0usize, 0usize);
    for i in 0..rows.len() {
        let row = &notes[rows[i].note_range.clone()];
        count += row.len();
        if i > 0 && times[i] - times[i - 1] <= 400.0 {
            jack += bit_count(masks[i] & masks[i - 1]);
        }
    }
    let (mut trill, mut i) = (0usize, 0usize);
    while i < masks.len().saturating_sub(5) {
        let (a, b) = (masks[i], masks[i + 1]);
        if a == 0 || b == 0 || a == b || times[i + 1] - times[i] > 200.0 {
            i += 1;
            continue;
        }
        let mut j = i + 2;
        while j < masks.len()
            && masks[j] == (if (j - i) % 2 == 0 { a } else { b })
            && times[j] - times[j - 1] <= 200.0
        {
            j += 1;
        }
        if j - i >= 6 {
            trill += (i..j).map(|n| bit_count(masks[n])).sum::<usize>();
            i = j;
        } else {
            i += 1;
        }
    }
    SingleJack {
        jack1_share: ratio(jack, count),
        trill_run_share: ratio(trill, count),
    }
}

fn inverse_gap_cap(beat: f64) -> f64 {
    if !beat.is_finite() || beat <= 0.0 {
        120.0
    } else {
        (beat * 0.27).clamp(120.0, 250.0)
    }
}

fn ln_pattern_stats(features: &EzChartFeatureExtraction, beat: f64) -> LnStats {
    let mut releases = HashSet::<u64>::new();
    let mut heads = HashSet::<u64>::new();
    let mut events = Vec::<(f64, i32)>::new();
    let mut spans = Vec::new();
    let columns = features
        .notes
        .iter()
        .map(|n| n.column)
        .max()
        .map_or(1, |c| c + 1);
    let mut by_column = vec![Vec::<ManiaNote>::new(); columns];
    for n in &features.notes {
        heads.insert(n.start_time_ms.to_bits());
        if n.column < by_column.len() {
            by_column[n.column].push(*n);
        }
        if n.end_time_ms > n.start_time_ms {
            spans.push(*n);
            releases.insert(n.end_time_ms.to_bits());
            events.push((n.start_time_ms, 1));
            events.push((n.end_time_ms, -1));
        }
    }
    events.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.1.cmp(&a.1)));
    let (mut active, mut eidx, mut mixed, mut tap_holding, mut head_tail) =
        (0i32, 0usize, 0usize, 0usize, 0usize);
    for row in &features.ordered_rows {
        while eidx < events.len() && events[eidx].0 < row.time_ms {
            active = (active + events[eidx].1).max(0);
            eidx += 1;
        }
        let ns = &features.ordered_notes[row.note_range.clone()];
        let hh = ns.iter().any(|n| n.end_time_ms > n.start_time_ms);
        let ht = ns.iter().any(|n| n.end_time_ms <= n.start_time_ms);
        mixed += usize::from(hh && ht);
        tap_holding += usize::from(ht && active > 0);
        head_tail += usize::from(releases.contains(&row.time_ms.to_bits()));
    }
    let release_only = releases.iter().filter(|t| !heads.contains(t)).count();
    let (mut durations, mut gaps) = (Vec::new(), Vec::new());
    let (mut inverse, mut next_count, mut active_release, mut coordinated, mut held_release) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut windows = HashMap::<i64, (usize, usize)>::new();
    for col in &mut by_column {
        col.sort_by(|a, b| {
            a.start_time_ms
                .total_cmp(&b.start_time_ms)
                .then_with(|| a.end_time_ms.total_cmp(&b.end_time_ms))
        });
        for i in 0..col.len() {
            let n = col[i];
            if n.end_time_ms <= n.start_time_ms {
                continue;
            }
            let dur = (n.end_time_ms - n.start_time_ms).max(1.0);
            durations.push(dur);
            let next = col.get(i + 1);
            let gap = next.map_or(f64::INFINITY, |x| x.start_time_ms - n.end_time_ms);
            if next.is_some() && gap >= 0.0 {
                next_count += 1;
                gaps.push(gap);
                let pair = windows
                    .entry((n.start_time_ms / INVERSE_WINDOW_MS).floor() as i64)
                    .or_default();
                pair.0 += 1;
                if gap <= inverse_gap_cap(beat) && gap <= dur {
                    pair.1 += 1;
                }
            }
            if gap >= 0.0 && gap <= inverse_gap_cap(beat) && gap / dur <= 0.7 {
                if next.is_some() {
                    inverse += 1;
                }
                continue;
            }
            active_release += 1;
            if heads.contains(&n.end_time_ms.to_bits()) {
                coordinated += 1;
            }
            held_release += usize::from(
                spans
                    .iter()
                    .any(|h| h.start_time_ms < n.end_time_ms && h.end_time_ms > n.end_time_ms),
            );
        }
    }
    let judged: Vec<_> = windows
        .values()
        .filter(|(n, _)| *n >= INVERSE_WINDOW_MIN_PAIRS)
        .collect();
    LnStats {
        inverse_release: ratio(inverse, next_count),
        release_only: ratio(release_only, releases.len()),
        head_tail: ratio(head_tail, features.ordered_rows.len().max(1)),
        mixed_rows: ratio(mixed, features.ordered_rows.len().max(1)),
        tap_while_holding: ratio(tap_holding, features.ordered_rows.len().max(1)),
        active_release: ratio(active_release, durations.len().max(1)),
        coordinated_release: ratio(coordinated, durations.len().max(1)),
        held_release: ratio(held_release, durations.len().max(1)),
        hold_p50: quantile(&durations, 0.5),
        inverse_window_coverage: if judged.len() >= INVERSE_WINDOW_MIN_WINDOWS {
            ratio(
                judged
                    .iter()
                    .filter(|(pairs, inverse)| {
                        *inverse as f64 / *pairs as f64 >= INVERSE_WINDOW_RATIO
                    })
                    .count(),
                judged.len(),
            )
        } else {
            0.0
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn analyze(key: usize, notes: &[ManiaNote], bpm: f64) -> EzPatternAnalysis {
        let map = EzManiaChartInput {
            key_count: key,
            bpm,
            total_length_ms: notes.iter().map(|n| n.end_time_ms).fold(0.0, f64::max),
            notes,
            timing_points: &[],
        };
        EzPatternAnalyzer::analyze(map, 1.0)
    }
    #[test]
    fn tag_ids_thresholds_and_key_classification_match_ez_rules() {
        assert_eq!(
            RcPatternAxis::ALL.map(RcPatternAxis::id),
            [
                "chordstream",
                "bracket",
                "delay",
                "stream",
                "jack",
                "tech",
                "ln",
                "chordjack",
                "speedjack",
                "handjack",
                "jumpstream",
                "handstream",
                "dumpstream",
                "quadstream"
            ]
        );
        assert_eq!(
            LnPatternAxis::ALL.map(LnPatternAxis::id),
            [
                "ln_density",
                "ln_overlap",
                "ln_release",
                "ln_chord",
                "ln_hold_p90",
                "ln_general",
                "ln_inverse",
                "ln_release_pattern",
                "ln_tech"
            ]
        );
        assert_eq!(pattern_tag_min_score("chordjack"), CHORDJACK_TAG_MIN_SCORE);
        assert_eq!(pattern_tag_min_score("delay"), DELAY_TAG_MIN_SCORE);
        assert_eq!(ln_primary_min_ratio(7), LN_PRIMARY_7K_MIN_RATIO);
        assert!(!chart_is_jack(Some(4), 0.2, 0.5, None));
        assert!(chart_is_jack(Some(7), 0.2, 0.5, None));
        assert!(!jack_vetoes_tech(Some(7), 0.2, 0.5, None));
    }
    #[test]
    fn four_key_chord_loop_has_complete_sorted_candidate_vocabulary() {
        let mut notes = Vec::new();
        for row in 0..80 {
            let t = row as f64 * 120.0;
            let cols = if row % 2 == 0 { [0, 1] } else { [2, 3] };
            for c in cols {
                notes.push(ManiaNote {
                    start_time_ms: t,
                    end_time_ms: t,
                    column: c,
                });
            }
        }
        let result = analyze(4, &notes, 150.0);
        assert_eq!(result.all_patterns.len(), 14);
        assert!(
            result
                .all_patterns
                .windows(2)
                .all(|p| p[0].score >= p[1].score)
        );
        assert!(
            result
                .all_patterns
                .iter()
                .all(|p| (0.0..=1.0).contains(&p.score))
        );
        assert!(result.all_patterns.iter().any(|p| p.id == "chordjack"));
    }
    #[test]
    fn seven_key_ln_signal_and_subtypes_are_retained_even_below_top_five() {
        let mut notes = Vec::new();
        for row in 0..100 {
            let t = row as f64 * 300.0;
            let col = row % 7;
            let end = t + if row % 2 == 0 { 250.0 } else { 0.0 };
            notes.push(ManiaNote {
                start_time_ms: t,
                end_time_ms: end,
                column: col,
            });
        }
        let result = analyze(7, &notes, 120.0);
        assert!(result.all_patterns.iter().any(|p| p.id == "ln"));
        assert!(
            result
                .all_patterns
                .iter()
                .any(|p| p.id == "ln_release_pattern")
        );
        assert!(result.patterns.iter().any(|p| p.id == "ln"));
        assert_eq!(result.key_count, 7);
    }
    #[test]
    fn precomputed_features_give_same_result_as_direct_analysis() {
        let notes: Vec<_> = (0..100)
            .map(|i| ManiaNote {
                start_time_ms: i as f64 * 100.0,
                end_time_ms: if i % 4 == 0 {
                    i as f64 * 100.0 + 250.0
                } else {
                    i as f64 * 100.0
                },
                column: i % 4,
            })
            .collect();
        let map = EzManiaChartInput {
            key_count: 4,
            bpm: 150.0,
            total_length_ms: 10000.0,
            notes: &notes,
            timing_points: &[],
        };
        let features = EzChartFeatureExtractor::extract(map, 1.0).unwrap();
        assert_eq!(
            EzPatternAnalyzer::analyze_features(map, 1.0, &features),
            EzPatternAnalyzer::analyze(map, 1.0)
        );
    }
}

/// Pattern threshold and classification helpers from EzPatternTagRules.
pub fn pattern_tag_min_score(id: &str) -> f64 {
    match id {
        "chordjack" => CHORDJACK_TAG_MIN_SCORE,
        "delay" => DELAY_TAG_MIN_SCORE,
        _ => PATTERN_TAG_MIN_SCORE,
    }
}
pub fn ln_primary_min_ratio(key_count: usize) -> f64 {
    if key_count == 7 {
        LN_PRIMARY_7K_MIN_RATIO
    } else {
        LN_PRIMARY_MIN_RATIO
    }
}
pub fn uses_pattern_skill_axes(key_count: Option<usize>) -> bool {
    matches!(key_count, Some(6 | 7 | 8))
}
pub fn chart_is_jack(
    key_count: Option<usize>,
    chordjack: f64,
    jack: f64,
    jack_share: Option<f64>,
) -> bool {
    if !uses_pattern_skill_axes(key_count) {
        return chordjack >= CHORDJACK_TAG_MIN_SCORE;
    }
    if jack >= PATTERN_TAG_MIN_SCORE {
        return true;
    }
    jack_share.map_or(chordjack >= CHORDJACK_TAG_MIN_SCORE, |share| {
        share >= CLUSTER_SHARE_MIN
    })
}
pub fn jack_vetoes_tech(
    key_count: Option<usize>,
    chordjack: f64,
    jack: f64,
    jack_share: Option<f64>,
) -> bool {
    if !uses_pattern_skill_axes(key_count) {
        return chordjack >= CHORDJACK_TAG_MIN_SCORE;
    }
    if jack >= JACK_TECH_VETO_MIN_SCORE {
        return true;
    }
    jack_share.map_or(chordjack >= CHORDJACK_TAG_MIN_SCORE, |share| {
        share >= CLUSTER_SHARE_MIN
    })
}
