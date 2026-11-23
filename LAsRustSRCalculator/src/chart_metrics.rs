//! Pure beatmap chart measurements corresponding to Ez2Lazer's KPS calculators and chart carrier.

use std::collections::BTreeMap;

use crate::beatmap::GameMode;
use crate::mania::ManiaNote;
use crate::mania_converter::ManiaBeatmapConverter;
use crate::mods::Mod;
use crate::normalization::NormalizedBeatmap;

pub const DEFAULT_KPS_GRAPH_POINTS: usize = 256;
pub const MAX_ESTIMATED_INTERVALS: usize = 100_000;
const KPS_WINDOW_MS_PER_BEAT: f64 = 240_000.0;

/// One note-like object used for chart-wide counting and KPS measurement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChartObjectMeasurement {
    pub start_time_ms: f64,
    pub column: Option<usize>,
    pub is_hold: bool,
    pub end_time_ms: f64,
}

impl From<ManiaNote> for ChartObjectMeasurement {
    fn from(note: ManiaNote) -> Self {
        Self {
            start_time_ms: note.start_time_ms,
            column: Some(note.column),
            is_hold: note.end_time_ms > note.start_time_ms,
            end_time_ms: note.end_time_ms,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct KpsSummary {
    pub average_kps: f64,
    pub max_kps: f64,
    pub kps: Vec<f64>,
    pub interval_count: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChartMetrics {
    pub kps: KpsSummary,
    pub column_note_counts: BTreeMap<usize, usize>,
    pub column_hold_counts: BTreeMap<usize, usize>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ManiaChartMeasurements {
    pub key_count: usize,
    pub bpm: f64,
    pub total_length_ms: f64,
    pub notes: Vec<ManiaNote>,
    pub hold_count: usize,
    pub hold_ratio: f64,
    pub timing_points: Vec<(f64, f64)>,
    pub fully_represented: bool,
    pub chart_metrics: ChartMetrics,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartMetricsError {
    InvalidBucketCount,
    InvalidBpm,
    InvalidManiaMode,
    InvalidManiaColumns,
    InvalidManiaObject,
}

impl std::fmt::Display for ChartMetricsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBucketCount => f.write_str("KPS coarse bucket count must be positive"),
            Self::InvalidBpm => f.write_str("KPS requires a finite, positive beatmap BPM"),
            Self::InvalidManiaMode => {
                f.write_str("native Mania chart measurements require mode=Mania")
            }
            Self::InvalidManiaColumns => f.write_str("Mania chart has an invalid key count"),
            Self::InvalidManiaObject => {
                f.write_str("Mania chart contains an unsupported or invalid object")
            }
        }
    }
}

impl std::error::Error for ChartMetricsError {}

/// Computes per-column counts, hold counts, full 4-beat-window KPS statistics, and a fixed 256-point graph.
pub fn get_all_data_optimized(
    objects: &[ChartObjectMeasurement],
    bpm: f64,
    only_kps: bool,
) -> Result<ChartMetrics, ChartMetricsError> {
    calculate_kps(objects, bpm, only_kps, Some(DEFAULT_KPS_GRAPH_POINTS))
}

/// Computes the legacy unbounded KPS graph without column statistics.
pub fn get_kps_optimized(
    objects: &[ChartObjectMeasurement],
    bpm: f64,
) -> Result<KpsSummary, ChartMetricsError> {
    Ok(calculate_kps(objects, bpm, true, None)?.kps)
}

/// Computes KPS with all intervals retained. Objects must be stable-sorted by start time, as playable beatmaps are.
pub fn get_kps(
    objects: &[ChartObjectMeasurement],
    bpm: f64,
) -> Result<KpsSummary, ChartMetricsError> {
    get_kps_optimized(objects, bpm)
}

/// Builds the fixed coarse graph used by the former GetKpsCoarse path.
pub fn get_kps_coarse(
    objects: &[ChartObjectMeasurement],
    buckets: usize,
) -> Result<Vec<f64>, ChartMetricsError> {
    if buckets == 0 {
        return Err(ChartMetricsError::InvalidBucketCount);
    }
    if objects.is_empty() {
        return Ok(Vec::new());
    }
    let song_start = objects[0].start_time_ms;
    let song_end = objects[objects.len() - 1].start_time_ms;
    let duration = (song_end - song_start).max(1.0);
    let mut counts = vec![0usize; buckets];
    for object in objects {
        let raw_index = ((object.start_time_ms - song_start) * buckets as f64 / duration) as isize;
        let index = raw_index.clamp(0, buckets as isize - 1) as usize;
        counts[index] += 1;
    }
    let interval_ms = duration / buckets as f64;
    Ok(counts
        .into_iter()
        .map(|count| count as f64 / (interval_ms / 1000.0))
        .collect())
}

/// Uniform integer-index resampling with stable endpoints, matching DownsampleToFixedCount.
pub fn downsample_to_fixed_count<T: Copy>(source: &[T], target_count: usize) -> Vec<T> {
    if source.is_empty() || target_count == 0 {
        return Vec::new();
    }
    if source.len() <= target_count {
        return source.to_vec();
    }
    if target_count == 1 {
        return vec![source[0]];
    }
    let last = source.len() - 1;
    (0..target_count)
        .map(|index| source[index * last / (target_count - 1)])
        .collect()
}

/// Counts converted column objects including holds, matching GetColumnNoteCountsOptimized.
pub fn get_column_note_counts(objects: &[ChartObjectMeasurement]) -> BTreeMap<usize, usize> {
    let mut counts = BTreeMap::new();
    for object in objects {
        if let Some(column) = object.column {
            *counts.entry(column).or_insert(0) += 1;
        }
    }
    counts
}

/// Counts notes per column while excluding holds, matching the older EzBeatmapCalculator method.
pub fn get_column_note_counts_without_holds(
    objects: &[ChartObjectMeasurement],
) -> BTreeMap<usize, usize> {
    let mut counts = BTreeMap::new();
    for object in objects {
        if !object.is_hold {
            if let Some(column) = object.column {
                *counts.entry(column).or_insert(0) += 1;
            }
        }
    }
    counts
}

/// Builds an EzChartMeasurements-style object from a native Mania map.
///
/// This uses the Mania converter so sliders, spinners, and holds contribute the playable notes seen by analysis.
/// BPM is explicit because lazer stores the map's representative BPM on BeatmapInfo, not in the raw timing points.
pub fn measure_native_mania(
    beatmap: &NormalizedBeatmap,
    bpm: f64,
    mods: &[Mod],
) -> Result<ManiaChartMeasurements, ChartMetricsError> {
    if beatmap.mode != GameMode::Mania {
        return Err(ChartMetricsError::InvalidManiaMode);
    }
    if !bpm.is_finite() || bpm <= 0.0 {
        return Err(ChartMetricsError::InvalidBpm);
    }
    let (notes, converted_keys) = ManiaBeatmapConverter::convert(beatmap, mods)
        .map_err(|_| ChartMetricsError::InvalidManiaObject)?;
    let measurement_objects: Vec<_> = notes
        .iter()
        .copied()
        .map(ChartObjectMeasurement::from)
        .collect();
    let chart_metrics = get_all_data_optimized(&measurement_objects, bpm, false)?;
    let hold_count = notes
        .iter()
        .filter(|note| note.end_time_ms > note.start_time_ms)
        .count();
    let hold_ratio = if notes.is_empty() {
        0.0
    } else {
        hold_count as f64 / notes.len() as f64
    };
    let total_length_ms = notes
        .first()
        .zip(notes.last())
        .map_or(0.0, |(first, last)| {
            last.end_time_ms.max(last.start_time_ms) - first.start_time_ms
        });
    let timing_points = beatmap
        .control_points
        .timing
        .iter()
        .map(|point| (point.time, point.beat_length))
        .collect();
    Ok(ManiaChartMeasurements {
        key_count: converted_keys,
        bpm,
        total_length_ms,
        notes,
        hold_count,
        hold_ratio,
        timing_points,
        fully_represented: true,
        chart_metrics,
    })
}

fn calculate_kps(
    objects: &[ChartObjectMeasurement],
    bpm: f64,
    only_kps: bool,
    sample_limit: Option<usize>,
) -> Result<ChartMetrics, ChartMetricsError> {
    if !bpm.is_finite() || bpm <= 0.0 {
        return Err(ChartMetricsError::InvalidBpm);
    }
    let mut result = ChartMetrics::default();
    if objects.is_empty() {
        return Ok(result);
    }
    if !only_kps {
        result.column_note_counts = get_column_note_counts(objects);
        for object in objects {
            if object.is_hold {
                if let Some(column) = object.column {
                    *result.column_hold_counts.entry(column).or_insert(0) += 1;
                }
            }
        }
    }
    let interval = KPS_WINDOW_MS_PER_BEAT / bpm;
    let song_end = objects[objects.len() - 1].start_time_ms;
    let estimate_raw = song_end / interval + 1.0;
    let estimated_intervals = if estimate_raw.is_finite() && estimate_raw > 0.0 {
        (estimate_raw as usize).min(MAX_ESTIMATED_INTERVALS)
    } else {
        0
    };
    let sample_indices =
        sample_limit.map(|count| sampled_interval_indices(estimated_intervals, count));
    let capacity = sample_indices
        .as_ref()
        .map_or(estimated_intervals, |indices| indices.len());
    let mut graph = Vec::with_capacity(capacity);
    let (mut current_time, mut current_index, mut interval_index, mut sampled_index) =
        (0.0, 0, 0, 0);
    let (mut sum, mut maximum) = (0.0, 0.0);
    while current_time < song_end && interval_index < MAX_ESTIMATED_INTERVALS {
        let end_time = current_time + interval;
        let mut start_index = current_index;
        while start_index < objects.len() && objects[start_index].start_time_ms < current_time {
            start_index += 1;
        }
        let mut end_index = start_index;
        while end_index < objects.len() && objects[end_index].start_time_ms < end_time {
            end_index += 1;
        }
        let kps = (end_index - start_index) as f64 / (interval / 1000.0);
        sum += kps;
        if interval_index == 0 || kps > maximum {
            maximum = kps;
        }
        match &sample_indices {
            None => graph.push(kps),
            Some(indices)
                if sampled_index < indices.len() && interval_index == indices[sampled_index] =>
            {
                graph.push(kps);
                sampled_index += 1;
            }
            _ => {}
        }
        interval_index += 1;
        current_time += interval;
        current_index = start_index;
    }
    result.kps = KpsSummary {
        average_kps: if interval_index == 0 {
            0.0
        } else {
            sum / interval_index as f64
        },
        max_kps: maximum,
        kps: graph,
        interval_count: interval_index,
    };
    Ok(result)
}

fn sampled_interval_indices(interval_count: usize, target: usize) -> Vec<usize> {
    if interval_count == 0 || target == 0 {
        return Vec::new();
    }
    let sample_count = interval_count.min(target);
    if sample_count == interval_count {
        return (0..interval_count).collect();
    }
    if sample_count == 1 {
        return vec![0];
    }
    let last = interval_count - 1;
    (0..sample_count)
        .map(|index| index * last / (sample_count - 1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(time: f64, column: Option<usize>, is_hold: bool) -> ChartObjectMeasurement {
        ChartObjectMeasurement {
            start_time_ms: time,
            column,
            is_hold,
            end_time_ms: if is_hold { time + 100.0 } else { time },
        }
    }

    #[test]
    fn four_beat_kps_uses_left_closed_right_open_bins_and_all_bin_average() {
        let notes = [
            object(0.0, Some(0), false),
            object(1000.0, Some(1), true),
            object(2000.0, Some(1), false),
            object(4000.0, Some(2), false),
        ];
        let summary = get_kps(&notes, 120.0).unwrap();
        assert_eq!(summary.interval_count, 2);
        assert_eq!(summary.kps, vec![1.0, 0.5]);
        assert_eq!(summary.average_kps, 0.75);
        assert_eq!(summary.max_kps, 1.0);
    }

    #[test]
    fn optimized_metrics_include_holds_in_total_and_track_them_by_column() {
        let notes = [
            object(0.0, Some(2), false),
            object(100.0, Some(2), true),
            object(200.0, Some(4), true),
            object(300.0, None, false),
        ];
        let metrics = get_all_data_optimized(&notes, 120.0, false).unwrap();
        assert_eq!(metrics.column_note_counts, BTreeMap::from([(2, 2), (4, 1)]));
        assert_eq!(metrics.column_hold_counts, BTreeMap::from([(2, 1), (4, 1)]));
        assert_eq!(
            get_column_note_counts_without_holds(&notes),
            BTreeMap::from([(2, 1)])
        );
        assert_eq!(metrics.kps.kps, vec![2.0]);
        assert_eq!(metrics.kps.average_kps, 2.0);
    }

    #[test]
    fn coarse_graph_and_fixed_resampling_keep_endpoints() {
        let notes = [
            object(10.0, None, false),
            object(70.0, None, false),
            object(130.0, None, false),
        ];
        assert_eq!(
            get_kps_coarse(&notes, 2).unwrap(),
            vec![1000.0 / 60.0, 2000.0 / 60.0]
        );
        assert_eq!(
            downsample_to_fixed_count(&(0..=10).collect::<Vec<_>>(), 4),
            vec![0, 3, 6, 10]
        );
        assert_eq!(downsample_to_fixed_count(&[5], 0), Vec::<i32>::new());
    }

    #[test]
    fn empty_and_invalid_inputs_have_explicit_results() {
        assert_eq!(
            get_all_data_optimized(&[], 120.0, false).unwrap(),
            ChartMetrics::default()
        );
        assert_eq!(get_kps(&[], 120.0).unwrap().average_kps, 0.0);
        assert_eq!(get_kps(&[], 0.0), Err(ChartMetricsError::InvalidBpm));
        assert_eq!(
            get_kps_coarse(&[], 0),
            Err(ChartMetricsError::InvalidBucketCount)
        );
    }
}
