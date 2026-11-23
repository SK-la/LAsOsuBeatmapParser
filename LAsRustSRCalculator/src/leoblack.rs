//! Pure chart-row and primitive extraction for Ez2Lazer's LeoBlack Mania analyzer.
//!
//! This module deliberately stops before window pattern matching and cluster aggregation. It consumes the same
//! normalized note/timing view as CSI, so it does not depend on UI, database, or osu!framework types.

use crate::csi_features::EzManiaChartInput;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EzLeoBlackNoteType {
    Nothing,
    Normal,
    HoldHead,
    HoldBody,
    HoldTail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EzLeoBlackDirection {
    None,
    Left,
    Right,
    Outwards,
    Inwards,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EzLeoBlackBpm {
    pub meter: i32,
    pub ms_per_beat: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EzLeoBlackTimeItem<T> {
    pub time: f64,
    pub data: T,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EzLeoBlackChart {
    pub keys: usize,
    pub notes: Vec<EzLeoBlackTimeItem<Vec<EzLeoBlackNoteType>>>,
    pub bpm: Vec<EzLeoBlackTimeItem<EzLeoBlackBpm>>,
}

impl EzLeoBlackChart {
    pub fn first_note(&self) -> Option<f64> {
        self.notes.first().map(|note| note.time)
    }

    pub fn last_note(&self) -> Option<f64> {
        self.notes.last().map(|note| note.time)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EzLeoBlackPrimitive {
    pub index: usize,
    /// Time relative to the first chart row, in milliseconds.
    pub time: f64,
    /// Four times the gap from the preceding non-empty primitive row, matching the hub primitive model.
    pub ms_per_beat: f64,
    pub beat_length: f64,
    pub notes: usize,
    pub jacks: usize,
    pub direction: EzLeoBlackDirection,
    pub roll: bool,
    pub keys: usize,
    pub left_hand_keys: usize,
    pub ln_heads: Vec<usize>,
    pub ln_bodies: Vec<usize>,
    pub ln_tails: Vec<usize>,
    pub normal_notes: Vec<usize>,
    pub raw_notes: Vec<usize>,
}

#[derive(Clone, Copy)]
struct CellEvent {
    time: f64,
    column: usize,
    kind: EzLeoBlackNoteType,
}

#[derive(Clone, Copy)]
struct HoldSpan {
    column: usize,
    start: f64,
    end: f64,
}

#[derive(Clone, Copy)]
struct BodyEvent {
    row_index: usize,
    column: usize,
    delta: i32,
}

pub struct EzLeoBlackChartBuilder;

impl EzLeoBlackChartBuilder {
    /// Builds the sparse row grid used by LeoBlack. Invalid-time notes and out-of-range columns are skipped, as in
    /// Ez2Lazer; a zero-key or empty chart produces `None`.
    pub fn try_build(input: EzManiaChartInput<'_>) -> Option<EzLeoBlackChart> {
        let keys = input.key_count;
        if keys == 0 || input.notes.is_empty() {
            return None;
        }

        let mut cell_events = Vec::with_capacity(input.notes.len().saturating_mul(2));
        let mut hold_spans = Vec::new();

        for note in input.notes {
            let start = note.start_time_ms;
            let end = note.end_time_ms;
            if note.column >= keys || !start.is_finite() {
                continue;
            }

            if end.is_finite() && end > start {
                cell_events.push(CellEvent {
                    time: start,
                    column: note.column,
                    kind: EzLeoBlackNoteType::HoldHead,
                });
                cell_events.push(CellEvent {
                    time: end,
                    column: note.column,
                    kind: EzLeoBlackNoteType::HoldTail,
                });
                hold_spans.push(HoldSpan {
                    column: note.column,
                    start,
                    end,
                });
            } else {
                cell_events.push(CellEvent {
                    time: start,
                    column: note.column,
                    kind: EzLeoBlackNoteType::Normal,
                });
            }
        }

        if cell_events.is_empty() {
            return None;
        }

        // Stable sorting preserves input order when multiple objects occupy the same cell and timestamp. C#'s
        // Dictionary<double, ...> also treats -0.0 and 0.0 as the same key; partial_cmp does likewise.
        cell_events.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        let mut rows: Vec<EzLeoBlackTimeItem<Vec<EzLeoBlackNoteType>>> = Vec::new();

        for event in cell_events {
            let is_new_row = rows.last().is_none_or(|row| row.time != event.time);
            if is_new_row {
                rows.push(EzLeoBlackTimeItem {
                    time: event.time,
                    data: vec![EzLeoBlackNoteType::Nothing; keys],
                });
            }

            let row = rows.last_mut().expect("the event created or found a row");
            let cell = &mut row.data[event.column];
            if *cell == EzLeoBlackNoteType::Nothing {
                *cell = event.kind;
            }
        }

        let sorted_times: Vec<f64> = rows.iter().map(|row| row.time).collect();
        let mut body_events = Vec::with_capacity(hold_spans.len().saturating_mul(2));
        for span in hold_spans {
            let from = lower_bound_greater(&sorted_times, span.start);
            let to = lower_bound_at_or_above(&sorted_times, span.end);
            if to > from {
                body_events.push(BodyEvent {
                    row_index: from,
                    column: span.column,
                    delta: 1,
                });
                body_events.push(BodyEvent {
                    row_index: to,
                    column: span.column,
                    delta: -1,
                });
            }
        }

        body_events.sort_by_key(|event| event.row_index);
        let mut active_holds = vec![0_i32; keys];
        let mut body_index = 0;
        for (row_index, row) in rows.iter_mut().enumerate() {
            while body_index < body_events.len() && body_events[body_index].row_index <= row_index {
                let event = body_events[body_index];
                active_holds[event.column] += event.delta;
                body_index += 1;
            }

            for (column, cell) in row.data.iter_mut().enumerate() {
                if active_holds[column] > 0 && *cell == EzLeoBlackNoteType::Nothing {
                    *cell = EzLeoBlackNoteType::HoldBody;
                }
            }
        }

        rows.retain(|row| {
            row.data.iter().any(|kind| {
                !matches!(
                    kind,
                    EzLeoBlackNoteType::Nothing | EzLeoBlackNoteType::HoldBody
                )
            })
        });
        if rows.is_empty() {
            return None;
        }

        let mut bpm = Vec::new();
        for &(time, ms_per_beat) in input.timing_points {
            if ms_per_beat.is_finite() && ms_per_beat > 0.0 {
                bpm.push(EzLeoBlackTimeItem {
                    time,
                    data: EzLeoBlackBpm {
                        meter: 4,
                        ms_per_beat,
                    },
                });
            }
        }
        if bpm.is_empty() {
            let ms_per_beat = if input.bpm.is_finite() && input.bpm > 0.0 {
                60_000.0 / input.bpm
            } else {
                500.0
            };
            bpm.push(EzLeoBlackTimeItem {
                time: 0.0,
                data: EzLeoBlackBpm {
                    meter: 4,
                    ms_per_beat,
                },
            });
        }

        Some(EzLeoBlackChart {
            keys,
            notes: rows,
            bpm,
        })
    }
}

pub struct EzLeoBlackPrimitives;

impl EzLeoBlackPrimitives {
    pub fn detect_direction(
        previous_row: &[usize],
        current_row: &[usize],
    ) -> (EzLeoBlackDirection, bool) {
        let pleftmost = previous_row[0] as isize;
        let prightmost = *previous_row.last().unwrap() as isize;
        let cleftmost = current_row[0] as isize;
        let crightmost = *current_row.last().unwrap() as isize;

        let leftmost_change = cleftmost - pleftmost;
        let rightmost_change = crightmost - prightmost;

        let direction = if leftmost_change > 0 {
            if rightmost_change > 0 {
                EzLeoBlackDirection::Right
            } else {
                EzLeoBlackDirection::Inwards
            }
        } else if leftmost_change < 0 {
            if rightmost_change < 0 {
                EzLeoBlackDirection::Left
            } else {
                EzLeoBlackDirection::Outwards
            }
        } else if rightmost_change < 0 {
            EzLeoBlackDirection::Inwards
        } else if rightmost_change > 0 {
            EzLeoBlackDirection::Outwards
        } else {
            EzLeoBlackDirection::None
        };

        let is_roll = pleftmost > crightmost || prightmost < cleftmost;
        (direction, is_roll)
    }

    pub fn calculate(chart: &EzLeoBlackChart) -> Vec<EzLeoBlackPrimitive> {
        let Some(first) = chart.notes.first() else {
            return Vec::new();
        };
        let first_note = first.time;
        let mut previous_row: Vec<usize> = first
            .data
            .iter()
            .enumerate()
            .filter_map(|(column, kind)| {
                matches!(
                    kind,
                    EzLeoBlackNoteType::Normal | EzLeoBlackNoteType::HoldHead
                )
                .then_some(column)
            })
            .collect();
        if previous_row.is_empty() {
            return Vec::new();
        }

        let mut previous_time = first_note;
        let left_hand_keys = keys_on_left_hand(chart.keys);
        let mut primitives = Vec::with_capacity(chart.notes.len().saturating_sub(1));

        for (row_index, item) in chart.notes.iter().enumerate().skip(1) {
            let mut current_row = Vec::new();
            let mut normal_notes = Vec::new();
            let mut ln_heads = Vec::new();
            let mut ln_bodies = Vec::new();
            let mut ln_tails = Vec::new();

            for (column, kind) in item.data.iter().enumerate() {
                match kind {
                    EzLeoBlackNoteType::Normal => {
                        current_row.push(column);
                        normal_notes.push(column);
                    }
                    EzLeoBlackNoteType::HoldHead => {
                        current_row.push(column);
                        ln_heads.push(column);
                    }
                    EzLeoBlackNoteType::HoldBody => ln_bodies.push(column),
                    EzLeoBlackNoteType::HoldTail => ln_tails.push(column),
                    EzLeoBlackNoteType::Nothing => {}
                }
            }

            if current_row.is_empty()
                && ln_heads.is_empty()
                && ln_bodies.is_empty()
                && ln_tails.is_empty()
            {
                continue;
            }

            let mut direction = EzLeoBlackDirection::None;
            let mut is_roll = false;
            let mut jacks = 0;
            if !current_row.is_empty() {
                (direction, is_roll) = Self::detect_direction(&previous_row, &current_row);
                jacks = current_row
                    .iter()
                    .filter(|column| previous_row.contains(column))
                    .count();
            }

            let time = item.time;
            primitives.push(EzLeoBlackPrimitive {
                index: row_index,
                time: time - first_note,
                ms_per_beat: (time - previous_time) * 4.0,
                beat_length: beat_length_at(chart, time),
                notes: current_row.len(),
                jacks,
                direction,
                roll: is_roll,
                keys: chart.keys,
                left_hand_keys,
                ln_heads,
                ln_bodies,
                ln_tails,
                normal_notes,
                raw_notes: current_row.clone(),
            });

            if !current_row.is_empty() {
                previous_row = current_row;
            }
            previous_time = time;
        }

        primitives
    }

    pub fn ln_percent(chart: &EzLeoBlackChart) -> f64 {
        let mut notes = 0usize;
        let mut holds = 0usize;
        for row in &chart.notes {
            for kind in &row.data {
                match kind {
                    EzLeoBlackNoteType::Normal => notes += 1,
                    EzLeoBlackNoteType::HoldHead => {
                        notes += 1;
                        holds += 1;
                    }
                    _ => {}
                }
            }
        }
        if notes == 0 {
            0.0
        } else {
            holds as f64 / notes as f64
        }
    }
}

fn keys_on_left_hand(keymode: usize) -> usize {
    match keymode {
        3 | 4 => 2,
        5 | 6 => 3,
        7 | 8 => 4,
        9 | 10 => 5,
        _ => (keymode / 2).max(1),
    }
}

fn beat_length_at(chart: &EzLeoBlackChart, time: f64) -> f64 {
    let Some(first) = chart.bpm.first() else {
        return 500.0;
    };
    let mut current = first.data.ms_per_beat;
    for item in &chart.bpm {
        if item.time > time {
            break;
        }
        current = item.data.ms_per_beat;
    }
    current
}

fn lower_bound_greater(sorted: &[f64], target: f64) -> usize {
    let mut lo = 0;
    let mut hi = sorted.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if sorted[mid] <= target {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

fn lower_bound_at_or_above(sorted: &[f64], target: f64) -> usize {
    let mut lo = 0;
    let mut hi = sorted.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if sorted[mid] < target {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

const CORE_PATTERNS: [&str; 6] = [
    "Stream",
    "Chordstream",
    "Jacks",
    "Coordination",
    "Density",
    "Wildcard",
];

#[derive(Clone, Debug, PartialEq)]
pub struct EzLeoBlackFoundPattern {
    pub pattern: &'static str,
    pub specific_type: Option<&'static str>,
    pub mixed: bool,
    pub start: f64,
    pub end: f64,
    pub ms_per_beat: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EzLeoBlackSpecificTypeRatio {
    pub name: &'static str,
    pub ratio: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EzLeoBlackCluster {
    pub pattern: &'static str,
    pub bpm: f64,
    pub amount: f64,
    pub importance: f64,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EzLeoBlackPatternResult {
    pub top_five_clusters: Vec<EzLeoBlackCluster>,
    pub category: String,
    pub clusters: Vec<EzLeoBlackCluster>,
}

#[derive(Clone, Debug)]
struct InternalCluster {
    pattern: &'static str,
    specific_types: Vec<EzLeoBlackSpecificTypeRatio>,
    rating_multiplier: f64,
    bpm: f64,
    mixed: bool,
    amount: f64,
}

impl InternalCluster {
    fn importance(&self) -> f64 {
        self.amount * self.rating_multiplier * self.bpm
    }

    fn format(&self) -> String {
        let name = self
            .specific_types
            .first()
            .filter(|specific| specific.ratio >= 0.0)
            .map_or(self.pattern, |specific| specific.name);
        let bpm = self.bpm.round_ties_even() as i64;
        if self.mixed {
            format!("{bpm}BPM Mixed {name}")
        } else {
            format!("{bpm}BPM {name}")
        }
    }
}

pub struct EzLeoBlackPatternAnalyzer;

impl EzLeoBlackPatternAnalyzer {
    /// Runs the pure chart-only LeoBlack pipeline. `None` matches Ez2Lazer's null result for empty or unusable
    /// chart input.
    pub fn try_analyze(input: EzManiaChartInput<'_>) -> Option<EzLeoBlackPatternResult> {
        if input.notes.is_empty() || input.key_count == 0 {
            return None;
        }
        let chart = EzLeoBlackChartBuilder::try_build(input)?;
        let first = chart.first_note()?;
        let last = chart.last_note()?;
        let ln_ratio = EzLeoBlackPrimitives::ln_percent(&chart);
        let mode_tag = resolve_mode_tag(ln_ratio, hb_row_ratio(&chart));
        let primitives = EzLeoBlackPrimitives::calculate(&chart);
        let patterns = find_patterns(chart.keys, last - first, &primitives);
        let filtered_patterns: Vec<_> = if mode_tag == "RC" {
            patterns
                .into_iter()
                .filter(|pattern| {
                    !matches!(pattern.pattern, "Coordination" | "Density" | "Wildcard")
                })
                .collect()
        } else {
            patterns
        };

        let mut clusters: Vec<_> = calculate_clustered_patterns(&filtered_patterns, mode_tag)
            .into_iter()
            .filter(|cluster| cluster.bpm > 25.0 || cluster.bpm == 0.0)
            .collect();
        clusters.sort_by(|a, b| b.amount.total_cmp(&a.amount));

        let filtered: Vec<_> = clusters
            .iter()
            .filter(|cluster| {
                !clusters.iter().any(|other| {
                    other.pattern == cluster.pattern
                        && other.amount * 0.5 > cluster.amount
                        && other.bpm > cluster.bpm
                })
            })
            .cloned()
            .collect();
        let mut pruned_clusters = Vec::new();
        for pattern in CORE_PATTERNS {
            pruned_clusters.extend(
                filtered
                    .iter()
                    .filter(|cluster| cluster.pattern == pattern)
                    .take(3)
                    .cloned(),
            );
        }
        pruned_clusters.sort_by(|a, b| b.importance().total_cmp(&a.importance()));

        let internal_category = categorise(&pruned_clusters);
        let public_clusters: Vec<_> = pruned_clusters
            .iter()
            .map(|cluster| EzLeoBlackCluster {
                pattern: cluster.pattern,
                bpm: cluster.bpm,
                amount: cluster.amount,
                importance: cluster.importance(),
                label: cluster.format(),
            })
            .collect();

        Some(EzLeoBlackPatternResult {
            top_five_clusters: public_clusters.iter().take(5).cloned().collect(),
            category: internal_category,
            clusters: public_clusters,
        })
    }
}

pub struct EzLeoBlackClusterMetrics;

impl EzLeoBlackClusterMetrics {
    /// Importance-weighted share for clusters whose pattern satisfies `pattern_match`.
    pub fn cluster_share(
        clusters: &[EzLeoBlackCluster],
        pattern_match: impl Fn(&str) -> bool,
    ) -> Option<f64> {
        let mut total = 0.0;
        let mut matched = 0.0;
        for cluster in clusters {
            let importance = cluster.importance;
            if !importance.is_finite() || importance <= 0.0 {
                continue;
            }
            total += importance;
            if pattern_match(cluster.pattern) {
                matched += importance;
            }
        }
        (total > 0.0).then_some(matched / total)
    }

    pub fn jack_share(clusters: &[EzLeoBlackCluster]) -> Option<f64> {
        Self::cluster_share(clusters, |pattern| {
            pattern.to_ascii_lowercase().contains("jack")
        })
    }

    pub fn stream_share(clusters: &[EzLeoBlackCluster]) -> Option<f64> {
        Self::cluster_share(clusters, |pattern| {
            pattern.to_ascii_lowercase().contains("stream")
        })
    }

    pub fn tech_category(category: Option<&str>) -> bool {
        category
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("tech")
    }

    pub fn cluster_trill(category: Option<&str>) -> bool {
        category
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("trill")
    }

    pub fn handstream_cluster(category: Option<&str>) -> bool {
        category
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("handstream")
    }
}

fn mode_tag_from_ln_ratio(ln_ratio: f64) -> &'static str {
    if !ln_ratio.is_finite() {
        "Mix"
    } else if ln_ratio <= 0.15 {
        "RC"
    } else if ln_ratio >= 0.9 {
        "LN"
    } else {
        "Mix"
    }
}

fn resolve_mode_tag(ln_ratio: f64, hb_ratio: f64) -> &'static str {
    let tag = mode_tag_from_ln_ratio(ln_ratio);
    if tag != "Mix" {
        tag
    } else if hb_ratio >= 0.1 {
        "HB"
    } else {
        "Mix"
    }
}

fn hb_row_ratio(chart: &EzLeoBlackChart) -> f64 {
    if chart.notes.is_empty() {
        return 0.0;
    }
    let hb_rows = chart
        .notes
        .iter()
        .filter(|row| {
            row.data.contains(&EzLeoBlackNoteType::HoldHead)
                && row.data.contains(&EzLeoBlackNoteType::Normal)
        })
        .count();
    hb_rows as f64 / chart.notes.len() as f64
}

type PatternMatcher = fn(&[EzLeoBlackPrimitive]) -> usize;

#[derive(Clone, Copy)]
struct SpecificPatterns {
    stream: &'static [(&'static str, PatternMatcher)],
    chordstream: &'static [(&'static str, PatternMatcher)],
    jacks: &'static [(&'static str, PatternMatcher)],
    coordination: &'static [(&'static str, PatternMatcher)],
    density: &'static [(&'static str, PatternMatcher)],
    wildcard: &'static [(&'static str, PatternMatcher)],
}

const FOUR_KEY_STREAM: &[(&str, PatternMatcher)] = &[
    ("Rolls", stream_4k_roll),
    ("Trills", stream_4k_trill),
    ("Minitrills", stream_4k_minitrill),
];
const FOUR_KEY_CHORDSTREAM: &[(&str, PatternMatcher)] = &[
    ("Handstream", chordstream_4k_handstream),
    ("Split Trill", chordstream_4k_splittrill),
    ("Jumptrill", chordstream_4k_jumptrill),
    ("Jumpstream", chordstream_4k_jumpstream),
];
const FOUR_KEY_JACKS: &[(&str, PatternMatcher)] = &[
    ("Longjacks", jacks_longjacks),
    ("Quadstream", jacks_4k_quadstream),
    ("Gluts", jacks_4k_gluts),
    ("Chordjacks", jacks_chordjacks),
    ("Minijacks", jacks_minijacks),
];
const SEVEN_KEY_CHORDSTREAM: &[(&str, PatternMatcher)] = &[
    ("Brackets", chordstream_7k_brackets),
    ("Double Stream", chordstream_7k_double_streams),
    ("Dense Chordstream", chordstream_7k_dense_chordstream),
    ("Light Chordstream", chordstream_7k_light_chordstream),
];
const OTHER_KEY_CHORDSTREAM: &[(&str, PatternMatcher)] = &[
    ("Chord Rolls", chordstream_7k_chord_roll),
    ("Double Stream", chordstream_7k_double_streams),
    ("Dense Chordstream", chordstream_7k_dense_chordstream),
    ("Light Chordstream", chordstream_7k_light_chordstream),
];
const SEVEN_KEY_JACKS: &[(&str, PatternMatcher)] = &[
    ("Longjacks", jacks_longjacks),
    ("Chordjacks", jacks_chordjacks),
    ("Minijacks", jacks_minijacks),
];
const COORDINATION: &[(&str, PatternMatcher)] = &[
    ("Column Lock", coordination_column_lock),
    ("Release", coordination_release),
    ("Shield", coordination_shield),
];
const FOUR_KEY_DENSITY: &[(&str, PatternMatcher)] = &[
    ("Inverse", density_inverse),
    ("JS Density", density_4k_jumpstream),
    ("HS Density", density_4k_handstream),
];
const SEVEN_KEY_DENSITY: &[(&str, PatternMatcher)] = &[
    ("Inverse", density_inverse),
    ("DS Density", density_7k_double_streams),
    ("DCS Density", density_7k_dense_chordstream),
    ("LCS Density", density_7k_light_chordstream),
];
const FOUR_KEY_WILDCARD: &[(&str, PatternMatcher)] =
    &[("Speedy WC", wildcard_speed), ("Jacky WC", wildcard_jack)];

fn specific_patterns(keys: usize) -> SpecificPatterns {
    match keys {
        4 => SpecificPatterns {
            stream: FOUR_KEY_STREAM,
            chordstream: FOUR_KEY_CHORDSTREAM,
            jacks: FOUR_KEY_JACKS,
            coordination: COORDINATION,
            density: FOUR_KEY_DENSITY,
            wildcard: FOUR_KEY_WILDCARD,
        },
        7 => SpecificPatterns {
            stream: &[],
            chordstream: SEVEN_KEY_CHORDSTREAM,
            jacks: SEVEN_KEY_JACKS,
            coordination: COORDINATION,
            density: SEVEN_KEY_DENSITY,
            wildcard: FOUR_KEY_WILDCARD,
        },
        _ => SpecificPatterns {
            stream: &[],
            chordstream: OTHER_KEY_CHORDSTREAM,
            jacks: SEVEN_KEY_JACKS,
            coordination: COORDINATION,
            density: SEVEN_KEY_DENSITY,
            wildcard: FOUR_KEY_WILDCARD,
        },
    }
}

fn find_patterns(
    keys: usize,
    last_note: f64,
    primitives: &[EzLeoBlackPrimitive],
) -> Vec<EzLeoBlackFoundPattern> {
    const WINDOW: usize = 8;
    let table = specific_patterns(keys);
    let mut output = Vec::new();
    for start in 0..primitives.len() {
        let end = (start + WINDOW).min(primitives.len());
        let window = &primitives[start..end];
        append_core_matches(
            &mut output,
            "Stream",
            core_stream(window),
            table.stream,
            window,
            last_note,
        );
        append_core_matches(
            &mut output,
            "Chordstream",
            core_chordstream(window),
            table.chordstream,
            window,
            last_note,
        );
        append_core_matches(
            &mut output,
            "Jacks",
            core_jacks(window),
            table.jacks,
            window,
            last_note,
        );
        append_core_matches(
            &mut output,
            "Coordination",
            core_coordination(window),
            table.coordination,
            window,
            last_note,
        );
        append_core_matches(
            &mut output,
            "Density",
            core_density(window),
            table.density,
            window,
            last_note,
        );
        append_core_matches(
            &mut output,
            "Wildcard",
            core_wildcard(window),
            table.wildcard,
            window,
            last_note,
        );
    }
    output
}

fn append_core_matches(
    output: &mut Vec<EzLeoBlackFoundPattern>,
    pattern: &'static str,
    core_count: usize,
    specific_patterns: &[(&'static str, PatternMatcher)],
    window: &[EzLeoBlackPrimitive],
    last_note: f64,
) {
    if core_count == 0 {
        return;
    }
    let matches: Vec<_> = specific_patterns
        .iter()
        .filter_map(|(name, matcher)| {
            let count = matcher(window);
            (count != 0).then_some((count, *name))
        })
        .collect();
    if matches.is_empty() {
        append_found_pattern(output, pattern, None, core_count, window, last_note);
    } else {
        for (specific_count, specific_type) in matches {
            append_found_pattern(
                output,
                pattern,
                Some(specific_type),
                core_count.max(specific_count),
                window,
                last_note,
            );
        }
    }
}

fn append_found_pattern(
    output: &mut Vec<EzLeoBlackFoundPattern>,
    pattern: &'static str,
    specific_type: Option<&'static str>,
    count: usize,
    window: &[EzLeoBlackPrimitive],
    last_note: f64,
) {
    let count = count.min(window.len());
    let mean_ms_per_beat = window[..count]
        .iter()
        .fold(0.0, |sum, row| sum + row.ms_per_beat)
        / count as f64;
    let mixed = window[..count]
        .iter()
        .any(|row| !((row.ms_per_beat - mean_ms_per_beat).abs() < 5.0));
    let first = &window[0];
    let end = if pattern == "Jacks" {
        let end_candidate = if count < window.len() {
            window[count].time
        } else {
            last_note
        };
        (first.time + first.ms_per_beat * 0.5).max(end_candidate)
    } else if count < window.len() {
        window[count].time
    } else {
        last_note
    };
    let ms_per_beat = if pattern == "Density" && specific_type == Some("Inverse") {
        0.0
    } else {
        mean_ms_per_beat
    };
    output.push(EzLeoBlackFoundPattern {
        pattern,
        specific_type,
        mixed,
        start: first.time,
        end,
        ms_per_beat,
    });
}

fn same_cols(a: &[usize], b: &[usize]) -> bool {
    a == b
}

fn contains_col(columns: &[usize], column: usize) -> bool {
    columns.contains(&column)
}

fn core_stream(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() >= 5
        && rows[..5].iter().all(|row| row.notes == 1 && row.jacks == 0)
        && rows[0].raw_notes[0] != rows[4].raw_notes[0]
    {
        5
    } else {
        0
    }
}

fn core_jacks(rows: &[EzLeoBlackPrimitive]) -> usize {
    rows.first()
        .is_some_and(|row| row.jacks > 1 && row.ms_per_beat < 2000.0)
        .then_some(1)
        .unwrap_or(0)
}

fn core_chordstream(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 4 {
        return 0;
    }
    let a = &rows[0];
    let b = &rows[1];
    let c = &rows[2];
    let d = &rows[3];
    if a.notes > 1
        && a.jacks == 0
        && b.jacks == 0
        && c.jacks == 0
        && d.jacks == 0
        && (b.notes > 1 || c.notes > 1 || d.notes > 1)
    {
        4
    } else {
        0
    }
}

fn core_coordination(rows: &[EzLeoBlackPrimitive]) -> usize {
    rows.first()
        .is_some_and(|row| {
            !row.ln_heads.is_empty() || !row.ln_bodies.is_empty() || !row.ln_tails.is_empty()
        })
        .then_some(1)
        .unwrap_or(0)
}

fn core_density(rows: &[EzLeoBlackPrimitive]) -> usize {
    core_coordination(rows)
}

fn core_wildcard(rows: &[EzLeoBlackPrimitive]) -> usize {
    core_coordination(rows)
}

fn jacks_chordjacks(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 2 {
        return 0;
    }
    let a = &rows[0];
    let b = &rows[1];
    if a.notes > 2 && b.notes > 1 && b.jacks >= 1 && (b.notes < a.notes || b.jacks < b.notes) {
        2
    } else {
        0
    }
}

fn jacks_minijacks(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() >= 2 && rows[0].jacks > 0 && rows[1].jacks == 0 {
        2
    } else {
        0
    }
}

fn jacks_longjacks(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 5 || rows[..5].iter().any(|row| row.jacks == 0) {
        return 0;
    }
    rows[0]
        .raw_notes
        .iter()
        .any(|&column| {
            rows[1..5]
                .iter()
                .all(|row| contains_col(&row.raw_notes, column))
        })
        .then_some(5)
        .unwrap_or(0)
}

fn jacks_4k_quadstream(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() >= 4 && rows[0].notes == 4 && rows[2].jacks == 0 && rows[3].jacks == 0 {
        4
    } else {
        0
    }
}

fn jacks_4k_gluts(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 3 || rows[1].jacks != 1 || rows[2].jacks != 1 {
        return 0;
    }
    if rows[0].raw_notes.iter().any(|&column| {
        contains_col(&rows[1].raw_notes, column) && contains_col(&rows[2].raw_notes, column)
    }) {
        0
    } else {
        3
    }
}

fn chordstream_4k_handstream(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() >= 4 && rows[0].notes == 3 && rows[..4].iter().all(|row| row.jacks == 0) {
        4
    } else {
        0
    }
}

fn chordstream_4k_jumpstream(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 4 {
        return 0;
    }
    let a = &rows[0];
    let b = &rows[1];
    let c = &rows[2];
    let d = &rows[3];
    if a.notes == 2
        && a.jacks == 0
        && b.notes == 1
        && b.jacks == 0
        && c.jacks == 0
        && d.jacks == 0
        && c.notes < 3
        && d.notes < 3
    {
        4
    } else {
        0
    }
}

fn chordstream_4k_jumptrill(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() >= 4
        && rows[..4].iter().all(|row| row.notes == 2)
        && rows[1..4].iter().all(|row| row.roll)
    {
        4
    } else {
        0
    }
}

fn chordstream_4k_splittrill(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() >= 3
        && rows[..3].iter().all(|row| row.notes == 2)
        && rows[1..3].iter().all(|row| row.jacks == 0 && !row.roll)
    {
        3
    } else {
        0
    }
}

fn stream_4k_roll(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 3 || rows[..3].iter().any(|row| row.notes != 1) {
        return 0;
    }
    let left = rows[..3]
        .iter()
        .all(|row| row.direction == EzLeoBlackDirection::Left);
    let right = rows[..3]
        .iter()
        .all(|row| row.direction == EzLeoBlackDirection::Right);
    if left || right { 3 } else { 0 }
}

fn stream_4k_trill(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() >= 4
        && rows[1..4].iter().all(|row| row.jacks == 0)
        && same_cols(&rows[0].raw_notes, &rows[2].raw_notes)
        && same_cols(&rows[1].raw_notes, &rows[3].raw_notes)
    {
        4
    } else {
        0
    }
}

fn stream_4k_minitrill(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() >= 4
        && rows[1..3].iter().all(|row| row.jacks == 0)
        && same_cols(&rows[0].raw_notes, &rows[2].raw_notes)
        && !same_cols(&rows[1].raw_notes, &rows[3].raw_notes)
    {
        4
    } else {
        0
    }
}

fn chordstream_7k_double_streams(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() >= 2
        && rows[0].notes == 2
        && rows[1].notes == 2
        && rows[1].jacks == 0
        && !rows[1].roll
    {
        2
    } else {
        0
    }
}

fn chordstream_7k_dense_chordstream(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() >= 2 && rows[0].notes > 1 && rows[1].notes > 1 && rows[1].jacks == 0 {
        2
    } else {
        0
    }
}

fn chordstream_7k_light_chordstream(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() >= 2 && rows[0].notes > 1 && rows[1].notes == 1 && rows[1].jacks == 0 {
        2
    } else {
        0
    }
}

fn chordstream_7k_chord_roll(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 3 {
        return 0;
    }
    let a = &rows[0];
    let b = &rows[1];
    let c = &rows[2];
    let same_direction = (b.direction == EzLeoBlackDirection::Left
        && c.direction == EzLeoBlackDirection::Left)
        || (b.direction == EzLeoBlackDirection::Right && c.direction == EzLeoBlackDirection::Right);
    if a.notes > 1 && b.notes > 1 && c.notes > 1 && b.roll && c.roll && same_direction {
        3
    } else {
        0
    }
}

fn chordstream_7k_brackets(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 3 {
        return 0;
    }
    let a = &rows[0];
    let b = &rows[1];
    let c = &rows[2];
    if a.notes > 2
        && b.notes > 2
        && c.notes > 2
        && !b.roll
        && !c.roll
        && b.jacks == 0
        && c.jacks == 0
        && a.notes + b.notes + c.notes > 9
    {
        3
    } else {
        0
    }
}

fn is_same_hand_adjacent(column_a: usize, column_b: usize, split: usize) -> bool {
    column_a.abs_diff(column_b) == 1 && (column_a < split) == (column_b < split)
}

fn jack_bpm(delta_ms: f64) -> f64 {
    if delta_ms <= 0.0 {
        230.0
    } else {
        (15_000.0 / delta_ms).min(230.0)
    }
}

fn has_ln_context(rows: &[EzLeoBlackPrimitive], window: usize) -> bool {
    rows.iter().take(window.min(rows.len())).any(|row| {
        !row.ln_heads.is_empty() || !row.ln_bodies.is_empty() || !row.ln_tails.is_empty()
    })
}

fn inverse_ready(rows: &[EzLeoBlackPrimitive]) -> bool {
    if rows.len() < 5 || rows[..5].iter().any(|row| !row.normal_notes.is_empty()) {
        return false;
    }
    let max_bodies = rows[..5]
        .iter()
        .map(|row| row.ln_bodies.len())
        .max()
        .unwrap_or(0);
    if max_bodies < 3 {
        return false;
    }
    let gaps: Vec<_> = (0..4)
        .filter(|&index| !rows[index].ln_tails.is_empty() && !rows[index + 1].ln_heads.is_empty())
        .map(|index| rows[index + 1].time - rows[index].time)
        .collect();
    if gaps.len() < 2 {
        return false;
    }
    let max = gaps.iter().copied().fold(gaps[0], f64::max);
    let min = gaps.iter().copied().fold(gaps[0], f64::min);
    max - min <= 5.0
}

fn head_rows(rows: &[EzLeoBlackPrimitive], count: usize) -> Vec<EzLeoBlackPrimitive> {
    let mut output = Vec::with_capacity(count.min(rows.len()));
    let mut previous_heads: Vec<usize> = Vec::new();
    for row in rows.iter().take(count.min(rows.len())) {
        let heads = &row.ln_heads;
        let jacks = heads
            .iter()
            .filter(|column| previous_heads.contains(column))
            .count();
        let (direction, roll) = if previous_heads.is_empty() || heads.is_empty() {
            (EzLeoBlackDirection::None, false)
        } else {
            EzLeoBlackPrimitives::detect_direction(&previous_heads, heads)
        };
        output.push(EzLeoBlackPrimitive {
            index: row.index,
            time: row.time,
            ms_per_beat: row.ms_per_beat,
            beat_length: row.beat_length,
            notes: heads.len(),
            jacks,
            direction,
            roll,
            keys: row.keys,
            left_hand_keys: row.left_hand_keys,
            ln_heads: row.ln_heads.clone(),
            ln_bodies: row.ln_bodies.clone(),
            ln_tails: row.ln_tails.clone(),
            normal_notes: Vec::new(),
            raw_notes: heads.clone(),
        });
        if !heads.is_empty() {
            previous_heads.clone_from(heads);
        }
    }
    output
}

fn coordination_column_lock(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 3 {
        return 0;
    }
    let first = &rows[0];
    let Some(&ln_column) = first.ln_heads.first() else {
        return 0;
    };
    let split = first.left_hand_keys;
    let mut adjacent_columns = Vec::with_capacity(2);
    if ln_column > 0 {
        let lower = ln_column - 1;
        if is_same_hand_adjacent(ln_column, lower, split) {
            adjacent_columns.push(lower);
        }
    }
    let upper = ln_column + 1;
    if upper < first.keys && is_same_hand_adjacent(ln_column, upper, split) {
        adjacent_columns.push(upper);
    }
    let scan = 8.min(rows.len());
    for adjacent in adjacent_columns {
        let hits: Vec<_> = rows[..scan]
            .iter()
            .filter(|row| {
                contains_col(&row.ln_bodies, ln_column) && contains_col(&row.normal_notes, adjacent)
            })
            .map(|row| row.time)
            .collect();
        if hits.len() < 3 {
            continue;
        }
        let max_bpm = hits
            .windows(2)
            .map(|pair| jack_bpm(pair[1] - pair[0]))
            .fold(0.0, f64::max);
        if max_bpm >= 90.0 {
            return 3;
        }
    }
    0
}

fn coordination_shield(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 2 {
        return 0;
    }
    let a = &rows[0];
    let b = &rows[1];
    let dt = b.time - a.time;
    let beat_limit = b.beat_length * 0.25;
    if dt < 0.0 || dt > beat_limit {
        return 0;
    }
    let normal_to_head = a
        .normal_notes
        .iter()
        .any(|column| contains_col(&b.ln_heads, *column));
    let tail_to_normal = a
        .ln_tails
        .iter()
        .any(|column| contains_col(&b.normal_notes, *column));
    if normal_to_head || tail_to_normal {
        2
    } else {
        0
    }
}

fn coordination_release(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 4
        || coordination_shield(rows) != 0
        || inverse_ready(rows)
        || wildcard_jack(rows) != 0
    {
        return 0;
    }
    let picked: Vec<_> = rows
        .iter()
        .take(4.min(rows.len()))
        .filter(|row| row.ln_tails.len() == 1)
        .collect();
    if picked.len() < 4 {
        return 0;
    }
    let use_rows = 5.min(picked.len());
    if use_rows.saturating_sub(1) < 2 {
        return 0;
    }
    let a_col = picked[1].ln_tails[0];
    let b_col = picked[2].ln_tails[0];
    let dt = picked[2].time - picked[1].time;
    if a_col != b_col && dt > 0.0 {
        if use_rows >= 5 { 5 } else { 4 }
    } else {
        0
    }
}

fn density_4k_jumpstream(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 4 || !rows.first().is_some_and(|row| !row.ln_heads.is_empty()) {
        return 0;
    }
    if chordstream_4k_jumpstream(&head_rows(rows, 4)) != 0 {
        4
    } else {
        0
    }
}

fn density_4k_handstream(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 4 || !rows.first().is_some_and(|row| !row.ln_heads.is_empty()) {
        return 0;
    }
    if chordstream_4k_handstream(&head_rows(rows, 4)) != 0 {
        4
    } else {
        0
    }
}

fn density_inverse(rows: &[EzLeoBlackPrimitive]) -> usize {
    if inverse_ready(rows) { 5 } else { 0 }
}

fn density_7k_double_streams(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 2 || !rows.first().is_some_and(|row| !row.ln_heads.is_empty()) {
        return 0;
    }
    if chordstream_7k_double_streams(&head_rows(rows, 2)) != 0 {
        2
    } else {
        0
    }
}

fn density_7k_dense_chordstream(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 2 || !rows.first().is_some_and(|row| !row.ln_heads.is_empty()) {
        return 0;
    }
    if chordstream_7k_dense_chordstream(&head_rows(rows, 2)) != 0 {
        2
    } else {
        0
    }
}

fn density_7k_light_chordstream(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 2 || !rows.first().is_some_and(|row| !row.ln_heads.is_empty()) {
        return 0;
    }
    if chordstream_7k_light_chordstream(&head_rows(rows, 2)) != 0 {
        2
    } else {
        0
    }
}

fn wildcard_jack(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 2 || !has_ln_context(rows, 6) {
        return 0;
    }
    let note_rows: Vec<_> = rows
        .iter()
        .take(6.max(4).min(rows.len()))
        .filter(|row| row.notes > 0)
        .cloned()
        .collect();
    if note_rows.len() < 2 {
        return 0;
    }
    if jacks_chordjacks(&note_rows) != 0 || jacks_minijacks(&note_rows) != 0 {
        return 4;
    }
    let check_count = 4.min(note_rows.len());
    let jack_rows = note_rows[..check_count]
        .iter()
        .filter(|row| row.jacks > 0)
        .count();
    let has_chord = note_rows[..check_count].iter().any(|row| row.notes >= 2);
    let fastest_ms_per_beat = note_rows[..check_count]
        .iter()
        .map(|row| row.ms_per_beat)
        .fold(f64::INFINITY, f64::min);
    if jack_rows >= 2 && (has_chord || fastest_ms_per_beat <= 185.0) {
        3
    } else {
        0
    }
}

fn wildcard_speed(rows: &[EzLeoBlackPrimitive]) -> usize {
    if rows.len() < 2 || !has_ln_context(rows, 4) {
        return 0;
    }
    let head_rows = head_rows(rows, 4.min(rows.len()));
    if rows[0].keys == 4 {
        if head_rows.len() >= 3 && stream_4k_roll(&head_rows[..3]) != 0 {
            return 3;
        }
        if head_rows.len() >= 2 {
            let same_direction = matches!(
                head_rows[0].direction,
                EzLeoBlackDirection::Left | EzLeoBlackDirection::Right
            ) && head_rows[0].direction == head_rows[1].direction;
            if same_direction || head_rows[0].ms_per_beat <= 180.0 {
                return 3;
            }
        }
    } else {
        if head_rows.len() >= 3 && chordstream_7k_chord_roll(&head_rows[..3]) != 0 {
            return 3;
        }
        if head_rows.len() >= 2 {
            let first = &head_rows[0];
            let second = &head_rows[1];
            let same_direction = first.notes >= 2
                && second.notes >= 2
                && first.direction == second.direction
                && matches!(
                    first.direction,
                    EzLeoBlackDirection::Left | EzLeoBlackDirection::Right
                );
            if same_direction || first.ms_per_beat <= 170.0 {
                return 3;
            }
        }
    }
    0
}

#[derive(Clone, Copy)]
struct ClusterBuilder {
    sum_ms: f64,
    timed_count: usize,
    original_ms_per_beat: f64,
    bpm: f64,
}

impl ClusterBuilder {
    fn new(value: f64) -> Self {
        let timed = value >= 40.0;
        Self {
            sum_ms: if timed { value } else { 0.0 },
            timed_count: usize::from(timed),
            original_ms_per_beat: value,
            bpm: 0.0,
        }
    }

    fn add(&mut self, value: f64) {
        if value >= 40.0 {
            self.sum_ms += value;
            self.timed_count += 1;
        }
    }

    fn calculate(&mut self) {
        self.bpm = if self.timed_count == 0 {
            0.0
        } else {
            (60_000.0 / (self.sum_ms / self.timed_count as f64)).round_ties_even()
        };
    }
}

fn calculate_clustered_patterns(
    patterns: &[EzLeoBlackFoundPattern],
    mode_tag: &str,
) -> Vec<InternalCluster> {
    let mut non_mixed: Vec<ClusterBuilder> = Vec::new();
    let mut mixed: Vec<(&'static str, ClusterBuilder)> = Vec::new();
    let mut assignment: Vec<(bool, usize)> = Vec::with_capacity(patterns.len());

    for pattern in patterns {
        if pattern.mixed {
            if let Some(index) = mixed.iter().position(|(name, _)| *name == pattern.pattern) {
                mixed[index].1.add(pattern.ms_per_beat);
                assignment.push((true, index));
            } else {
                mixed.push((pattern.pattern, ClusterBuilder::new(pattern.ms_per_beat)));
                assignment.push((true, mixed.len() - 1));
            }
        } else if let Some(index) = non_mixed
            .iter()
            .position(|cluster| (cluster.original_ms_per_beat - pattern.ms_per_beat).abs() < 5.0)
        {
            non_mixed[index].add(pattern.ms_per_beat);
            assignment.push((false, index));
        } else {
            non_mixed.push(ClusterBuilder::new(pattern.ms_per_beat));
            assignment.push((false, non_mixed.len() - 1));
        }
    }

    for cluster in &mut non_mixed {
        cluster.calculate();
    }
    for (_, cluster) in &mut mixed {
        cluster.calculate();
    }
    let mut groups: Vec<(&'static str, bool, f64, Vec<usize>)> = Vec::new();
    for (index, pattern) in patterns.iter().enumerate() {
        let (is_mixed, cluster_index) = assignment[index];
        let bpm = if is_mixed {
            mixed[cluster_index].1.bpm
        } else {
            non_mixed[cluster_index].bpm
        };
        let group_index = groups
            .iter()
            .position(|(name, is_mixed, group_bpm, _)| {
                *name == pattern.pattern && *is_mixed == pattern.mixed && *group_bpm == bpm
            })
            .unwrap_or_else(|| {
                groups.push((pattern.pattern, pattern.mixed, bpm, Vec::new()));
                groups.len() - 1
            });
        groups[group_index].3.push(index);
    }

    let has_density_or_wildcard = groups
        .iter()
        .any(|(pattern, _, _, _)| matches!(*pattern, "Density" | "Wildcard"));
    let mut output = Vec::with_capacity(groups.len());

    for (pattern, mixed, bpm, indices) in groups {
        let mut intervals: Vec<_> = indices
            .iter()
            .map(|&index| (patterns[index].start, patterns[index].end))
            .collect();
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut counter: Vec<(&'static str, usize)> = Vec::new();
        for &index in &indices {
            if let Some(specific) = patterns[index].specific_type {
                if let Some((_, count)) = counter.iter_mut().find(|(name, _)| *name == specific) {
                    *count += 1;
                } else {
                    counter.push((specific, 1));
                }
            }
        }
        let mut specific_types: Vec<_> = counter
            .into_iter()
            .map(|(name, count)| EzLeoBlackSpecificTypeRatio {
                name,
                ratio: count as f64 / indices.len() as f64,
            })
            .collect();
        specific_types.sort_by(|a, b| b.ratio.total_cmp(&a.ratio));
        let dominant_specific = specific_types.first().map(|specific| specific.name);
        let mut rating_multiplier = resolve_rating_multiplier(pattern, dominant_specific, mode_tag);
        if has_density_or_wildcard
            && specific_types
                .iter()
                .any(|specific| specific.name == "Release" && specific.ratio > 0.0)
        {
            rating_multiplier *= 0.8;
        }
        output.push(InternalCluster {
            pattern,
            specific_types,
            rating_multiplier,
            bpm,
            mixed,
            amount: pattern_amount(&intervals),
        });
    }
    output
}

fn pattern_amount(sorted_intervals: &[(f64, f64)]) -> f64 {
    let Some(&(first_start, first_end)) = sorted_intervals.first() else {
        return 0.0;
    };
    let mut total_time = 0.0;
    let mut current_start = first_start;
    let mut current_end = first_end;
    for &(start, end) in sorted_intervals {
        if current_end < end {
            total_time += current_end - current_start;
            current_start = start;
            current_end = end;
        } else {
            current_end = current_end.max(end);
        }
    }
    total_time + current_end - current_start
}

fn resolve_rating_multiplier(pattern: &str, specific_type: Option<&str>, mode_tag: &str) -> f64 {
    let default = match pattern {
        "Stream" => 1.0 / 3.0,
        "Chordstream" => 0.65,
        "Jacks" => 0.9,
        "Coordination" => 0.75,
        "Density" => 0.9,
        "Wildcard" => 1.0,
        _ => 1.0,
    };
    let core_ln = matches!(pattern, "Coordination" | "Density" | "Wildcard");
    let core_rc = matches!(pattern, "Stream" | "Chordstream" | "Jacks");
    let subtype = specific_type
        .map(|name| subtype_rating(name, mode_tag))
        .unwrap_or(default);
    let value = if mode_tag == "RC" && core_ln {
        specific_type
            .map(|name| subtype_rating(name, "Mix"))
            .unwrap_or(default)
            * 0.0
    } else if mode_tag == "LN" && core_rc {
        subtype * 0.3
    } else {
        subtype
    };
    value
}

fn subtype_rating(name: &str, mode_tag: &str) -> f64 {
    let rc = match name {
        "Rolls" | "Trills" | "Minitrills" => 1.0 / 3.0,
        "Handstream" | "Split Trill" | "Jumptrill" | "Jumpstream" | "Brackets"
        | "Double Stream" | "Dense Chordstream" | "Light Chordstream" | "Chord Rolls" => 0.65,
        "Longjacks" | "Quadstream" | "Gluts" | "Chordjacks" | "Minijacks" => 0.9,
        "Column Lock" => 1.5,
        "Release" => 0.73,
        "Shield" => 0.8,
        "JS Density" | "HS Density" | "DS Density" | "LCS Density" | "DCS Density" => 1.0,
        "Inverse" => 1.5,
        "Jacky WC" => 0.55,
        "Speedy WC" => 0.8,
        _ => 1.0,
    };
    match mode_tag {
        "LN" => match name {
            "Release" => 1.0,
            "JS Density" | "HS Density" | "DS Density" | "LCS Density" | "DCS Density" => 0.9,
            _ => rc,
        },
        "HB" => match name {
            "Release" => 0.3,
            "JS Density" | "HS Density" | "DS Density" | "LCS Density" | "DCS Density" => 0.9,
            "Inverse" => 0.0,
            "Jacky WC" => 0.65,
            "Speedy WC" => 0.45,
            _ => rc,
        },
        "Mix" => match name {
            "Release" => 0.3,
            "JS Density" | "HS Density" | "DS Density" | "LCS Density" | "DCS Density" => 0.9,
            "Inverse" => 0.0,
            "Jacky WC" | "Speedy WC" => 0.45,
            _ => rc,
        },
        _ => rc,
    }
}

fn categorise(clusters: &[InternalCluster]) -> String {
    let Some(first) = clusters.first() else {
        return "Uncategorised".to_owned();
    };
    let important: Vec<_> = clusters
        .iter()
        .take_while(|cluster| cluster.importance() / first.importance() > 0.5)
        .collect();
    let Some(cluster) = important.first() else {
        return "Uncategorised".to_owned();
    };
    let tech = cluster.mixed;
    let name = if cluster
        .specific_types
        .first()
        .is_some_and(|specific| specific.ratio > 0.05)
    {
        cluster.specific_types[0].name
    } else if cluster.specific_types.len() >= 2
        && cluster.specific_types[0].name == "Jumpstream"
        && cluster.specific_types[1].name == "Handstream"
        && cluster.specific_types[1].ratio / cluster.specific_types[0].ratio > 0.4
    {
        "Jumpstream/Handstream"
    } else {
        cluster.pattern
    };
    if tech {
        format!("{name} Tech")
    } else {
        name.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mania::ManiaNote;

    #[test]
    fn chart_rows_fill_hold_bodies_without_overwriting_note_cells() {
        let notes = [
            ManiaNote {
                start_time_ms: 0.0,
                end_time_ms: 1000.0,
                column: 0,
            },
            ManiaNote {
                start_time_ms: 250.0,
                end_time_ms: 250.0,
                column: 1,
            },
            ManiaNote {
                start_time_ms: 500.0,
                end_time_ms: 500.0,
                column: 0,
            },
            ManiaNote {
                start_time_ms: 500.0,
                end_time_ms: 500.0,
                column: 2,
            },
            ManiaNote {
                start_time_ms: 1000.0,
                end_time_ms: 1000.0,
                column: 0,
            },
        ];
        let input = EzManiaChartInput {
            key_count: 4,
            bpm: 120.0,
            total_length_ms: 2000.0,
            notes: &notes,
            timing_points: &[(0.0, 500.0)],
        };

        let chart = EzLeoBlackChartBuilder::try_build(input).unwrap();
        assert_eq!(chart.notes.len(), 4);
        assert_eq!(chart.notes[0].data[0], EzLeoBlackNoteType::HoldHead);
        assert_eq!(chart.notes[1].data[0], EzLeoBlackNoteType::HoldBody);
        assert_eq!(chart.notes[2].data[0], EzLeoBlackNoteType::Normal);
        assert_eq!(chart.notes[2].data[2], EzLeoBlackNoteType::Normal);
        assert_eq!(chart.notes[3].data[0], EzLeoBlackNoteType::HoldTail);
    }

    #[test]
    fn overlapping_holds_keep_body_active_until_the_last_end() {
        let notes = [
            ManiaNote {
                start_time_ms: 0.0,
                end_time_ms: 900.0,
                column: 0,
            },
            ManiaNote {
                start_time_ms: 200.0,
                end_time_ms: 700.0,
                column: 0,
            },
            ManiaNote {
                start_time_ms: 400.0,
                end_time_ms: 400.0,
                column: 1,
            },
            ManiaNote {
                start_time_ms: 900.0,
                end_time_ms: 900.0,
                column: 0,
            },
        ];
        let chart = EzLeoBlackChartBuilder::try_build(EzManiaChartInput {
            key_count: 4,
            bpm: 150.0,
            total_length_ms: 1000.0,
            notes: &notes,
            timing_points: &[],
        })
        .unwrap();

        let row = chart.notes.iter().find(|row| row.time == 400.0).unwrap();
        assert_eq!(row.data[0], EzLeoBlackNoteType::HoldBody);
        let at_end = chart.notes.iter().find(|row| row.time == 900.0).unwrap();
        assert_eq!(at_end.data[0], EzLeoBlackNoteType::HoldTail);
        assert_eq!(chart.bpm[0].data.ms_per_beat, 400.0);
    }

    #[test]
    fn primitives_match_hub_row_timing_direction_and_hold_ratio() {
        let notes = [
            ManiaNote {
                start_time_ms: 0.0,
                end_time_ms: 0.0,
                column: 0,
            },
            ManiaNote {
                start_time_ms: 125.0,
                end_time_ms: 125.0,
                column: 1,
            },
            ManiaNote {
                start_time_ms: 250.0,
                end_time_ms: 250.0,
                column: 2,
            },
            ManiaNote {
                start_time_ms: 375.0,
                end_time_ms: 375.0,
                column: 3,
            },
            ManiaNote {
                start_time_ms: 500.0,
                end_time_ms: 1000.0,
                column: 3,
            },
        ];
        let chart = EzLeoBlackChartBuilder::try_build(EzManiaChartInput {
            key_count: 4,
            bpm: 120.0,
            total_length_ms: 1200.0,
            notes: &notes,
            timing_points: &[(0.0, 400.0), (400.0, 300.0)],
        })
        .unwrap();
        let primitives = EzLeoBlackPrimitives::calculate(&chart);

        assert_eq!(primitives.len(), 5);
        assert_eq!(primitives[0].index, 1);
        assert_eq!(primitives[0].time, 125.0);
        assert_eq!(primitives[0].ms_per_beat, 500.0);
        assert_eq!(primitives[0].beat_length, 400.0);
        assert_eq!(primitives[0].direction, EzLeoBlackDirection::Right);
        assert_eq!(primitives[2].direction, EzLeoBlackDirection::Right);
        assert_eq!(primitives[3].jacks, 1);
        assert_eq!(primitives[3].beat_length, 300.0);
        assert_eq!(EzLeoBlackPrimitives::ln_percent(&chart), 0.2);
    }

    #[test]
    fn same_time_notes_form_one_row_and_invalid_notes_are_ignored() {
        let notes = [
            ManiaNote {
                start_time_ms: 1.0,
                end_time_ms: 1.0,
                column: 1,
            },
            ManiaNote {
                start_time_ms: 1.0,
                end_time_ms: 1.0,
                column: 3,
            },
            ManiaNote {
                start_time_ms: f64::NAN,
                end_time_ms: f64::NAN,
                column: 0,
            },
            ManiaNote {
                start_time_ms: 5.0,
                end_time_ms: 6.0,
                column: 4,
            },
        ];
        let chart = EzLeoBlackChartBuilder::try_build(EzManiaChartInput {
            key_count: 4,
            bpm: f64::NAN,
            total_length_ms: 10.0,
            notes: &notes,
            timing_points: &[(0.0, f64::INFINITY)],
        })
        .unwrap();

        assert_eq!(chart.notes.len(), 1);
        assert_eq!(chart.notes[0].time, 1.0);
        assert_eq!(chart.notes[0].data[1], EzLeoBlackNoteType::Normal);
        assert_eq!(chart.notes[0].data[3], EzLeoBlackNoteType::Normal);
        assert_eq!(chart.bpm[0].data.ms_per_beat, 500.0);
    }

    #[test]
    fn chart_pipeline_extracts_jack_cluster_and_weighted_share() {
        let mut notes = Vec::new();
        for row in 0..24 {
            let time = row as f64 * 100.0;
            notes.push(ManiaNote {
                start_time_ms: time,
                end_time_ms: time,
                column: 0,
            });
            notes.push(ManiaNote {
                start_time_ms: time,
                end_time_ms: time,
                column: 1,
            });
        }
        let analysis = EzLeoBlackPatternAnalyzer::try_analyze(EzManiaChartInput {
            key_count: 4,
            bpm: 150.0,
            total_length_ms: 2400.0,
            notes: &notes,
            timing_points: &[(0.0, 400.0)],
        })
        .unwrap();

        assert!(!analysis.clusters.is_empty());
        assert_eq!(analysis.category, "Longjacks");
        assert!(
            analysis
                .clusters
                .iter()
                .any(|cluster| cluster.pattern == "Jacks" && cluster.bpm == 150.0)
        );
        assert_eq!(
            analysis.top_five_clusters.len(),
            analysis.clusters.len().min(5)
        );
        assert_eq!(
            EzLeoBlackClusterMetrics::jack_share(&analysis.clusters),
            Some(1.0)
        );
        assert_eq!(
            EzLeoBlackClusterMetrics::stream_share(&analysis.clusters),
            Some(0.0)
        );
        assert!(!EzLeoBlackClusterMetrics::tech_category(Some(
            &analysis.category
        )));
    }

    #[test]
    fn cluster_share_ignores_non_positive_and_non_finite_importance() {
        let clusters = [
            EzLeoBlackCluster {
                pattern: "Jacks",
                bpm: 120.0,
                amount: 5.0,
                importance: 80.0,
                label: "Jacks".to_owned(),
            },
            EzLeoBlackCluster {
                pattern: "Chordstream",
                bpm: 200.0,
                amount: 1.0,
                importance: 20.0,
                label: "Chordstream".to_owned(),
            },
            EzLeoBlackCluster {
                pattern: "Stream",
                bpm: 0.0,
                amount: 0.0,
                importance: f64::NAN,
                label: "Stream".to_owned(),
            },
        ];
        assert_eq!(EzLeoBlackClusterMetrics::jack_share(&clusters), Some(0.8));
        assert_eq!(EzLeoBlackClusterMetrics::stream_share(&clusters), Some(0.2));
        assert_eq!(EzLeoBlackClusterMetrics::cluster_share(&[], |_| true), None);
        assert!(EzLeoBlackClusterMetrics::tech_category(Some(
            "Jumpstream Tech"
        )));
        assert!(EzLeoBlackClusterMetrics::cluster_trill(Some("Trill")));
        assert!(EzLeoBlackClusterMetrics::handstream_cluster(Some(
            "Handstream"
        )));
        assert!(!EzLeoBlackClusterMetrics::tech_category(Some("Stream")));
    }
}
