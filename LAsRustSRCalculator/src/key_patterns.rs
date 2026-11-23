//! Mania key-pattern radar extraction matching Ez2Lazer's EzKeyPattern analyzers.

use std::borrow::Cow;

use crate::csi_features::EzManiaChartInput;
use crate::mania::ManiaNote;
use crate::skills::{SkillAxis, SkillVector};

pub const TIME_TOLERANCE_MS: f64 = 3.0;

/// Stable key-pattern skill axes used by Ez2Lazer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EzKeyPatternSkillAxis {
    Bracket,
    Chord,
    Jack,
    Delay,
    Dump,
}

impl SkillAxis for EzKeyPatternSkillAxis {
    const ALL: &'static [Self] = &[
        Self::Bracket,
        Self::Chord,
        Self::Jack,
        Self::Delay,
        Self::Dump,
    ];

    fn id(self) -> &'static str {
        match self {
            Self::Bracket => "Bracket",
            Self::Chord => "Chord",
            Self::Jack => "Jack",
            Self::Delay => "Delay",
            Self::Dump => "Dump",
        }
    }

    fn display_name(self) -> &'static str {
        self.id()
    }

    fn in_radar(self) -> bool {
        true
    }
}

/// Radar axes returned by the EzKeyPattern display, including derived LN percentage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EzKeyPatternRadarAxis {
    Bracket,
    Chord,
    LnPercent,
    Jack,
    Delay,
    Dump,
}

impl EzKeyPatternRadarAxis {
    pub const ALL: [Self; 6] = [
        Self::Bracket,
        Self::Chord,
        Self::LnPercent,
        Self::Jack,
        Self::Delay,
        Self::Dump,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Bracket => "Bracket",
            Self::Chord => "Chord",
            Self::LnPercent => "LN%",
            Self::Jack => "Jack",
            Self::Delay => "Delay",
            Self::Dump => "Dump",
        }
    }
}

/// Six-value Radar output in the same canonical axis order as Ez2Lazer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EzKeyPatternRadar {
    values: [f64; 6],
}

impl EzKeyPatternRadar {
    pub fn from_relative(skills: &SkillVector<EzKeyPatternSkillAxis>, hold_ratio: f64) -> Self {
        let ln_percent = if hold_ratio.is_finite() {
            round2_away(hold_ratio.clamp(0.0, 1.0) * 100.0)
        } else {
            0.0
        };

        Self {
            values: [
                skills.get(EzKeyPatternSkillAxis::Bracket),
                skills.get(EzKeyPatternSkillAxis::Chord),
                ln_percent,
                skills.get(EzKeyPatternSkillAxis::Jack),
                skills.get(EzKeyPatternSkillAxis::Delay),
                skills.get(EzKeyPatternSkillAxis::Dump),
            ],
        }
    }

    pub fn get(&self, axis: EzKeyPatternRadarAxis) -> f64 {
        self.values[EzKeyPatternRadarAxis::ALL
            .iter()
            .position(|candidate| *candidate == axis)
            .expect("radar axis is declared in ALL")]
    }

    pub fn as_slice(&self) -> &[f64; 6] {
        &self.values
    }
}

/// One row of nearly simultaneous Mania notes.
#[derive(Clone, Debug, PartialEq)]
pub struct EzManiaKeyPatternRow {
    pub time_ms: f64,
    pub beat_length_ms: f64,
    pub columns: Vec<usize>,
}

impl EzManiaKeyPatternRow {
    pub fn note_count(&self) -> usize {
        self.columns.len()
    }

    pub fn min_column(&self) -> usize {
        self.columns.first().copied().unwrap_or(0)
    }

    pub fn max_column(&self) -> usize {
        self.columns.last().copied().unwrap_or(0)
    }

    pub fn column_span(&self) -> usize {
        if self.columns.len() <= 1 {
            0
        } else {
            self.max_column() - self.min_column()
        }
    }

    pub fn adjacent_link_count(&self) -> usize {
        self.columns
            .windows(2)
            .filter(|columns| columns[1] - columns[0] == 1)
            .count()
    }

    pub fn separated_link_count(&self) -> usize {
        self.columns
            .windows(2)
            .filter(|columns| columns[1] - columns[0] > 1)
            .count()
    }

    pub fn total_gap_count(&self) -> usize {
        self.columns
            .windows(2)
            .map(|columns| columns[1].saturating_sub(columns[0] + 1))
            .sum()
    }

    pub fn average_column(&self) -> f64 {
        if self.columns.is_empty() {
            return 0.0;
        }

        self.columns.iter().sum::<usize>() as f64 / self.columns.len() as f64
    }
}

/// Pairwise features for two successive rows, when they form a sufficiently close subdivision.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EzManiaAdjacentRowPatternMetrics {
    pub delta_ms: f64,
    pub dump_density: f64,
    pub delay_density: f64,
    pub centre_step: f64,
    pub normalized_centre_step: f64,
    pub structure_similarity: f64,
    pub direction: i32,
}

pub struct EzManiaKeyPatternHelper;

impl EzManiaKeyPatternHelper {
    /// Returns notes in time order, borrowing the parsed order when possible.
    pub fn notes_in_time_order(chart: EzManiaChartInput<'_>) -> Cow<'_, [ManiaNote]> {
        let notes = chart.notes;
        if notes
            .windows(2)
            .all(|pair| pair[1].start_time_ms >= pair[0].start_time_ms)
        {
            return Cow::Borrowed(notes);
        }

        let mut ordered = notes.to_vec();
        ordered.sort_by(|left, right| left.start_time_ms.total_cmp(&right.start_time_ms));
        Cow::Owned(ordered)
    }

    /// Builds distinct-column rows, grouping note heads within the shared 3 ms tolerance.
    pub fn build_rows(
        ordered_notes: &[ManiaNote],
        mut beat_length_at: impl FnMut(f64) -> f64,
    ) -> Vec<EzManiaKeyPatternRow> {
        let mut rows = Vec::with_capacity(ordered_notes.len());
        let Some(first) = ordered_notes.first() else {
            return rows;
        };

        let mut current_time = first.start_time_ms;
        let mut columns = vec![first.column];
        for note in &ordered_notes[1..] {
            if (note.start_time_ms - current_time).abs() > TIME_TOLERANCE_MS {
                columns.sort_unstable();
                columns.dedup();
                rows.push(EzManiaKeyPatternRow {
                    time_ms: current_time,
                    beat_length_ms: beat_length_at(current_time),
                    columns: std::mem::take(&mut columns),
                });
                current_time = note.start_time_ms;
            }
            columns.push(note.column);
        }

        columns.sort_unstable();
        columns.dedup();
        rows.push(EzManiaKeyPatternRow {
            time_ms: current_time,
            beat_length_ms: beat_length_at(current_time),
            columns,
        });
        rows
    }

    /// Tests whether a time lies on one of the beat's subdivisions within the chart tolerance.
    pub fn is_on_division(time_ms: f64, beat_length_ms: f64, divisor: i32) -> bool {
        if beat_length_ms <= 0.0 || divisor <= 0 {
            return false;
        }

        let interval = beat_length_ms / divisor as f64;
        if interval <= 0.0 {
            return false;
        }

        let remainder = time_ms % interval;
        remainder <= TIME_TOLERANCE_MS || (interval - remainder).abs() <= TIME_TOLERANCE_MS
    }

    /// Gets the uninherited beat length active at `time_ms`, using the first point before the map's first point.
    pub fn beat_length_at(chart: EzManiaChartInput<'_>, time_ms: f64) -> f64 {
        let points = chart.timing_points;
        if points.is_empty() {
            return if chart.bpm > 0.0 {
                60_000.0 / chart.bpm
            } else {
                0.0
            };
        }

        let index = points.partition_point(|point| point.0 <= time_ms);
        if index > 0 {
            points[index - 1].1
        } else {
            points[0].1
        }
    }

    pub fn try_get_adjacent_pattern_metrics(
        previous: &EzManiaKeyPatternRow,
        current: &EzManiaKeyPatternRow,
        total_columns: usize,
    ) -> Option<EzManiaAdjacentRowPatternMetrics> {
        let beat_length = if previous.beat_length_ms > 0.0 {
            previous.beat_length_ms
        } else {
            current.beat_length_ms
        };
        let delta = current.time_ms - previous.time_ms;
        if beat_length <= 0.0
            || delta <= TIME_TOLERANCE_MS
            || delta > beat_length / 2.0 + TIME_TOLERANCE_MS
        {
            return None;
        }

        let centre_step = (current.average_column() - previous.average_column()).abs();
        let direction = (current.average_column() - previous.average_column()).signum() as i32;
        let normalized_centre_step = if total_columns <= 1 {
            0.0
        } else {
            (centre_step / (total_columns as f64 - 1.0)).clamp(0.0, 1.0)
        };

        Some(EzManiaAdjacentRowPatternMetrics {
            delta_ms: delta,
            dump_density: Self::get_dump_density_score(delta, beat_length),
            delay_density: Self::get_delay_density_score(delta, beat_length),
            centre_step,
            normalized_centre_step,
            structure_similarity: Self::get_row_structure_similarity(previous, current),
            direction,
        })
    }

    pub fn get_row_structure_similarity(
        left: &EzManiaKeyPatternRow,
        right: &EzManiaKeyPatternRow,
    ) -> f64 {
        let note_similarity = 1.0
            - left.note_count().abs_diff(right.note_count()) as f64
                / left.note_count().max(right.note_count()).max(1) as f64;
        let span_similarity = 1.0
            - left.column_span().abs_diff(right.column_span()) as f64
                / left.column_span().max(right.column_span()).max(1) as f64;
        let gap_similarity = 1.0
            - left.total_gap_count().abs_diff(right.total_gap_count()) as f64
                / left.total_gap_count().max(right.total_gap_count()).max(1) as f64;

        (note_similarity * 0.45 + span_similarity * 0.3 + gap_similarity * 0.25).clamp(0.0, 1.0)
    }

    pub fn get_dump_density_score(delta_ms: f64, beat_length_ms: f64) -> f64 {
        if beat_length_ms <= 0.0 || delta_ms <= TIME_TOLERANCE_MS {
            return 0.0;
        }
        if delta_ms <= beat_length_ms / 16.0 + TIME_TOLERANCE_MS {
            1.0
        } else if delta_ms <= beat_length_ms / 12.0 + TIME_TOLERANCE_MS {
            0.95
        } else if delta_ms <= beat_length_ms / 8.0 + TIME_TOLERANCE_MS {
            0.85
        } else if delta_ms <= beat_length_ms / 6.0 + TIME_TOLERANCE_MS {
            0.75
        } else if delta_ms <= beat_length_ms / 4.0 + TIME_TOLERANCE_MS {
            0.6
        } else if delta_ms <= beat_length_ms / 3.0 + TIME_TOLERANCE_MS {
            0.45
        } else if delta_ms <= beat_length_ms / 2.0 + TIME_TOLERANCE_MS {
            0.3
        } else {
            0.0
        }
    }

    pub fn get_delay_density_score(delta_ms: f64, beat_length_ms: f64) -> f64 {
        if beat_length_ms <= 0.0 || delta_ms <= TIME_TOLERANCE_MS {
            return 0.0;
        }
        if delta_ms <= beat_length_ms / 16.0 + TIME_TOLERANCE_MS {
            1.0
        } else if delta_ms <= beat_length_ms / 12.0 + TIME_TOLERANCE_MS {
            0.9
        } else if delta_ms <= beat_length_ms / 8.0 + TIME_TOLERANCE_MS {
            0.75
        } else if delta_ms <= beat_length_ms / 6.0 + TIME_TOLERANCE_MS {
            0.6
        } else {
            0.0
        }
    }

    pub fn get_fine_timing_severity(time_ms: f64, beat_length_ms: f64) -> f64 {
        if beat_length_ms <= 0.0 {
            return 0.0;
        }
        if [1, 2, 3, 4]
            .into_iter()
            .any(|divisor| Self::is_on_division(time_ms, beat_length_ms, divisor))
        {
            return 0.0;
        }
        if Self::is_on_division(time_ms, beat_length_ms, 6) {
            0.55
        } else if Self::is_on_division(time_ms, beat_length_ms, 8) {
            0.68
        } else if Self::is_on_division(time_ms, beat_length_ms, 12) {
            0.82
        } else if Self::is_on_division(time_ms, beat_length_ms, 16) {
            0.9
        } else if Self::is_on_division(time_ms, beat_length_ms, 24) {
            0.96
        } else {
            1.0
        }
    }

    pub fn get_relative_difference(left: f64, right: f64) -> f64 {
        ((left - right).abs() / left.abs().max(right.abs()).max(0.0001)).clamp(0.0, 1.0)
    }

    pub fn scale_to_ten(ratio: f64) -> f64 {
        Self::to_key_pattern_score(ratio * 10.0)
    }

    pub fn to_key_pattern_score(value: f64) -> f64 {
        (value.clamp(0.0, 10.0) * 100.0).round() / 100.0
    }
}

pub struct EzManiaKeyPatternBracket;

impl EzManiaKeyPatternBracket {
    pub fn compute(rows: &[EzManiaKeyPatternRow], total_columns: usize) -> f64 {
        if rows.is_empty() || total_columns == 0 {
            return 0.0;
        }

        let mut structure_score = 0.0;
        let mut multi_note_rows = 0;
        for row in rows.iter().filter(|row| row.note_count() >= 2) {
            multi_note_rows += 1;
            let pair_count = (row.note_count() - 1) as f64;
            let separated_ratio = row.separated_link_count() as f64 / pair_count;
            let adjacent_ratio = row.adjacent_link_count() as f64 / pair_count;
            let gap_spread = if total_columns <= 1 {
                0.0
            } else {
                (row.total_gap_count() as f64 / (total_columns - 1) as f64).clamp(0.0, 1.0)
            };
            let intensity = ((row.note_count() - 1) as f64 / 2.0).clamp(0.0, 1.0);
            structure_score += intensity
                * (separated_ratio * 0.72 + gap_spread * 0.9 - adjacent_ratio * 0.2)
                    .clamp(0.0, 1.0);
        }

        if multi_note_rows == 0 {
            0.0
        } else {
            EzManiaKeyPatternHelper::scale_to_ten(structure_score / multi_note_rows as f64)
        }
    }
}

pub struct EzManiaKeyPatternChord;

impl EzManiaKeyPatternChord {
    pub fn compute(rows: &[EzManiaKeyPatternRow]) -> f64 {
        let mut structure_score = 0.0;
        let mut multi_note_rows = 0;
        for row in rows.iter().filter(|row| row.note_count() >= 2) {
            multi_note_rows += 1;
            let pair_count = (row.note_count() - 1) as f64;
            let adjacent_ratio = row.adjacent_link_count() as f64 / pair_count;
            let separated_ratio = row.separated_link_count() as f64 / pair_count;
            let compactness = if row.column_span() == 0 {
                1.0
            } else {
                ((row.note_count() - 1) as f64 / row.column_span() as f64).clamp(0.0, 1.0)
            };
            let intensity = ((row.note_count() - 1) as f64 / 2.0).clamp(0.0, 1.0);
            structure_score += intensity
                * (adjacent_ratio * 0.72 + compactness * 0.68 - separated_ratio * 0.25)
                    .clamp(0.0, 1.0);
        }

        if multi_note_rows == 0 {
            0.0
        } else {
            EzManiaKeyPatternHelper::scale_to_ten(structure_score / multi_note_rows as f64)
        }
    }
}

pub struct EzManiaKeyPatternJack;

impl EzManiaKeyPatternJack {
    pub fn compute(notes: &[ManiaNote], mut beat_length_at: impl FnMut(f64) -> f64) -> f64 {
        if notes.is_empty() {
            return 0.0;
        }

        let mut groups: Vec<Vec<ManiaNote>> = Vec::new();
        let mut group_columns = Vec::new();
        for note in notes {
            let index = match group_columns
                .iter()
                .position(|&column| column == note.column)
            {
                Some(index) => index,
                None => {
                    group_columns.push(note.column);
                    groups.push(Vec::new());
                    groups.len() - 1
                }
            };
            groups[index].push(*note);
        }

        let mut total_pairs = 0;
        let mut weighted_pairs = 0.0;
        for column_notes in &groups {
            for pair in column_notes.windows(2) {
                let beat_length = beat_length_at(pair[0].start_time_ms);
                if beat_length <= 0.0 {
                    continue;
                }
                total_pairs += 1;
                let delta = pair[1].start_time_ms - pair[0].start_time_ms;
                if delta <= beat_length / 4.0 + TIME_TOLERANCE_MS {
                    weighted_pairs += 1.0;
                } else if delta <= beat_length / 2.0 + TIME_TOLERANCE_MS {
                    weighted_pairs += 0.6;
                }
            }
        }

        if total_pairs == 0 {
            0.0
        } else {
            EzManiaKeyPatternHelper::scale_to_ten(weighted_pairs / total_pairs as f64)
        }
    }
}

pub struct EzManiaKeyPatternDelay;

impl EzManiaKeyPatternDelay {
    pub fn compute(rows: &[EzManiaKeyPatternRow], total_columns: usize) -> f64 {
        if rows.len() < 3 {
            return 0.0;
        }

        let mut delay_score = 0.0;
        let mut analysed_triplets = 0;
        for index in 1..rows.len() - 1 {
            let Some(previous) = EzManiaKeyPatternHelper::try_get_adjacent_pattern_metrics(
                &rows[index - 1],
                &rows[index],
                total_columns,
            ) else {
                continue;
            };
            let Some(next) = EzManiaKeyPatternHelper::try_get_adjacent_pattern_metrics(
                &rows[index],
                &rows[index + 1],
                total_columns,
            ) else {
                continue;
            };

            let fine_timing = EzManiaKeyPatternHelper::get_fine_timing_severity(
                rows[index - 1].time_ms,
                rows[index - 1].beat_length_ms,
            )
            .max(EzManiaKeyPatternHelper::get_fine_timing_severity(
                rows[index].time_ms,
                rows[index].beat_length_ms,
            ))
            .max(EzManiaKeyPatternHelper::get_fine_timing_severity(
                rows[index + 1].time_ms,
                rows[index + 1].beat_length_ms,
            ));
            let fine_density =
                ((previous.delay_density + next.delay_density) / 2.0).max(fine_timing);
            if fine_density <= 0.0 {
                continue;
            }

            let direction_chaos = if previous.direction != 0
                && next.direction != 0
                && previous.direction != next.direction
            {
                1.0
            } else {
                0.0
            };
            let interval_chaos =
                EzManiaKeyPatternHelper::get_relative_difference(previous.delta_ms, next.delta_ms);
            let movement_chaos = EzManiaKeyPatternHelper::get_relative_difference(
                previous.centre_step,
                next.centre_step,
            );
            let structure_chaos =
                1.0 - (previous.structure_similarity + next.structure_similarity) / 2.0;
            let spread = previous
                .normalized_centre_step
                .max(next.normalized_centre_step);

            let mut dump_like = 0.0;
            if previous.direction != 0 && previous.direction == next.direction {
                let interval_uniformity = 1.0 - interval_chaos;
                let movement_uniformity = 1.0 - movement_chaos;
                let structure_uniformity =
                    (previous.structure_similarity + next.structure_similarity) / 2.0;
                dump_like = (interval_uniformity * 0.45
                    + movement_uniformity * 0.35
                    + structure_uniformity * 0.2)
                    .clamp(0.0, 1.0);
            }

            let chaos = (direction_chaos * 0.35
                + interval_chaos * 0.25
                + movement_chaos * 0.15
                + structure_chaos * 0.1
                + spread * 0.15)
                .clamp(0.0, 1.0);
            if chaos <= 0.15 {
                continue;
            }

            delay_score += fine_density * chaos * (1.0 - dump_like * 0.85);
            analysed_triplets += 1;
        }

        if analysed_triplets == 0 {
            0.0
        } else {
            EzManiaKeyPatternHelper::scale_to_ten(delay_score / analysed_triplets as f64)
        }
    }
}

pub struct EzManiaKeyPatternDump;

impl EzManiaKeyPatternDump {
    pub fn compute(rows: &[EzManiaKeyPatternRow], total_columns: usize) -> f64 {
        if rows.len() < 3 {
            return 0.0;
        }

        let mut dump_score = 0.0;
        let mut analysed_triplets = 0;
        for index in 1..rows.len() - 1 {
            let Some(previous) = EzManiaKeyPatternHelper::try_get_adjacent_pattern_metrics(
                &rows[index - 1],
                &rows[index],
                total_columns,
            ) else {
                continue;
            };
            let Some(next) = EzManiaKeyPatternHelper::try_get_adjacent_pattern_metrics(
                &rows[index],
                &rows[index + 1],
                total_columns,
            ) else {
                continue;
            };

            if previous.direction == 0
                || next.direction == 0
                || previous.direction != next.direction
            {
                continue;
            }

            let interval_uniformity = 1.0
                - EzManiaKeyPatternHelper::get_relative_difference(
                    previous.delta_ms,
                    next.delta_ms,
                );
            let movement_uniformity = 1.0
                - EzManiaKeyPatternHelper::get_relative_difference(
                    previous.centre_step,
                    next.centre_step,
                );
            let structure_uniformity =
                (previous.structure_similarity + next.structure_similarity) / 2.0;
            let density = (previous.dump_density + next.dump_density) / 2.0;
            let spread = previous
                .normalized_centre_step
                .max(next.normalized_centre_step);
            let triplet_score = density
                * (interval_uniformity * 0.4
                    + movement_uniformity * 0.35
                    + structure_uniformity * 0.2
                    + spread * 0.05)
                    .clamp(0.0, 1.0);
            if triplet_score <= 0.0 {
                continue;
            }

            dump_score += triplet_score;
            analysed_triplets += 1;
        }

        if analysed_triplets == 0 {
            0.0
        } else {
            EzManiaKeyPatternHelper::scale_to_ten(dump_score / analysed_triplets as f64)
        }
    }
}

/// Pure five-axis chart analysis. LN percentage is derived from CSI hold ratio at radar time.
#[derive(Clone, Debug, PartialEq)]
pub struct EzKeyPatternAnalysis {
    pub relative_skills: SkillVector<EzKeyPatternSkillAxis>,
    pub note_count: usize,
    pub hold_ratio: f64,
}

pub struct EzKeyPatternUnit;

impl EzKeyPatternUnit {
    pub const VERSION: u32 = 20261116;
    pub const RELATIVE_FULL_SCORE: f64 = 10.0;

    pub fn supports_ruleset(ruleset_online_id: i32) -> bool {
        ruleset_online_id == 3
    }

    pub fn is_quantifiable_sunny(sunny_sr: f64) -> bool {
        sunny_sr.is_finite() && sunny_sr > 0.0
    }

    pub fn to_quantified(relative: f64, sunny_sr: f64) -> f64 {
        round2_away(relative / Self::RELATIVE_FULL_SCORE * sunny_sr)
    }

    pub fn to_relative(quantified: f64, sunny_sr: f64) -> f64 {
        if sunny_sr > 0.0 {
            round2_away(quantified / sunny_sr * Self::RELATIVE_FULL_SCORE)
        } else {
            quantified
        }
    }

    pub fn compute(chart: EzManiaChartInput<'_>) -> EzKeyPatternAnalysis {
        let ordered = EzManiaKeyPatternHelper::notes_in_time_order(chart);
        let note_count = ordered.len();
        let hold_ratio = if note_count > 0 {
            ordered
                .iter()
                .filter(|note| note.end_time_ms > note.start_time_ms)
                .count() as f64
                / note_count as f64
        } else {
            0.0
        };

        if ordered.is_empty() {
            return EzKeyPatternAnalysis {
                relative_skills: SkillVector::zeros(),
                note_count: 0,
                hold_ratio: 0.0,
            };
        }

        let total_columns = ordered
            .iter()
            .map(|note| note.column + 1)
            .max()
            .unwrap_or(0);
        let rows = EzManiaKeyPatternHelper::build_rows(&ordered, |time| {
            EzManiaKeyPatternHelper::beat_length_at(chart, time)
        });
        let mut skills = SkillVector::zeros();
        skills.set(
            EzKeyPatternSkillAxis::Bracket,
            EzManiaKeyPatternBracket::compute(&rows, total_columns),
        );
        skills.set(
            EzKeyPatternSkillAxis::Chord,
            EzManiaKeyPatternChord::compute(&rows),
        );
        skills.set(
            EzKeyPatternSkillAxis::Jack,
            EzManiaKeyPatternJack::compute(&ordered, |time| {
                EzManiaKeyPatternHelper::beat_length_at(chart, time)
            }),
        );
        skills.set(
            EzKeyPatternSkillAxis::Delay,
            EzManiaKeyPatternDelay::compute(&rows, total_columns),
        );
        skills.set(
            EzKeyPatternSkillAxis::Dump,
            EzManiaKeyPatternDump::compute(&rows, total_columns),
        );

        EzKeyPatternAnalysis {
            relative_skills: skills,
            note_count,
            hold_ratio: hold_ratio.clamp(0.0, 1.0),
        }
    }

    pub fn compute_radar(chart: EzManiaChartInput<'_>) -> EzKeyPatternRadar {
        let analysis = Self::compute(chart);
        EzKeyPatternRadar::from_relative(&analysis.relative_skills, analysis.hold_ratio)
    }

    pub fn quantify(
        relative: &SkillVector<EzKeyPatternSkillAxis>,
        sunny_sr: f64,
    ) -> SkillVector<EzKeyPatternSkillAxis> {
        SkillVector::from_fn(|axis| Self::to_quantified(relative.get(axis), sunny_sr))
    }

    /// Creates the five persisted dense values, or returns `None` until a usable Sunny SR exists.
    pub fn build_skills_for_persistence(
        analysis: &EzKeyPatternAnalysis,
        sunny_sr: f64,
    ) -> Option<SkillVector<EzKeyPatternSkillAxis>> {
        if analysis.note_count == 0 {
            return Some(analysis.relative_skills.clone());
        }
        Self::is_quantifiable_sunny(sunny_sr)
            .then(|| Self::quantify(&analysis.relative_skills, sunny_sr))
    }
}

fn round2_away(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(time_ms: f64, columns: &[usize], beat_length_ms: f64) -> EzManiaKeyPatternRow {
        EzManiaKeyPatternRow {
            time_ms,
            beat_length_ms,
            columns: columns.to_vec(),
        }
    }

    #[test]
    fn groups_rows_by_three_millisecond_tolerance_and_deduplicates_columns() {
        let notes = [
            ManiaNote {
                start_time_ms: 100.0,
                end_time_ms: 100.0,
                column: 1,
            },
            ManiaNote {
                start_time_ms: 103.0,
                end_time_ms: 103.0,
                column: 0,
            },
            ManiaNote {
                start_time_ms: 103.0,
                end_time_ms: 103.0,
                column: 1,
            },
            ManiaNote {
                start_time_ms: 106.0001,
                end_time_ms: 106.0001,
                column: 2,
            },
        ];
        let rows = EzManiaKeyPatternHelper::build_rows(&notes, |_| 500.0);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].time_ms, 100.0);
        assert_eq!(rows[0].columns, [0, 1]);
        assert_eq!(rows[1].columns, [2]);
    }

    #[test]
    fn bracket_and_chord_synthetic_goldens_match_ez_formulas() {
        let wide_bracket = row(0.0, &[0, 3], 500.0);
        let compact_chord = row(0.0, &[0, 1], 500.0);
        assert_eq!(EzManiaKeyPatternBracket::compute(&[wide_bracket], 4), 5.0);
        assert_eq!(EzManiaKeyPatternChord::compute(&[compact_chord]), 5.0);
    }

    #[test]
    fn jack_density_uses_source_clock_beat_length() {
        let notes = [
            ManiaNote {
                start_time_ms: 0.0,
                end_time_ms: 0.0,
                column: 2,
            },
            ManiaNote {
                start_time_ms: 125.0,
                end_time_ms: 125.0,
                column: 2,
            },
            ManiaNote {
                start_time_ms: 325.0,
                end_time_ms: 325.0,
                column: 2,
            },
            ManiaNote {
                start_time_ms: 625.0,
                end_time_ms: 625.0,
                column: 2,
            },
        ];
        let score = EzManiaKeyPatternJack::compute(&notes, |_| 500.0);
        assert_eq!(score, 5.33);
    }

    #[test]
    fn adjacent_metrics_and_density_boundaries_follow_ez_thresholds() {
        assert_eq!(
            EzManiaKeyPatternHelper::get_dump_density_score(31.0, 500.0),
            1.0
        );
        assert_eq!(
            EzManiaKeyPatternHelper::get_dump_density_score(50.0, 500.0),
            0.85
        );
        assert_eq!(
            EzManiaKeyPatternHelper::get_delay_density_score(100.0, 500.0),
            0.0
        );
        let metrics = EzManiaKeyPatternHelper::try_get_adjacent_pattern_metrics(
            &row(0.0, &[0], 500.0),
            &row(125.0, &[3], 500.0),
            4,
        )
        .unwrap();
        assert_eq!(metrics.normalized_centre_step, 1.0);
        assert_eq!(metrics.direction, 1);
        assert!(
            EzManiaKeyPatternHelper::try_get_adjacent_pattern_metrics(
                &row(0.0, &[0], 500.0),
                &row(253.0001, &[1], 500.0),
                4,
            )
            .is_none()
        );
    }

    #[test]
    fn score_conversion_uses_away_from_zero_cent_rounding() {
        assert_eq!(EzKeyPatternUnit::to_quantified(1.25, 8.0), 1.0);
        assert_eq!(EzKeyPatternUnit::to_quantified(1.255, 8.0), 1.0);
        assert_eq!(EzKeyPatternUnit::to_relative(8.0, 8.0), 10.0);
        assert!(!EzKeyPatternUnit::is_quantifiable_sunny(0.0));
        assert!(EzKeyPatternUnit::is_quantifiable_sunny(0.01));
    }
}
