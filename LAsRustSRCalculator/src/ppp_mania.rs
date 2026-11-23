//! Ez2Lazer's Mania PP+ chart strain and star rating.

use crate::mania::ManiaNote;
use crate::skills::{EzPPPManiaSkillAxis, SkillVector};

const INDIVIDUAL_DECAY: f64 = 0.125;
const OVERALL_DECAY: f64 = 0.3;
const SECTION_LENGTH: f64 = 400.0;
const GAMMA: f64 = 0.9;
const STAR_SCALE: f64 = 0.018;
const TOLERANCE_MS: f64 = 1.0;

pub struct EzPPPMania;

impl EzPPPMania {
    /// Returns the strain and star rating axes for playable Mania notes in converter order.
    ///
    /// Supply the original order within chords. Ez2Lazer sorts by start time, and its equal-time
    /// ordering can affect the strain of later objects. Nonpositive and NaN rates fall back to 1.
    pub fn calculate(notes: &[ManiaNote], clock_rate: f64) -> SkillVector<EzPPPManiaSkillAxis> {
        let rate = if clock_rate > 0.0 { clock_rate } else { 1.0 };
        if notes.is_empty() {
            return SkillVector::zeros();
        }
        if notes.iter().any(|note| {
            !note.start_time_ms.is_finite()
                || !note.end_time_ms.is_finite()
                || !(note.start_time_ms / rate).is_finite()
        }) {
            return SkillVector::zeros();
        }

        let mut notes = notes.to_vec();
        let depth_limit = 2 * (notes.len().ilog2() + 1);
        sort_like_dotnet(&mut notes, depth_limit);

        // Empty columns have no effect on overlap detection or strain. Valid Mania charts have few
        // columns. A sparse map also avoids sizing allocations from external column numbers.
        let mut columns = std::collections::BTreeMap::<usize, ColumnState>::new();
        let mut overall = 1.0;
        let mut individual = 0.0_f64;
        let mut previous_start = notes[0].start_time_ms;
        let mut previous_time = previous_start / rate;
        let mut section_end = (previous_time / SECTION_LENGTH).ceil() * SECTION_LENGTH;
        let mut peak = 0.0;
        let mut peaks = Vec::new();

        for note in notes {
            let time = note.start_time_ms / rate;
            let delta = (note.start_time_ms - previous_start) / rate;
            let end = note.end_time_ms.max(note.start_time_ms);
            let mut hold_across = false;
            let mut nearest_release = None::<f64>;

            for state in columns.values() {
                if note.start_time_ms - state.start <= TOLERANCE_MS {
                    continue;
                }
                if state.end - note.start_time_ms > TOLERANCE_MS && end - state.end > TOLERANCE_MS {
                    let release = (end - state.end).abs();
                    nearest_release = Some(nearest_release.map_or(release, |old| old.min(release)));
                }
                hold_across |= state.end - end > TOLERANCE_MS;
            }

            let hold_factor = if hold_across { 1.25 } else { 1.0 };
            let hold_bonus = nearest_release
                .map_or(0.0, |release| 1.0 / (1.0 + (0.27 * (30.0 - release)).exp()));
            let column = columns.get(&note.column);
            let column_delta = column.map_or(0.0, |state| {
                ((note.start_time_ms - state.start) / rate).max(0.0)
            });
            let column_strain = decay(column_delta, INDIVIDUAL_DECAY)
                * column.map_or(0.0, |state| state.strain)
                + 2.0 * hold_factor;

            let first_object = columns.is_empty();
            let next_individual = if !first_object && delta <= TOLERANCE_MS {
                individual.max(column_strain)
            } else {
                column_strain
            };
            let next_overall =
                decay(delta, OVERALL_DECAY) * overall + (1.0 + hold_bonus) * hold_factor;
            let level = next_individual + next_overall;
            let increment = if first_object {
                level
            } else {
                (level - (individual + overall)).max(0.0)
            };

            // A boundary inherits the previous level in each decay channel. The object itself
            // contributes the increase in level, and an exact boundary belongs to the old section.
            while time > section_end {
                peaks.push(peak);
                let gap = section_end - previous_time;
                peak = if first_object {
                    0.0
                } else {
                    decay(gap, INDIVIDUAL_DECAY) * individual + decay(gap, OVERALL_DECAY) * overall
                };
                if peak == 0.0 {
                    // All remaining boundaries before this object inherit zero once both levels
                    // underflow. Those zero peaks sort after every value counted in the sum.
                    section_end = (time / SECTION_LENGTH).ceil() * SECTION_LENGTH;
                    break;
                }
                let next_boundary = section_end + SECTION_LENGTH;
                if next_boundary == section_end {
                    return SkillVector::zeros();
                }
                section_end = next_boundary;
            }
            if increment.is_finite() && increment > peak {
                peak = increment;
            }

            columns.insert(
                note.column,
                ColumnState {
                    start: note.start_time_ms,
                    end,
                    strain: column_strain,
                },
            );
            individual = next_individual;
            overall = next_overall;
            previous_time = time;
            previous_start = note.start_time_ms;
        }

        peaks.push(peak);
        peaks.sort_unstable_by(|a, b| b.total_cmp(a));
        let mut sum = 0.0;
        for (index, peak) in peaks.into_iter().enumerate() {
            if peak > 0.0 {
                sum += peak * GAMMA.powf(index as f64);
            }
        }
        let star = STAR_SCALE * sum;
        SkillVector::from_fn(|_| if star.is_finite() { star } else { 0.0 })
    }
}

struct ColumnState {
    start: f64,
    end: f64,
    strain: f64,
}

fn decay(delta_ms: f64, beta: f64) -> f64 {
    if delta_ms <= 0.0 {
        1.0
    } else {
        beta.powf(delta_ms / 1000.0)
    }
}

// .NET List.Sort uses an introsort with insertion sort on partitions of at most 16.
// The unstable tie order is observable for same-time notes that each change column strain.
fn sort_like_dotnet(mut notes: &mut [ManiaNote], mut depth_limit: u32) {
    while notes.len() > 16 {
        if depth_limit == 0 {
            heap_sort(notes);
            return;
        }
        depth_limit -= 1;
        let hi = notes.len() - 1;
        let middle = hi / 2;
        swap_if_greater(notes, 0, middle);
        swap_if_greater(notes, 0, hi);
        swap_if_greater(notes, middle, hi);
        let pivot = notes[middle].start_time_ms;
        notes.swap(middle, hi - 1);
        let mut left = 0;
        let mut right = hi - 1;
        while left < right {
            left += 1;
            while notes[left].start_time_ms < pivot {
                left += 1;
            }
            right -= 1;
            while notes[right].start_time_ms > pivot {
                right -= 1;
            }
            if left >= right {
                break;
            }
            notes.swap(left, right);
        }
        notes.swap(left, hi - 1);
        sort_like_dotnet(&mut notes[left + 1..], depth_limit);
        notes = &mut notes[..left];
    }
    match notes.len() {
        0 | 1 => return,
        2 => return swap_if_greater(notes, 0, 1),
        3 => {
            swap_if_greater(notes, 0, 1);
            swap_if_greater(notes, 0, 2);
            swap_if_greater(notes, 1, 2);
            return;
        }
        _ => (),
    }
    for index in 1..notes.len() {
        let value = notes[index];
        let mut insertion_index = index;
        while insertion_index > 0 && notes[insertion_index - 1].start_time_ms > value.start_time_ms
        {
            notes[insertion_index] = notes[insertion_index - 1];
            insertion_index -= 1;
        }
        notes[insertion_index] = value;
    }
}

fn swap_if_greater(notes: &mut [ManiaNote], left: usize, right: usize) {
    if notes[left].start_time_ms > notes[right].start_time_ms {
        notes.swap(left, right);
    }
}

fn heap_sort(notes: &mut [ManiaNote]) {
    for node in (0..notes.len() / 2).rev() {
        sift_down(notes, node);
    }
    for end in (1..notes.len()).rev() {
        notes.swap(0, end);
        sift_down(&mut notes[..end], 0);
    }
}

fn sift_down(notes: &mut [ManiaNote], mut node: usize) {
    let value = notes[node];
    while node < notes.len() / 2 {
        let mut child = 2 * node + 1;
        if child + 1 < notes.len() && notes[child].start_time_ms < notes[child + 1].start_time_ms {
            child += 1;
        }
        if value.start_time_ms >= notes[child].start_time_ms {
            break;
        }
        notes[node] = notes[child];
        node = child;
    }
    notes[node] = value;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(column: usize, start_time_ms: f64, end_time_ms: f64) -> ManiaNote {
        ManiaNote {
            column,
            start_time_ms,
            end_time_ms,
        }
    }

    fn assert_axes(notes: &[ManiaNote], rate: f64, expected: f64) {
        let actual = EzPPPMania::calculate(notes, rate);
        for (axis, value) in actual.iter() {
            assert!(
                (value - expected).abs() <= 1e-13,
                "{axis:?}: expected {expected}, got {value}"
            );
        }
    }

    #[test]
    fn empty_and_single_note_baseline() {
        assert_axes(&[], 1.0, 0.0);
        assert_axes(&[note(0, 190.0, 190.0)], 1.0, 0.072);
    }

    #[test]
    fn exact_boundary_stays_in_the_previous_section() {
        let first = note(0, 400.0, 400.0);
        assert_axes(&[first, note(1, 400.0, 400.0)], 1.0, 0.072);
        assert_axes(
            &[first, note(1, 400.0001, 400.0001)],
            1.0,
            0.13679999999999998,
        );
    }

    // Values computed by the Ez2Lazer C# EzPPPMania and EzPPPMath implementation.
    #[test]
    fn overlapping_holds_and_gaps_match_ez2lazer_at_multiple_rates() {
        let notes = [
            note(2, 800.0, 950.0),
            note(1, 350.0, 600.0),
            note(0, 100.0, 500.0),
            note(3, 450.0, 450.0),
            note(0, 530.0, 530.0),
            note(2, 800.0, 800.0),
            note(0, 890.0, 930.0),
            note(1, 920.0, 960.0),
        ];
        assert_axes(&notes, 1.0, 0.3160639923286889);
        assert_axes(&notes, 1.5, 0.2184112157758277);
        assert_axes(&notes, f64::NAN, 0.3160639923286889);
    }

    #[test]
    fn simultaneous_objects_keep_dotnet_partition_order() {
        let mut notes = vec![note(1, 0.0, 120.0)];
        for index in 1..19 {
            notes.push(note(
                (index * index + index / 4) % 4,
                50.0,
                50.0 + index as f64 * 40.0,
            ));
        }
        notes.push(note(1, 70.0, 95.0));
        notes.push(note(3, 801.0, 850.0));

        assert_axes(&notes, 1.0, 0.7108502309489018);
        notes.reverse();
        assert_axes(&notes, 1.0, 0.6924454687945184);
    }

    #[test]
    fn three_object_partition_matches_dotnet_swap_order() {
        let mut notes = [
            note(0, 50.0, 100.0),
            note(1, 50.0, 150.0),
            note(2, 0.0, 0.0),
        ];
        sort_like_dotnet(&mut notes, 4);
        assert_eq!(notes.map(|note| note.column), [2, 1, 0]);
    }
}
