//! Mania chart structure measurements used by Ez2Lazer CSI.

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use crate::mania::ManiaNote;

#[path = "csi_feature_math.rs"]
mod math;
use math::*;

/// Inputs to the CSI chart feature pass in source-map milliseconds.
#[derive(Clone, Copy, Debug)]
pub struct EzManiaChartInput<'a> {
    pub key_count: usize,
    pub bpm: f64,
    /// End of the playable chart on the beatmap clock, including any hold tails.
    pub total_length_ms: f64,
    pub notes: &'a [ManiaNote],
    /// Uninherited (start time, beat length) points.
    pub timing_points: &'a [(f64, f64)],
}

/// A same-time row. The range addresses the row's notes in
/// [`EzChartFeatureExtraction::ordered_notes`].
#[derive(Clone, Debug, PartialEq)]
pub struct EzChartRow {
    pub time_ms: f64,
    pub note_range: Range<usize>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EzChartFeatureMetrics {
    pub key_count: usize,
    pub note_count: usize,
    pub duration_ms: f64,
    pub hold_ratio: f64,
    pub chord_ratio: f64,
    pub two_note_chord_ratio: f64,
    pub peak_nps_1s: f64,
    pub peak_nps_5s: f64,
    pub nps_5s_p50: f64,
    pub nps_5s_p90: f64,
    pub nps_5s_p95: f64,
    pub sustained_nps_10s: f64,
    pub sustained_nps_30s: f64,
    pub sustained_nps_60s: f64,
    pub active_nps: f64,
    pub long_gap_ratio: f64,
    pub long_gap_count: usize,
    pub jack_pressure: f64,
    pub stream_pressure: f64,
    pub jumpstream_pressure: f64,
    pub chordjack_pressure: f64,
    pub chord_column_overlap_ratio: f64,
    pub adjacent_column_rehit_share: f64,
    pub two_back_column_rehit_share: f64,
    pub two_back_column_rehit_excess: f64,
    pub tech_pressure: f64,
    pub row_burst_pressure: f64,
    pub fast_row_ratio: f64,
    pub row_interval_entropy: f64,
    pub off_grid_row_share: f64,
    pub pattern_variety: f64,
    pub row_pattern_entropy: f64,
    pub row_pattern_variety: f64,
    pub repeated_row_pattern_ratio: f64,
    pub alternating_row_pattern_ratio: f64,
    pub row_pattern_change_rate: f64,
    pub row_motif_repeat_ratio: f64,
    pub rhythm_motif_repeat_ratio: f64,
    pub adjacent_motif_repeat_ratio: f64,
    pub strain_spikiness: f64,
    pub sustained_pressure_ratio: f64,
    pub anchor_pressure: f64,
    pub ln_release_pressure: f64,
    pub ln_density: f64,
    pub ln_overlap_pressure: f64,
    pub ln_chord_pressure: f64,
    pub ln_hold_duration_avg: f64,
    pub ln_hold_duration_p90: f64,
    pub chord_size_change_rate: f64,
    pub direction_change_rate: f64,
    pub stamina_pressure: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EzChartFeatureExtraction {
    /// Valid notes in source order, with times divided by rate.
    pub notes: Vec<ManiaNote>,
    pub note_times: Vec<f64>,
    pub duration_ms: f64,
    pub ordered_notes: Vec<ManiaNote>,
    pub ordered_rows: Vec<EzChartRow>,
    pub metrics: EzChartFeatureMetrics,
    pub warnings: Vec<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EzChartFeatureError {
    InvalidKeyCount,
    InvalidRate,
    InvalidTime,
}

impl std::fmt::Display for EzChartFeatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidKeyCount => "CSI key count must be in 1..=18",
            Self::InvalidRate => "CSI rate must be finite and positive",
            Self::InvalidTime => {
                "CSI note times and chart end must be finite at the requested rate"
            }
        })
    }
}

impl std::error::Error for EzChartFeatureError {}

pub struct EzChartFeatureExtractor;

impl EzChartFeatureExtractor {
    /// Extracts the CSI chart features. Notes outside the chart's column count are skipped.
    pub fn extract(
        map: EzManiaChartInput<'_>,
        rate: f64,
    ) -> Result<EzChartFeatureExtraction, EzChartFeatureError> {
        if !(1..=18).contains(&map.key_count) {
            return Err(EzChartFeatureError::InvalidKeyCount);
        }
        if !rate.is_finite() || rate <= 0.0 {
            return Err(EzChartFeatureError::InvalidRate);
        }
        let source_duration = map.total_length_ms / rate;
        if !source_duration.is_finite() {
            return Err(EzChartFeatureError::InvalidTime);
        }

        let mut notes = Vec::with_capacity(map.notes.len());
        let mut hold_events = Vec::new();
        let mut hold_durations = Vec::new();
        let mut releases = Vec::new();
        let mut total_hold_ms = 0.0;

        for note in map.notes.iter().filter(|note| note.column < map.key_count) {
            let start = note.start_time_ms / rate;
            let end = note.end_time_ms / rate;
            let elapsed = end - start;
            if !start.is_finite() || !end.is_finite() || !elapsed.is_finite() {
                return Err(EzChartFeatureError::InvalidTime);
            }
            if elapsed > 0.0 {
                hold_durations.push(elapsed);
                total_hold_ms += elapsed;
                hold_events.push((start, 1_i32));
                hold_events.push((end, -1_i32));
                releases.push(end);
            }
            notes.push(ManiaNote {
                start_time_ms: start,
                end_time_ms: end,
                column: note.column,
            });
        }

        let mut ordered_notes = notes.clone();
        ordered_notes.sort_by(|left, right| {
            left.start_time_ms
                .partial_cmp(&right.start_time_ms)
                .unwrap()
        });
        releases.sort_by(f64::total_cmp);
        let note_times: Vec<_> = ordered_notes
            .iter()
            .map(|note| note.start_time_ms)
            .collect();
        let duration_ms = source_duration.max(note_times.last().copied().unwrap_or(0.0));
        let mut warnings = Vec::new();
        if notes.len() < 50 || duration_ms <= 0.0 {
            warnings.push("This map has very little note data, so the estimate is low confidence.");
        }

        let mut ordered_rows = Vec::new();
        let mut rows = RowMeasurements::new(map.key_count);
        let mut row_start = 0;
        while row_start < ordered_notes.len() {
            let time = ordered_notes[row_start].start_time_ms;
            let mut row_end = row_start + 1;
            while row_end < ordered_notes.len() && ordered_notes[row_end].start_time_ms == time {
                row_end += 1;
            }
            let row_notes = &ordered_notes[row_start..row_end];
            let mut columns: Vec<_> = row_notes.iter().map(|note| note.column).collect();
            columns.sort_unstable();
            let has_hold = row_notes.iter().any(is_hold);
            rows.measure(time, &columns, has_hold);
            ordered_rows.push(EzChartRow {
                time_ms: time,
                note_range: row_start..row_end,
            });
            row_start = row_end;
        }

        let metrics = metrics(
            map,
            rate,
            duration_ms,
            &rows,
            &note_times,
            &releases,
            &hold_durations,
            hold_events,
            total_hold_ms,
        );
        Ok(EzChartFeatureExtraction {
            notes,
            note_times,
            duration_ms,
            ordered_notes,
            ordered_rows,
            metrics,
            warnings,
        })
    }
}

fn is_hold(note: &ManiaNote) -> bool {
    note.end_time_ms > note.start_time_ms
}

struct RowMeasurements {
    last_by_column: Vec<f64>,
    column_counts: Vec<usize>,
    jack_values: Vec<f64>,
    stream_values: Vec<f64>,
    jumpstream_values: Vec<f64>,
    row_densities: Vec<f64>,
    row_density_weights: Vec<f64>,
    row_intervals: Vec<f64>,
    row_rates: Vec<f64>,
    column_intervals: Vec<f64>,
    column_rates: Vec<f64>,
    chord_rows: usize,
    two_note_chord_rows: usize,
    hold_rows: usize,
    ln_chord_rows: usize,
    long_gap_count: usize,
    long_gap_ms: f64,
    fast_row_count: usize,
    direction_changes: usize,
    previous_column: Option<usize>,
    previous_direction: i32,
    previous_chord_size: usize,
    chord_size_changes: usize,
    repeated_row_patterns: usize,
    alternating_row_patterns: usize,
    chord_pair_count: usize,
    chord_pair_overlap_count: usize,
    adjacent_column_rehit_notes: usize,
    two_back_column_rehit_notes: usize,
    row_pattern_change_sum: f64,
    row_masks: Vec<u32>,
    row_times: Vec<f64>,
    row_signatures: Vec<u32>,
}

impl RowMeasurements {
    fn new(key_count: usize) -> Self {
        Self {
            last_by_column: vec![f64::NEG_INFINITY; key_count],
            column_counts: vec![0; key_count],
            jack_values: Vec::new(),
            stream_values: Vec::new(),
            jumpstream_values: Vec::new(),
            row_densities: Vec::new(),
            row_density_weights: Vec::new(),
            row_intervals: Vec::new(),
            row_rates: Vec::new(),
            column_intervals: Vec::new(),
            column_rates: Vec::new(),
            chord_rows: 0,
            two_note_chord_rows: 0,
            hold_rows: 0,
            ln_chord_rows: 0,
            long_gap_count: 0,
            long_gap_ms: 0.0,
            fast_row_count: 0,
            direction_changes: 0,
            previous_column: None,
            previous_direction: 0,
            previous_chord_size: 0,
            chord_size_changes: 0,
            repeated_row_patterns: 0,
            alternating_row_patterns: 0,
            chord_pair_count: 0,
            chord_pair_overlap_count: 0,
            adjacent_column_rehit_notes: 0,
            two_back_column_rehit_notes: 0,
            row_pattern_change_sum: 0.0,
            row_masks: Vec::new(),
            row_times: Vec::new(),
            row_signatures: Vec::new(),
        }
    }

    fn measure(&mut self, time: f64, columns: &[usize], has_hold: bool) {
        let chord_size = columns.len();
        let mask = columns
            .iter()
            .fold(0_u32, |mask, &column| mask | 1 << column);
        let row_index = self.row_times.len();
        let key_count = self.column_counts.len();
        self.chord_rows += usize::from(chord_size >= 2);
        self.two_note_chord_rows += usize::from(chord_size == 2);
        self.hold_rows += usize::from(has_hold);
        self.ln_chord_rows += usize::from(has_hold && chord_size >= 2);

        let mut interval_bucket = 0_i32;
        if let Some(previous_index) = row_index.checked_sub(1) {
            let previous_mask = self.row_masks[previous_index];
            let gap = time - self.row_times[previous_index];
            let overlap = (previous_mask & mask).count_ones() as usize;
            if gap < 1000.0 && self.previous_chord_size >= 2 && chord_size >= 2 {
                self.chord_pair_count += 1;
                self.chord_pair_overlap_count += usize::from(overlap > 0);
            }
            if gap <= 500.0 {
                self.adjacent_column_rehit_notes += overlap;
            }
            self.repeated_row_patterns += usize::from(mask == previous_mask);
            self.row_pattern_change_sum +=
                (mask ^ previous_mask).count_ones() as f64 / key_count as f64;
            self.chord_size_changes += usize::from(self.previous_chord_size != chord_size);
            if gap >= 2500.0 {
                self.long_gap_count += 1;
                self.long_gap_ms += gap;
            }
            if gap > 0.0 && gap < 1200.0 {
                let density = chord_size as f64 * 1000.0 / gap;
                self.row_intervals.push(gap);
                self.row_rates.push(1000.0 / gap);
                self.row_densities.push(density);
                self.row_density_weights.push(gap.max(1.0));
                if chord_size == 2 {
                    self.jumpstream_values.push(density.min(60.0));
                }
                self.fast_row_count += usize::from(gap <= 80.0);
                interval_bucket = (gap / 5.0).round_ties_even() as i32;
            } else {
                interval_bucket = -1;
            }
        }
        if let Some(two_back_index) = row_index.checked_sub(2) {
            let two_back_mask = self.row_masks[two_back_index];
            if time - self.row_times[two_back_index] <= 500.0 {
                self.two_back_column_rehit_notes += (mask & two_back_mask).count_ones() as usize;
            }
            self.alternating_row_patterns += usize::from(mask == two_back_mask);
        }

        self.row_signatures
            .push(mask * 256 + (interval_bucket + 1) as u32);
        self.row_masks.push(mask);
        self.row_times.push(time);
        self.previous_chord_size = chord_size;

        for &column in columns {
            self.column_counts[column] += 1;
            let same_gap = time - self.last_by_column[column];
            if same_gap > 0.0 && same_gap < 1000.0 {
                self.jack_values.push((15000.0 / same_gap).min(230.0));
            }
            if same_gap > 0.0 && same_gap < 1600.0 {
                self.column_intervals.push(same_gap);
                self.column_rates.push(1000.0 / same_gap);
            }

            for neighbor in [
                column.checked_sub(1),
                (column + 1 < key_count).then_some(column + 1),
            ]
            .into_iter()
            .flatten()
            {
                let gap = time - self.last_by_column[neighbor];
                if gap > 0.0 && gap < 260.0 {
                    self.stream_values.push((260.0 - gap) / 35.0);
                }
            }
            if let Some(previous) = self.previous_column {
                let direction = (column as i32 - previous as i32).signum();
                if direction != 0 {
                    if self.previous_direction != 0 && direction != self.previous_direction {
                        self.direction_changes += 1;
                    }
                    self.previous_direction = direction;
                }
            }
            self.previous_column = Some(column);
            self.last_by_column[column] = time;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn metrics(
    map: EzManiaChartInput<'_>,
    rate: f64,
    duration: f64,
    rows: &RowMeasurements,
    notes: &[f64],
    releases: &[f64],
    hold_durations: &[f64],
    mut hold_events: Vec<(f64, i32)>,
    total_hold_ms: f64,
) -> EzChartFeatureMetrics {
    let mut tail_intervals = Vec::new();
    let mut tail_rates = Vec::new();
    for pair in releases.windows(2) {
        let gap = pair[1] - pair[0];
        if gap > 0.0 && gap < 1600.0 {
            tail_intervals.push(gap);
            tail_rates.push(1000.0 / gap);
        }
    }
    let row_count = rows.row_masks.len();
    let note_count = notes.len();
    let chord_ratio = ratio(rows.chord_rows, row_count);
    let peak_nps_1s = count_in_window(notes, 1000.0) as f64;
    let peak_nps_5s = count_in_window(notes, 5000.0) as f64 / 5.0;
    let [nps_5s_p50, nps_5s_p90, nps_5s_p95] = sampled_window_quantiles(notes, duration);
    let sustained_nps_10s = count_in_window(notes, 10_000.0) as f64 / 10.0;
    let note_span = if let [first, .., last] = notes {
        (last - first).max(1.0)
    } else {
        duration
    };
    let jack_pressure = quantile(&rows.jack_values, 0.92);
    let burst_density = quantile(&rows.row_densities, 0.9);
    let row_mask_base = ((1_i64 << map.key_count) + 1).max(32);
    let row_mask_values: Vec<_> = rows.row_masks.iter().map(|&mask| mask as f64).collect();
    let unique_row_masks: HashSet<_> = rows.row_masks.iter().collect();
    let average_column_count = ratio(rows.column_counts.iter().sum(), map.key_count);
    let column_imbalance = if average_column_count > 0.0 {
        rows.column_counts
            .iter()
            .map(|&count| (count as f64 - average_column_count).abs())
            .sum::<f64>()
            / (map.key_count as f64 * average_column_count)
    } else {
        0.0
    };
    let anchor_pressure = column_imbalance * (0.5 + (jack_pressure / 150.0).min(1.5))
        + (quantile(&rows.column_rates, 0.9) - 7.0).max(0.0) * 0.04;

    // At the same time, a hold head precedes a release for peak overlap.
    hold_events.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap()
            .then_with(|| right.1.cmp(&left.1))
    });
    let mut active = 0_i32;
    let mut peak = 0;
    let mut area = 0.0;
    let mut previous_time = hold_events.first().map_or(0.0, |event| event.0);
    for (time, delta) in hold_events {
        area += (time - previous_time).max(0.0) * f64::from(active);
        active = (active + delta).max(0);
        peak = peak.max(active);
        previous_time = time;
    }
    let off_grid_times;
    let off_grid_rows = if rate == 1.0 {
        &rows.row_times
    } else {
        off_grid_times = distinct_source_row_times(map);
        &off_grid_times
    };
    let adjacent_rehit = ratio(rows.adjacent_column_rehit_notes, note_count);
    let two_back_rehit = ratio(rows.two_back_column_rehit_notes, note_count);
    EzChartFeatureMetrics {
        key_count: map.key_count,
        note_count,
        duration_ms: duration,
        hold_ratio: ratio(hold_durations.len(), note_count),
        chord_ratio,
        two_note_chord_ratio: ratio(rows.two_note_chord_rows, row_count),
        peak_nps_1s,
        peak_nps_5s,
        nps_5s_p50,
        nps_5s_p90,
        nps_5s_p95,
        sustained_nps_10s,
        sustained_nps_30s: count_in_window(notes, 30_000.0) as f64 / 30.0,
        sustained_nps_60s: count_in_window(notes, 60_000.0) as f64 / 60.0,
        active_nps: note_count as f64 / (note_span / 1000.0).max(1.0),
        long_gap_ratio: positive_ratio(rows.long_gap_ms, duration),
        long_gap_count: rows.long_gap_count,
        jack_pressure,
        stream_pressure: quantile(&rows.stream_values, 0.9),
        jumpstream_pressure: quantile(&rows.jumpstream_values, 0.9),
        chordjack_pressure: jack_pressure * (0.28 + chord_ratio * 1.35)
            + burst_density * chord_ratio * 0.6,
        chord_column_overlap_ratio: ratio(rows.chord_pair_overlap_count, rows.chord_pair_count),
        adjacent_column_rehit_share: adjacent_rehit,
        two_back_column_rehit_share: two_back_rehit,
        two_back_column_rehit_excess: two_back_rehit - adjacent_rehit,
        tech_pressure: ratio(rows.direction_changes, row_count) * 4.4
            + ratio(rows.chord_size_changes, row_count) * 3.5
            + chord_ratio * 1.6
            + average(&rows.row_densities) * 0.018,
        row_burst_pressure: quantile(&rows.row_rates, 0.9),
        fast_row_ratio: ratio(rows.fast_row_count, rows.row_intervals.len()),
        row_interval_entropy: bucket_entropy(&rows.row_intervals, 5.0),
        off_grid_row_share: off_grid_row_share(map, off_grid_rows),
        pattern_variety: 0.5 * rao_quadratic_entropy_log(&rows.row_intervals, 5.0, 1)
            + 1.125 * rao_quadratic_entropy_log(&rows.column_intervals, 5.0, 2)
            + 0.11 * rao_quadratic_entropy_log(&tail_intervals, 5.0, 1),
        row_pattern_entropy: bucket_entropy(&row_mask_values, 1.0),
        row_pattern_variety: ratio(unique_row_masks.len(), row_count.min(1 << map.key_count)),
        repeated_row_pattern_ratio: ratio(rows.repeated_row_patterns, row_count.saturating_sub(1)),
        alternating_row_pattern_ratio: ratio(
            rows.alternating_row_patterns,
            row_count.saturating_sub(2),
        ),
        row_pattern_change_rate: positive_ratio(
            rows.row_pattern_change_sum,
            row_count.saturating_sub(1) as f64,
        ),
        row_motif_repeat_ratio: numeric_ngram_repeat_ratio(&rows.row_masks, row_mask_base),
        rhythm_motif_repeat_ratio: numeric_ngram_repeat_ratio(
            &rows.row_signatures,
            row_mask_base * 256,
        ),
        adjacent_motif_repeat_ratio: adjacent_ngram_repeat_ratio(&rows.row_signatures),
        strain_spikiness: strain_spikiness(&rows.row_densities, &rows.row_density_weights),
        sustained_pressure_ratio: sustained_nps_10s / peak_nps_1s.max(peak_nps_5s).max(1.0),
        anchor_pressure,
        ln_release_pressure: count_in_window(releases, 5000.0) as f64 / 5.0
            + quantile(&tail_rates, 0.9) * 0.15,
        ln_density: positive_ratio(total_hold_ms, duration * map.key_count as f64),
        ln_overlap_pressure: positive_ratio(area, duration) + f64::from(peak) * 0.35,
        ln_chord_pressure: ratio(rows.ln_chord_rows, rows.hold_rows),
        ln_hold_duration_avg: average(hold_durations),
        ln_hold_duration_p90: quantile(hold_durations, 0.9),
        chord_size_change_rate: ratio(rows.chord_size_changes, row_count),
        direction_change_rate: ratio(rows.direction_changes, note_count),
        stamina_pressure: sustained_nps_10s,
    }
}

fn distinct_source_row_times(map: EzManiaChartInput<'_>) -> Vec<f64> {
    let mut times: Vec<_> = map
        .notes
        .iter()
        .filter(|note| note.column < map.key_count)
        .map(|note| note.start_time_ms)
        .collect();
    times.sort_by(f64::total_cmp);
    times.dedup_by(|left, right| *left == *right);
    times
}

fn off_grid_row_share(map: EzManiaChartInput<'_>, rows: &[f64]) -> f64 {
    if rows.len() < 2 {
        return 0.0;
    }
    let mut timings: Vec<_> = map
        .timing_points
        .iter()
        .copied()
        .filter(|&(time, length)| time.is_finite() && length > 0.0)
        .collect();
    timings.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap());
    let fallback = if map.bpm > 0.0 {
        60_000.0 / map.bpm
    } else {
        f64::INFINITY
    };
    let mut timing_index = 0;
    let mut off_grid = 0;
    for pair in rows.windows(2) {
        let time = pair[1];
        let gap = time - pair[0];
        while timing_index + 1 < timings.len() && timings[timing_index + 1].0 <= time + 1.0 {
            timing_index += 1;
        }
        let beat_length = timings
            .get(timing_index)
            .map_or(fallback, |&(_, length)| length);
        if gap <= 55.0 {
            off_grid += 1;
        } else if gap <= beat_length {
            let divisor = gap_snap_divisor(gap, beat_length);
            if divisor >= 6 || (divisor == 0 && gap <= beat_length / 2.0) {
                off_grid += 1;
            }
        }
    }
    ratio(off_grid, rows.len())
}

fn gap_snap_divisor(gap: f64, beat_length: f64) -> u32 {
    [1, 2, 3, 4, 6, 8, 12, 16, 24, 32]
        .into_iter()
        .find(|&divisor| {
            let unit = beat_length / f64::from(divisor);
            let steps = (gap / unit).round_ties_even();
            steps >= 1.0 && (steps * unit - gap).abs() <= 2.0 + gap * 0.01
        })
        .unwrap_or(0)
}

fn numeric_ngram_repeat_ratio(values: &[u32], base: i64) -> f64 {
    if values.len() < 5 {
        return 0.0;
    }
    let mut seen = HashSet::new();
    let mut repeated = 0;
    for window in values.windows(4) {
        // Match the unchecked C# long key used for wider Mania modes.
        let key = window.iter().fold(0_i64, |key, &value| {
            key.wrapping_mul(base).wrapping_add(i64::from(value))
        });
        repeated += usize::from(!seen.insert(key));
    }
    ratio(repeated, values.len() - 3)
}

fn adjacent_ngram_repeat_ratio(values: &[u32]) -> f64 {
    let repeated = values
        .windows(8)
        .filter(|window| window[..4] == window[4..])
        .count();
    ratio(repeated, values.len().saturating_sub(7))
}

fn sampled_window_quantiles(notes: &[f64], duration: f64) -> [f64; 3] {
    if notes.is_empty() || duration <= 0.0 {
        return [0.0; 3];
    }
    let last_sample = (duration / 1000.0).floor() as u64;
    let sample_count = u128::from(last_sample) + 1;
    let mut samples = HashMap::<u64, usize>::new();
    for &time in notes.iter().filter(|&&time| time >= 0.0) {
        let containing = (time / 1000.0).floor() as u64;
        for sample in containing.saturating_sub(5)..=last_sample.min(containing.saturating_add(1)) {
            let start = sample as f64 * 1000.0;
            if start <= time && time < start + 5000.0 {
                *samples.entry(sample).or_default() += 1;
            }
        }
    }
    let empty = sample_count - samples.len() as u128;
    let mut counts: Vec<_> = samples.into_values().collect();
    counts.sort_unstable();
    [0.5_f64, 0.9, 0.95].map(|q| {
        let index = (last_sample as f64 * q).floor() as u128;
        index
            .checked_sub(empty)
            .and_then(|i| counts.get(i as usize))
            .map_or(0.0, |&count| count as f64 / 5.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(time: f64, column: usize, duration: f64) -> ManiaNote {
        ManiaNote {
            start_time_ms: time,
            end_time_ms: time + duration,
            column,
        }
    }

    #[test]
    fn matches_ez2lazer_csi_features_for_chords_holds_and_rate() {
        let mut notes = Vec::new();
        for row in 0..70 {
            let time = 100.0
                + row as f64 * 125.0
                + if row >= 35 { 2700.0 } else { 0.0 }
                + if row % 13 == 5 { 20.0 } else { 0.0 };
            let hold = if row % 5 == 1 {
                500.0 + (row % 3) as f64 * 125.0
            } else {
                0.0
            };
            notes.push(note(time, row % 4, hold));
            if row % 3 == 1 {
                let other_hold = if row % 6 == 1 { 250.0 } else { 0.0 };
                notes.push(note(time, (row + 2) % 4, other_hold));
            }
        }
        notes.reverse();
        notes.push(note(50000.0, 5, 0.0));
        notes.push(note(-200.0, 1, 0.0));
        let input = EzManiaChartInput {
            key_count: 4,
            bpm: 120.0,
            total_length_ms: 12000.0,
            notes: &notes,
            timing_points: &[(5000.0, 450.0), (100.0, 500.0)],
        };
        let regular = EzChartFeatureExtractor::extract(input, 1.0).unwrap();
        let faster = EzChartFeatureExtractor::extract(input, 1.25).unwrap();
        assert!(regular.warnings.is_empty());
        assert_eq!(regular.ordered_rows.len(), 71);
        assert_eq!(regular.ordered_rows[0].time_ms, -200.0);
        assert_eq!(regular.notes[0], notes[0]);
        assert_eq!(faster.note_times[0], -160.0);

        // Expected values emitted by the original EzChartFeatureExtractor with .NET 11.
        macro_rules! check {
            (m.$field:ident, $regular:expr, $faster:expr) => {
                for (actual, expected) in [
                    (regular.metrics.$field as f64, $regular),
                    (faster.metrics.$field as f64, $faster),
                ] {
                    assert!(
                        (actual - expected).abs() <= 1e-11 * expected.abs().max(1.0),
                        "{}: expected {expected}, got {actual}",
                        stringify!($field)
                    );
                }
            };
        }
        check!(m.key_count, 4_f64, 4_f64);
        check!(m.note_count, 94_f64, 94_f64);
        check!(m.duration_ms, 12000_f64, 9600_f64);
        check!(m.hold_ratio, 0.2765957446808511_f64, 0.2765957446808511_f64);
        check!(m.chord_ratio, 0.323943661971831_f64, 0.323943661971831_f64);
        check!(
            m.two_note_chord_ratio,
            0.323943661971831_f64,
            0.323943661971831_f64
        );
        check!(m.peak_nps_1s, 12_f64, 15_f64);
        check!(m.peak_nps_5s, 9.6_f64, 9.6_f64);
        check!(m.nps_5s_p50, 5.2_f64, 7.4_f64);
        check!(m.nps_5s_p90, 8.2_f64, 9.2_f64);
        check!(m.nps_5s_p95, 9.2_f64, 9.2_f64);
        check!(m.sustained_nps_10s, 7.9_f64, 9.4_f64);
        check!(
            m.sustained_nps_30s,
            3.1333333333333333_f64,
            3.1333333333333333_f64
        );
        check!(
            m.sustained_nps_60s,
            1.5666666666666667_f64,
            1.5666666666666667_f64
        );
        check!(m.active_nps, 8.086021505376344_f64, 10.10752688172043_f64);
        check!(m.long_gap_ratio, 0.23541666666666666_f64, 0_f64);
        check!(m.long_gap_count, 1_f64, 0_f64);
        check!(m.jack_pressure, 60_f64, 75_f64);
        check!(
            m.stream_pressure,
            3.857142857142857_f64,
            4.571428571428571_f64
        );
        check!(m.jumpstream_pressure, 16_f64, 20_f64);
        check!(
            m.chordjack_pressure,
            46.14929577464789_f64,
            57.686619718309856_f64
        );
        check!(m.chord_column_overlap_ratio, 0_f64, 0_f64);
        check!(m.adjacent_column_rehit_share, 0_f64, 0_f64);
        check!(
            m.two_back_column_rehit_share,
            0.4787234042553192_f64,
            0.4787234042553192_f64
        );
        check!(
            m.two_back_column_rehit_excess,
            0.4787234042553192_f64,
            0.4787234042553192_f64
        );
        check!(
            m.tech_pressure,
            7.997471883776423_f64,
            8.045431404016302_f64
        );
        check!(m.row_burst_pressure, 8_f64, 10_f64);
        check!(m.fast_row_ratio, 0_f64, 0_f64);
        check!(
            m.row_interval_entropy,
            0.8479131717597226_f64,
            0.8479131717597226_f64
        );
        check!(
            m.off_grid_row_share,
            0.5774647887323944_f64,
            0.5774647887323944_f64
        );
        check!(
            m.pattern_variety,
            2.1981524847604077_f64,
            2.1115390825666487_f64
        );
        check!(
            m.row_pattern_entropy,
            2.5825377959579727_f64,
            2.5825377959579727_f64
        );
        check!(m.row_pattern_variety, 0.375_f64, 0.375_f64);
        check!(m.repeated_row_pattern_ratio, 0_f64, 0_f64);
        check!(m.alternating_row_pattern_ratio, 0_f64, 0_f64);
        check!(
            m.row_pattern_change_rate,
            0.6642857142857143_f64,
            0.6642857142857143_f64
        );
        check!(
            m.row_motif_repeat_ratio,
            0.8088235294117647_f64,
            0.8088235294117647_f64
        );
        check!(
            m.rhythm_motif_repeat_ratio,
            0.38235294117647056_f64,
            0.38235294117647056_f64
        );
        check!(m.adjacent_motif_repeat_ratio, 0_f64, 0_f64);
        check!(
            m.strain_spikiness,
            1.1765508989452058_f64,
            1.1765508989452058_f64
        );
        check!(
            m.sustained_pressure_ratio,
            0.6583333333333333_f64,
            0.6266666666666667_f64
        );
        check!(
            m.anchor_pressure,
            0.02872340425531915_f64,
            0.031914893617021274_f64
        );
        check!(m.ln_release_pressure, 3.8_f64, 4.1_f64);
        check!(m.ln_density, 0.2421875_f64, 0.2421875_f64);
        check!(m.ln_overlap_pressure, 2.01875_f64, 2.01875_f64);
        check!(
            m.ln_chord_pressure,
            0.6086956521739131_f64,
            0.6086956521739131_f64
        );
        check!(
            m.ln_hold_duration_avg,
            447.11538461538464_f64,
            357.6923076923077_f64
        );
        check!(m.ln_hold_duration_p90, 750_f64, 600_f64);
        check!(
            m.chord_size_change_rate,
            0.647887323943662_f64,
            0.647887323943662_f64
        );
        check!(
            m.direction_change_rate,
            0.8617021276595744_f64,
            0.8617021276595744_f64
        );
        check!(m.stamina_pressure, 7.9_f64, 9.4_f64);
    }

    #[test]
    fn sparse_sample_quantiles_preserve_half_open_bounds_and_empty_windows() {
        let notes = [-25.0, 0.0, 1000.0, 4999.0, 5000.0, 5000.0];
        assert_eq!(sampled_window_quantiles(&notes, 5000.0), [0.6; 3]);
        assert_eq!(sampled_window_quantiles(&notes, 1e15), [0.0; 3]);
    }

    #[test]
    fn rejects_unrepresentable_inputs_before_grouping_rows() {
        let input = EzManiaChartInput {
            key_count: 4,
            bpm: 120.0,
            total_length_ms: 0.0,
            notes: &[],
            timing_points: &[],
        };
        assert_eq!(
            EzChartFeatureExtractor::extract(input, 0.0),
            Err(EzChartFeatureError::InvalidRate)
        );
        let empty = EzChartFeatureExtractor::extract(input, 1.0).unwrap();
        assert_eq!(empty.metrics.key_count, 4);
        assert_eq!(empty.notes.len(), 0);
        assert_eq!(empty.metrics.peak_nps_1s, 0.0);
        assert_eq!(empty.warnings.len(), 1);
    }
}
