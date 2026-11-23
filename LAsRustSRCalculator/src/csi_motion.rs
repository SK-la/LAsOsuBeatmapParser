//! Four-key movement shares used by the Ez2Lazer CSI speed/tech filing model.

use crate::csi_features::EzManiaChartInput;
use crate::dan_filing::DanMotionFeatures;
use crate::mania::ManiaNote;

const ROW_EPSILON_MS: f64 = 10.0;
const MIN_GAP_MS: f64 = 25.0;
const MAX_GAP_MS: f64 = 400.0;
const MIN_ROWS: usize = 24;

/// Computes four-key wrist-versus-roll shares for CSI/Dan chart analysis.
pub struct EzCsiMotionComputer;

impl EzCsiMotionComputer {
    /// Returns `None` outside 4K or when fewer than 24 timing rows can be measured.
    pub fn compute(chart: EzManiaChartInput<'_>) -> Option<DanMotionFeatures> {
        Self::compute_notes(chart.notes, chart.key_count)
    }

    /// Computes motion features from playable notes in source-map milliseconds.
    pub fn compute_notes(notes: &[ManiaNote], key_count: usize) -> Option<DanMotionFeatures> {
        if key_count != 4 || notes.len() < 32 {
            return None;
        }

        let rows = build_rows(notes);
        if rows.len() < MIN_ROWS {
            return None;
        }

        let mut pair_weight = 0.0;
        let mut same_hand_weight = 0.0;
        let mut mini_jack_weight = 0.0;
        let mut triplet_weight = 0.0;
        let mut cross_hand_trill_weight = 0.0;
        let mut rhythm_weight = 0.0;
        let mut rhythm_break_weight = 0.0;

        for pair in rows.windows(2) {
            let [a, b] = pair else {
                unreachable!("a two-row window has exactly two rows")
            };
            let gap = b.time_ms - a.time_ms;
            if gap <= 0.0 || gap > MAX_GAP_MS {
                continue;
            }

            let weight = weight_for(gap);
            pair_weight += weight;
            if a.columns.len() == 1 && b.columns.len() == 1 {
                let (left, right) = (a.columns[0], b.columns[0]);
                if left == right {
                    mini_jack_weight += weight;
                } else if hand(left) == hand(right) {
                    same_hand_weight += weight;
                }
            }
        }

        for triple in rows.windows(3) {
            let [a, b, c] = triple else {
                unreachable!("a three-row window has exactly three rows")
            };
            let first_gap = b.time_ms - a.time_ms;
            let second_gap = c.time_ms - b.time_ms;
            if first_gap <= 0.0
                || second_gap <= 0.0
                || first_gap > MAX_GAP_MS
                || second_gap > MAX_GAP_MS
            {
                continue;
            }

            let weight = weight_for(first_gap.max(second_gap));
            rhythm_weight += weight;
            if !ratio_is_musical(second_gap / first_gap) {
                rhythm_break_weight += weight;
            }

            if a.columns.len() != 1 || b.columns.len() != 1 || c.columns.len() != 1 {
                continue;
            }

            triplet_weight += weight;
            let (first, middle, last) = (a.columns[0], b.columns[0], c.columns[0]);
            if first == last && first != middle && hand(first) != hand(middle) {
                cross_hand_trill_weight += weight;
            }
        }

        Some(DanMotionFeatures {
            rhythm_break: round4(share(rhythm_break_weight, rhythm_weight)),
            cross_hand_trill: round4(share(cross_hand_trill_weight, triplet_weight)),
            mini_jack: round4(share(mini_jack_weight, pair_weight)),
            same_hand: round4(share(same_hand_weight, pair_weight)),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct MotionRow {
    time_ms: f64,
    columns: Vec<usize>,
}

fn build_rows(notes: &[ManiaNote]) -> Vec<MotionRow> {
    let mut sorted_notes = notes.to_vec();
    if !sorted_notes
        .windows(2)
        .all(|pair| pair[0].start_time_ms <= pair[1].start_time_ms)
    {
        sorted_notes.sort_by(|left, right| left.start_time_ms.total_cmp(&right.start_time_ms));
    }

    let mut rows = Vec::<MotionRow>::new();
    for note in sorted_notes {
        if rows
            .last()
            .is_some_and(|row| note.start_time_ms - row.time_ms <= ROW_EPSILON_MS)
        {
            rows.last_mut()
                .expect("the previous-row predicate requires a row")
                .columns
                .push(note.column);
        } else {
            rows.push(MotionRow {
                time_ms: note.start_time_ms,
                columns: vec![note.column],
            });
        }
    }

    for row in &mut rows {
        row.columns.sort_unstable();
    }
    rows
}

fn hand(column: usize) -> usize {
    usize::from(column >= 2)
}

fn weight_for(gap: f64) -> f64 {
    1.0 / gap.max(MIN_GAP_MS)
}

fn share(numerator: f64, denominator: f64) -> f64 {
    if denominator > 0.0 {
        numerator / denominator
    } else {
        0.0
    }
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round_ties_even() / 10_000.0
}

fn ratio_is_musical(ratio: f64) -> bool {
    const TARGETS: [f64; 9] = [1.0, 2.0, 0.5, 1.5, 2.0 / 3.0, 3.0, 1.0 / 3.0, 4.0, 0.25];
    TARGETS
        .into_iter()
        .any(|target| (ratio - target).abs() <= target * 0.08)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(time_ms: f64, column: usize) -> ManiaNote {
        ManiaNote {
            start_time_ms: time_ms,
            end_time_ms: time_ms,
            column,
        }
    }

    #[test]
    fn rejects_other_keymodes_and_charts_below_the_minimum_size() {
        let notes: Vec<_> = (0..40)
            .map(|index| note(index as f64 * 50.0, index % 4))
            .collect();
        assert!(EzCsiMotionComputer::compute_notes(&notes, 7).is_none());
        assert!(EzCsiMotionComputer::compute_notes(&notes[..31], 4).is_none());

        let clustered: Vec<_> = (0..40)
            .map(|index| note(index as f64 % 2.0, index % 4))
            .collect();
        assert!(EzCsiMotionComputer::compute_notes(&clustered, 4).is_none());
    }

    #[test]
    fn alternating_cross_hand_singles_match_the_expected_weighted_shares() {
        let notes: Vec<_> = (0..80)
            .map(|index| note(index as f64 * 80.0, index % 4))
            .collect();
        let features = EzCsiMotionComputer::compute_notes(&notes, 4).unwrap();
        assert_eq!(features.rhythm_break, 0.0);
        assert_eq!(features.same_hand, 0.5063);
        assert_eq!(features.mini_jack, 0.0);
        assert_eq!(features.cross_hand_trill, 0.0);
    }

    #[test]
    fn repeated_cross_hand_trills_and_non_musical_timing_are_detected() {
        let mut time = 0.0;
        let mut notes = Vec::new();
        for index in 0..80 {
            notes.push(note(time, if index % 2 == 0 { 0 } else { 2 }));
            time += if index % 2 == 0 { 80.0 } else { 100.0 };
        }

        let features = EzCsiMotionComputer::compute_notes(&notes, 4).unwrap();
        assert_eq!(features.cross_hand_trill, 1.0);
        assert_eq!(features.rhythm_break, 1.0);
        assert_eq!(features.same_hand, 0.0);
        assert_eq!(features.mini_jack, 0.0);
    }

    #[test]
    fn same_column_repeats_are_weighted_as_minijacks() {
        let notes: Vec<_> = (0..80)
            .map(|index| note(index as f64 * 80.0, if index % 2 == 0 { 1 } else { 1 }))
            .collect();
        let features = EzCsiMotionComputer::compute_notes(&notes, 4).unwrap();
        assert_eq!(features.mini_jack, 1.0);
        assert_eq!(features.same_hand, 0.0);
        assert_eq!(features.cross_hand_trill, 0.0);
    }

    #[test]
    fn epsilon_row_grouping_preserves_duplicates_and_handles_unsorted_notes() {
        let notes = [note(30.0, 0), note(20.0, 1), note(29.0, 0), note(41.0, 2)];
        let rows = build_rows(&notes);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].time_ms, 20.0);
        assert_eq!(rows[0].columns, vec![0, 0, 1]);
        assert_eq!(rows[1].time_ms, 41.0);
    }

    #[test]
    fn four_decimal_rounding_uses_dotnet_midpoint_to_even() {
        assert_eq!(round4(1.23445), 1.2344);
        assert_eq!(round4(1.23455), 1.2346);
    }
}
