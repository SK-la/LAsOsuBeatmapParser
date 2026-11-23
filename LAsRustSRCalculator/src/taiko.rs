//! Official osu!taiko difficulty calculation over already-converted Taiko hit objects.
//!
//! The input is the output sequence of `TaikoBeatmapConverter`, including any hits expanded from sliders.
//! This module ports the difficulty-object construction, rhythm grouping, mono-colour pattern preprocessing, four
//! skill evaluators, section strain aggregation, and the final attributes formula. It does not parse `.osu` objects
//! or convert sliders/spinners; the converter must supply each object's resolved end time, effective BPM base
//! (`timing BPM * global slider velocity * scroll speed`, before playback-rate mods), and max-combo contribution.

use std::error::Error;
use std::fmt::{Display, Formatter};

pub use crate::attributes::TaikoDifficultyAttributes;
use crate::mods::{Mod, calculate_rate_with_mods};

const DIFFICULTY_MULTIPLIER: f64 = 0.084375;
const RHYTHM_SKILL_MULTIPLIER: f64 = 0.770 * DIFFICULTY_MULTIPLIER;
const READING_SKILL_MULTIPLIER: f64 = 0.100 * DIFFICULTY_MULTIPLIER;
const COLOUR_SKILL_MULTIPLIER: f64 = 0.375 * DIFFICULTY_MULTIPLIER;
const STAMINA_SKILL_MULTIPLIER: f64 = 0.445 * DIFFICULTY_MULTIPLIER;
const SECTION_LENGTH_MS: f64 = 400.0;
const SECTION_DECAY: f64 = 0.9;
const RHYTHM_STRAIN_DECAY: f64 = 0.4;
const COLOUR_STRAIN_DECAY: f64 = 0.8;
const STAMINA_STRAIN_DECAY: f64 = 0.4;
const RHYTHM_GROUPING_TOLERANCE_MS: f64 = 5.0;

/// The colour of a regular Taiko `Hit` object. Strong/finisher hits use the same colour axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaikoHitType {
    Centre,
    Rim,
}

/// Converted Taiko object kind. Only `Hit` objects participate in the four object evaluators, but rolls and swells
/// remain in the timeline and can change the temporal context of later hit objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaikoObjectType {
    Hit,
    DrumRoll,
    Swell,
}

/// A converter-produced Taiko object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TaikoNote {
    pub start_time_ms: f64,
    pub end_time_ms: f64,
    /// Timing BPM multiplied by global slider velocity and active scroll speed, before DT/HT/rate mods.
    pub effective_bpm: f64,
    pub object_type: TaikoObjectType,
    /// Required for `Hit`; ignored for drum rolls and swells.
    pub hit_type: Option<TaikoHitType>,
    /// Strong/finisher status. It affects scoring and combo, but not current star difficulty evaluators.
    pub is_strong: bool,
    /// `HitObject.GetMaxCombo()` contribution after converter/default generation, including nested judgements.
    pub max_combo_contribution: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaikoDifficultyError {
    InvalidClockRate,
    InvalidOverallDifficulty,
    NonFiniteObjectValue { object_index: usize },
    EndBeforeStart { object_index: usize },
    HitTypeMissing { object_index: usize },
    HitTypeUnexpected { object_index: usize },
    NegativeComboContribution { object_index: usize },
    MaxComboOverflow,
}

impl Display for TaikoDifficultyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidClockRate => {
                f.write_str("Taiko clock rate must be finite and greater than zero")
            }
            Self::InvalidOverallDifficulty => {
                f.write_str("Taiko overall difficulty must be finite")
            }
            Self::NonFiniteObjectValue { object_index } => {
                write!(
                    f,
                    "Taiko object {object_index} has a non-finite time or effective BPM"
                )
            }
            Self::EndBeforeStart { object_index } => {
                write!(f, "Taiko object {object_index} ends before it starts")
            }
            Self::HitTypeMissing { object_index } => {
                write!(f, "Taiko Hit object {object_index} has no centre/rim type")
            }
            Self::HitTypeUnexpected { object_index } => {
                write!(
                    f,
                    "non-Hit Taiko object {object_index} unexpectedly has a centre/rim type"
                )
            }
            Self::NegativeComboContribution { object_index } => {
                write!(
                    f,
                    "Taiko object {object_index} has a negative max-combo contribution"
                )
            }
            Self::MaxComboOverflow => {
                f.write_str("Taiko max combo exceeds the C# Int32 result range")
            }
        }
    }
}

impl Error for TaikoDifficultyError {}

/// Official Taiko difficulty calculator over converter output.
pub struct TaikoDifficultyCalculator;

impl TaikoDifficultyCalculator {
    /// Calculate Taiko star rating and difficulty attributes.
    ///
    /// `notes` must be in the playable object order produced by the Taiko converter. `is_convert` is true when
    /// the source ruleset is osu!standard. Use the playable beatmap's mod-adjusted overall difficulty and
    /// effective BPM before any clock-rate adjustment. This calculation applies rate modifiers to time and BPM.
    pub fn calculate(
        notes: &[TaikoNote],
        mods: &[Mod],
        overall_difficulty: f64,
        is_convert: bool,
    ) -> Result<TaikoDifficultyAttributes, TaikoDifficultyError> {
        let clock_rate =
            calculate_rate_with_mods(mods).map_err(|_| TaikoDifficultyError::InvalidClockRate)?;
        if !overall_difficulty.is_finite() {
            return Err(TaikoDifficultyError::InvalidOverallDifficulty);
        }

        let mut max_combo = 0_i32;
        for (object_index, note) in notes.iter().enumerate() {
            if !note.start_time_ms.is_finite()
                || !note.end_time_ms.is_finite()
                || !note.effective_bpm.is_finite()
            {
                return Err(TaikoDifficultyError::NonFiniteObjectValue { object_index });
            }
            if note.end_time_ms < note.start_time_ms {
                return Err(TaikoDifficultyError::EndBeforeStart { object_index });
            }
            if note.max_combo_contribution < 0 {
                return Err(TaikoDifficultyError::NegativeComboContribution { object_index });
            }
            match (note.object_type, note.hit_type) {
                (TaikoObjectType::Hit, None) => {
                    return Err(TaikoDifficultyError::HitTypeMissing { object_index });
                }
                (TaikoObjectType::Hit, Some(_)) => {}
                (_, Some(_)) => {
                    return Err(TaikoDifficultyError::HitTypeUnexpected { object_index });
                }
                (_, None) => {}
            }
            max_combo = max_combo
                .checked_add(note.max_combo_contribution)
                .ok_or(TaikoDifficultyError::MaxComboOverflow)?;
        }

        if notes.is_empty() {
            return Ok(TaikoDifficultyAttributes {
                mods: mods.to_vec(),
                max_combo,
                ..TaikoDifficultyAttributes::default()
            });
        }

        // The C# calculator creates difficulty hit objects beginning at converted hitobject index 2. The first
        // two objects remain in the time baseline but never become skill events.
        let objects = build_difficulty_objects(notes, clock_rate);
        let colour_data = preprocess_colour(&objects);
        let rhythm_data = preprocess_rhythm(&objects);
        let relax = mods.iter().any(|modifier| matches!(modifier, Mod::Relax));
        let hit_window = calculate_great_window(overall_difficulty, clock_rate);

        let (rhythm_strains, rhythm_values) =
            process_rhythm(&objects, &colour_data, &rhythm_data, hit_window);
        let (reading_strains, reading_values) = process_reading(&objects, &colour_data);
        let colour_strains = process_colour(&objects, &colour_data, &rhythm_data);
        let (stamina_strains, stamina_values) =
            process_stamina(&objects, &colour_data, is_convert, false);
        let (mono_stamina_strains, _) = process_stamina(&objects, &colour_data, is_convert, true);

        let stamina_difficulty_value = difficulty_value(&stamina_strains);
        let rhythm_skill = difficulty_value(&rhythm_strains) * RHYTHM_SKILL_MULTIPLIER;
        let reading_skill = difficulty_value(&reading_strains) * READING_SKILL_MULTIPLIER;
        let colour_skill = difficulty_value(&colour_strains) * COLOUR_SKILL_MULTIPLIER;
        let stamina_skill = stamina_difficulty_value * STAMINA_SKILL_MULTIPLIER;
        let mono_stamina_skill = difficulty_value(&mono_stamina_strains) * STAMINA_SKILL_MULTIPLIER;
        let mono_stamina_factor = if stamina_skill == 0.0 {
            1.0
        } else {
            (mono_stamina_skill / stamina_skill).powf(5.0)
        };

        let stamina_difficult_strains =
            count_top_weighted_strains(&stamina_values, stamina_difficulty_value);
        let pattern_multiplier = (stamina_skill * colour_skill).powf(0.10);
        let strain_length_bonus =
            1.0 + 0.15 * reverse_lerp(stamina_difficult_strains, 1000.0, 1555.0);

        let combined_peaks = combine_peaks(
            &rhythm_strains.section_peaks,
            &reading_strains.section_peaks,
            &colour_strains.section_peaks,
            &stamina_strains.section_peaks,
            pattern_multiplier,
            strain_length_bonus,
            is_convert,
            relax,
        );
        let combined_objects = combine_peaks(
            &rhythm_values,
            &reading_values,
            &colour_strains.object_values,
            &stamina_values,
            pattern_multiplier,
            strain_length_bonus,
            is_convert,
            relax,
        );

        let consistency_factor = if combined_objects.is_empty() {
            0.0
        } else {
            let mut sorted = combined_objects.clone();
            sorted.sort_by(|left, right| right.total_cmp(left));
            let top_count = 1 + sorted.len() / 20;
            let top_average = sorted[..top_count].iter().sum::<f64>() / top_count as f64;
            combined_objects.iter().sum::<f64>() / (top_average * combined_objects.len() as f64)
        };

        let combined_difficulty = if combined_objects.is_empty() {
            0.0
        } else {
            weighted_peaks(&combined_peaks)
        };
        let star_rating = rescale(combined_difficulty * 1.4);

        let skill_sum = rhythm_skill + reading_skill + colour_skill + stamina_skill;
        let skill_rating = star_rating / skill_sum;
        let rhythm_difficulty = rhythm_skill * skill_rating;
        let reading_difficulty = reading_skill * skill_rating;
        let colour_difficulty = colour_skill * skill_rating;
        let stamina_difficulty = stamina_skill * skill_rating;

        Ok(TaikoDifficultyAttributes {
            mods: mods.to_vec(),
            star_rating,
            max_combo,
            mechanical_difficulty: colour_difficulty + stamina_difficulty,
            rhythm_difficulty,
            reading_difficulty,
            colour_difficulty,
            stamina_difficulty,
            mono_stamina_factor,
            consistency_factor,
            stamina_top_strains: stamina_difficult_strains,
        })
    }
}

#[derive(Clone, Debug)]
struct DifficultyObject {
    start_time: f64,
    delta_time: f64,
    effective_bpm: f64,
    object_type: TaikoObjectType,
    hit_type: Option<TaikoHitType>,
    previous: Option<usize>,
    previous_note: Option<usize>,
    next_note: Option<usize>,
    previous_mono: [Option<usize>; 2],
    rhythm_ratio: f64,
}

impl DifficultyObject {
    fn is_hit(&self) -> bool {
        self.object_type == TaikoObjectType::Hit
    }
}

#[derive(Clone, Debug, Default)]
struct ColourObjectData {
    mono_streak: Option<usize>,
    alternating_pattern: Option<usize>,
    repeating_pattern: Option<usize>,
}

#[derive(Clone, Debug)]
struct MonoStreak {
    objects: Vec<usize>,
    hit_type: Option<TaikoHitType>,
    parent_pattern: usize,
    index: usize,
}

#[derive(Clone, Debug)]
struct AlternatingMonoPattern {
    streaks: Vec<usize>,
    parent_repeating: usize,
    index: usize,
}

#[derive(Clone, Debug)]
struct RepeatingHitPattern {
    patterns: Vec<usize>,
    previous: Option<usize>,
    repetition_interval: usize,
}

#[derive(Clone, Debug, Default)]
struct ColourData {
    per_object: Vec<ColourObjectData>,
    streaks: Vec<MonoStreak>,
    alternating_patterns: Vec<AlternatingMonoPattern>,
    repeating_patterns: Vec<RepeatingHitPattern>,
}

#[derive(Clone, Debug, Default)]
struct SameRhythmGroup {
    objects: Vec<usize>,
    previous: Option<usize>,
    interval: f64,
    duration: f64,
    hit_object_interval: Option<f64>,
    hit_object_interval_ratio: f64,
}

#[derive(Clone, Debug)]
struct SamePatternGroup {
    rhythm_groups: Vec<usize>,
    group_interval: f64,
    interval_ratio: f64,
}

#[derive(Clone, Debug, Default)]
struct RhythmData {
    per_object: Vec<RhythmObjectData>,
    same_rhythm_groups: Vec<SameRhythmGroup>,
    same_pattern_groups: Vec<SamePatternGroup>,
}

#[derive(Clone, Debug, Default)]
struct RhythmObjectData {
    same_rhythm_group: Option<usize>,
    same_pattern_group: Option<usize>,
}

#[derive(Clone, Debug, Default)]
struct SkillResult {
    section_peaks: Vec<f64>,
    object_values: Vec<f64>,
}

fn build_difficulty_objects(notes: &[TaikoNote], clock_rate: f64) -> Vec<DifficultyObject> {
    let capacity = notes.len().saturating_sub(2);
    let mut objects: Vec<DifficultyObject> = Vec::with_capacity(capacity);
    let mut last_mono: [Vec<usize>; 2] = [Vec::new(), Vec::new()];
    let mut note_indices: Vec<usize> = Vec::new();

    for source_index in 2..notes.len() {
        let note = notes[source_index];
        let previous_note = if note.object_type == TaikoObjectType::Hit {
            note_indices.last().copied()
        } else {
            None
        };
        let mut previous_mono = [None, None];

        if note.object_type == TaikoObjectType::Hit {
            let colour_index = match note.hit_type.unwrap() {
                TaikoHitType::Centre => 0,
                TaikoHitType::Rim => 1,
            };
            previous_mono[colour_index] = last_mono[colour_index].last().copied();
            last_mono[colour_index].push(objects.len());
            note_indices.push(objects.len());
        }

        let previous = objects.len().checked_sub(1);
        let previous_start_source = notes[source_index - 1].start_time_ms;
        let start_time = note.start_time_ms / clock_rate;
        let delta_time = (note.start_time_ms - previous_start_source) / clock_rate;

        objects.push(DifficultyObject {
            start_time,
            delta_time,
            effective_bpm: note.effective_bpm * clock_rate,
            object_type: note.object_type,
            hit_type: note.hit_type,
            previous,
            previous_note,
            next_note: None,
            previous_mono,
            rhythm_ratio: 1.0,
        });
    }

    let note_objects: Vec<usize> = objects
        .iter()
        .enumerate()
        .filter_map(|(index, object)| object.is_hit().then_some(index))
        .collect();
    for (position, &object_index) in note_objects.iter().enumerate() {
        objects[object_index].previous_note = position
            .checked_sub(1)
            .map(|previous| note_objects[previous]);
        objects[object_index].next_note = note_objects.get(position + 1).copied();
    }

    for index in 0..objects.len() {
        let Some(previous) = objects[index].previous else {
            objects[index].rhythm_ratio = 1.0;
            continue;
        };
        let actual_ratio = objects[index].delta_time / objects[previous].delta_time;
        objects[index].rhythm_ratio = closest_common_rhythm_ratio(actual_ratio);
    }

    objects
}

fn closest_common_rhythm_ratio(actual: f64) -> f64 {
    const COMMON_RATIOS: [f64; 9] = [1.0, 2.0, 0.5, 3.0, 1.0 / 3.0, 1.5, 2.0 / 3.0, 1.25, 0.8];
    let mut best = COMMON_RATIOS[0];
    let mut best_difference = (best - actual).abs();
    for ratio in COMMON_RATIOS.into_iter().skip(1) {
        let difference = (ratio - actual).abs();
        if difference < best_difference {
            best = ratio;
            best_difference = difference;
        }
    }
    best
}

fn preprocess_colour(objects: &[DifficultyObject]) -> ColourData {
    let mut data = ColourData {
        per_object: vec![ColourObjectData::default(); objects.len()],
        ..ColourData::default()
    };

    for (index, object) in objects.iter().enumerate() {
        let previous_type = object
            .previous_note
            .and_then(|previous| objects[previous].hit_type);
        let current_type = if object.is_hit() {
            object.hit_type
        } else {
            None
        };
        let starts_streak =
            !object.is_hit() || object.previous_note.is_none() || previous_type != current_type;

        let streak_index = if starts_streak || data.streaks.is_empty() {
            let streak_index = data.streaks.len();
            data.streaks.push(MonoStreak {
                objects: Vec::new(),
                hit_type: current_type,
                parent_pattern: 0,
                index: 0,
            });
            streak_index
        } else {
            data.streaks.len() - 1
        };

        data.streaks[streak_index].objects.push(index);
        data.per_object[index].mono_streak = Some(streak_index);
    }

    for streak_index in 0..data.streaks.len() {
        let run_length = data.streaks[streak_index].objects.len();
        let pattern_index = if data
            .alternating_patterns
            .last()
            .and_then(|pattern| pattern.streaks.last())
            .map(|previous| data.streaks[*previous].objects.len() == run_length)
            .unwrap_or(false)
        {
            data.alternating_patterns.len() - 1
        } else {
            let index = data.alternating_patterns.len();
            data.alternating_patterns.push(AlternatingMonoPattern {
                streaks: Vec::new(),
                parent_repeating: 0,
                index: 0,
            });
            index
        };

        data.streaks[streak_index].parent_pattern = pattern_index;
        data.streaks[streak_index].index = data.alternating_patterns[pattern_index].streaks.len();
        data.alternating_patterns[pattern_index]
            .streaks
            .push(streak_index);
    }

    let mut pattern_index = 0;
    while pattern_index < data.alternating_patterns.len() {
        let coupled = pattern_index + 2 < data.alternating_patterns.len()
            && alternating_is_repetition(
                &data.alternating_patterns[pattern_index],
                &data.alternating_patterns[pattern_index + 2],
                &data.streaks,
            );

        let repeating_index = data.repeating_patterns.len();
        let previous_repeating = repeating_index.checked_sub(1);
        data.repeating_patterns.push(RepeatingHitPattern {
            patterns: Vec::new(),
            previous: previous_repeating,
            repetition_interval: 17,
        });

        if !coupled {
            data.repeating_patterns[repeating_index]
                .patterns
                .push(pattern_index);
            data.alternating_patterns[pattern_index].parent_repeating = repeating_index;
            data.alternating_patterns[pattern_index].index = 0;
            pattern_index += 1;
            continue;
        }

        let mut is_coupled = coupled;
        while is_coupled {
            data.repeating_patterns[repeating_index]
                .patterns
                .push(pattern_index);
            data.alternating_patterns[pattern_index].parent_repeating = repeating_index;
            data.alternating_patterns[pattern_index].index =
                data.repeating_patterns[repeating_index].patterns.len() - 1;
            pattern_index += 1;
            is_coupled = pattern_index + 2 < data.alternating_patterns.len()
                && alternating_is_repetition(
                    &data.alternating_patterns[pattern_index],
                    &data.alternating_patterns[pattern_index + 2],
                    &data.streaks,
                );
        }

        for _ in 0..2 {
            data.repeating_patterns[repeating_index]
                .patterns
                .push(pattern_index);
            data.alternating_patterns[pattern_index].parent_repeating = repeating_index;
            data.alternating_patterns[pattern_index].index =
                data.repeating_patterns[repeating_index].patterns.len() - 1;
            pattern_index += 1;
        }
    }

    for repeating_index in 0..data.repeating_patterns.len() {
        let current = data.repeating_patterns[repeating_index].clone();
        let mut previous = current.previous;
        let mut interval = 1;
        while interval < 16 {
            let Some(previous_index) = previous else {
                break;
            };
            if repeating_is_repetition(
                &current,
                &data.repeating_patterns[previous_index],
                &data.alternating_patterns,
                &data.streaks,
            ) {
                data.repeating_patterns[repeating_index].repetition_interval = interval.min(16);
                break;
            }
            previous = data.repeating_patterns[previous_index].previous;
            interval += 1;
        }
    }

    for (streak_index, streak) in data.streaks.iter().enumerate() {
        let pattern_index = streak.parent_pattern;
        let repeating_index = data.alternating_patterns[pattern_index].parent_repeating;
        for object_index in &streak.objects {
            data.per_object[*object_index].alternating_pattern = Some(pattern_index);
            data.per_object[*object_index].repeating_pattern = Some(repeating_index);
            debug_assert_eq!(
                data.per_object[*object_index].mono_streak,
                Some(streak_index)
            );
        }
    }

    data
}

fn alternating_is_repetition(
    current: &AlternatingMonoPattern,
    previous: &AlternatingMonoPattern,
    streaks: &[MonoStreak],
) -> bool {
    same_mono_length(current, previous, streaks)
        && current.streaks.len() == previous.streaks.len()
        && streaks[current.streaks[0]].hit_type == streaks[previous.streaks[0]].hit_type
}

fn same_mono_length(
    current: &AlternatingMonoPattern,
    previous: &AlternatingMonoPattern,
    streaks: &[MonoStreak],
) -> bool {
    streaks[current.streaks[0]].objects.len() == streaks[previous.streaks[0]].objects.len()
}

fn repeating_is_repetition(
    current: &RepeatingHitPattern,
    previous: &RepeatingHitPattern,
    patterns: &[AlternatingMonoPattern],
    streaks: &[MonoStreak],
) -> bool {
    current.patterns.len() == previous.patterns.len()
        && current.patterns.iter().zip(&previous.patterns).take(2).all(
            |(current_pattern, previous_pattern)| {
                same_mono_length(
                    &patterns[*current_pattern],
                    &patterns[*previous_pattern],
                    streaks,
                )
            },
        )
}

fn preprocess_rhythm(objects: &[DifficultyObject]) -> RhythmData {
    let mut data = RhythmData {
        per_object: vec![RhythmObjectData::default(); objects.len()],
        ..RhythmData::default()
    };
    let note_objects: Vec<usize> = objects
        .iter()
        .enumerate()
        .filter_map(|(i, o)| o.is_hit().then_some(i))
        .collect();
    let rhythm_chunks = group_by_interval(&note_objects, |index| objects[*index].delta_time);

    for chunk in rhythm_chunks {
        let chunk_index = data.same_rhythm_groups.len();
        let previous = chunk_index.checked_sub(1);
        let interval = previous
            .map(|previous_index| {
                let current_start = objects[chunk[0]].start_time;
                let previous_start =
                    objects[data.same_rhythm_groups[previous_index].objects[0]].start_time;
                if (current_start - previous_start).abs() <= RHYTHM_GROUPING_TOLERANCE_MS {
                    0.0
                } else {
                    current_start - previous_start
                }
            })
            .unwrap_or(f64::INFINITY);
        let duration = if chunk.len() <= 1 {
            0.0
        } else {
            objects[*chunk.last().unwrap()].start_time - objects[chunk[0]].start_time
        };

        let normalized_delta_times = normalize_delta_times(&chunk, objects);
        let intervals: Vec<f64> = normalized_delta_times.into_iter().skip(1).collect();
        let modal_delta = intervals.first().map(|value| value.round_ties_even());
        let previous_interval =
            previous.and_then(|index| data.same_rhythm_groups[index].hit_object_interval);
        let hit_object_interval = modal_delta.map(|current| {
            if previous_interval
                .map(|previous| (current - previous).abs() <= RHYTHM_GROUPING_TOLERANCE_MS)
                .unwrap_or(false)
            {
                previous_interval.unwrap()
            } else {
                current
            }
        });
        let hit_object_interval_ratio = previous_interval
            .zip(hit_object_interval)
            .map(|(previous, current)| current / previous)
            .unwrap_or(1.0);

        for object_index in &chunk {
            data.per_object[*object_index].same_rhythm_group = Some(chunk_index);
        }
        data.same_rhythm_groups.push(SameRhythmGroup {
            objects: chunk,
            previous,
            interval,
            duration,
            hit_object_interval,
            hit_object_interval_ratio,
        });
    }

    let rhythm_group_indices: Vec<usize> = (0..data.same_rhythm_groups.len()).collect();
    let pattern_chunks = group_by_interval(&rhythm_group_indices, |index| {
        data.same_rhythm_groups[*index].interval
    });
    for chunk in pattern_chunks {
        let group_index = data.same_pattern_groups.len();
        let previous = group_index.checked_sub(1);
        let group_interval = if chunk.len() > 1 {
            data.same_rhythm_groups[chunk[1]].interval
        } else {
            data.same_rhythm_groups[chunk[0]].interval
        };
        let interval_ratio = previous
            .map(|previous_index| {
                group_interval / data.same_pattern_groups[previous_index].group_interval
            })
            .unwrap_or(1.0);
        for rhythm_group in &chunk {
            for object_index in &data.same_rhythm_groups[*rhythm_group].objects {
                data.per_object[*object_index].same_pattern_group = Some(group_index);
            }
        }
        data.same_pattern_groups.push(SamePatternGroup {
            rhythm_groups: chunk,
            group_interval,
            interval_ratio,
        });
    }

    data
}

fn normalize_delta_times(group: &[usize], objects: &[DifficultyObject]) -> Vec<f64> {
    let mut distinct: Vec<f64> = group
        .iter()
        .map(|index| objects[*index].delta_time)
        .collect();
    distinct.sort_by(f64::total_cmp);
    distinct.dedup_by(|left, right| *left == *right);
    let mut clusters: Vec<Vec<f64>> = Vec::new();
    for value in distinct {
        if let Some(cluster) = clusters.last_mut() {
            if (value - cluster[0]).abs() <= RHYTHM_GROUPING_TOLERANCE_MS {
                cluster.push(value);
                continue;
            }
        }
        clusters.push(vec![value]);
    }
    let mut lookup = Vec::new();
    for mut cluster in clusters {
        cluster.sort_by(f64::total_cmp);
        let middle = cluster.len() / 2;
        let median = if cluster.len() % 2 == 1 {
            cluster[middle]
        } else {
            (cluster[middle - 1] + cluster[middle]) / 2.0
        };
        for value in cluster {
            lookup.push((value, median));
        }
    }
    group
        .iter()
        .map(|index| {
            let value = objects[*index].delta_time;
            lookup
                .iter()
                .find(|(source, _)| *source == value)
                .map(|(_, median)| *median)
                .unwrap_or(value)
        })
        .collect()
}

fn group_by_interval<T: Copy>(objects: &[T], interval: impl Fn(&T) -> f64) -> Vec<Vec<T>> {
    let mut groups = Vec::new();
    let mut index = 0;
    while index < objects.len() {
        let mut grouped = vec![objects[index]];
        index += 1;
        let mut returned_early = false;
        while index < objects.len().saturating_sub(1) {
            let left = interval(&objects[index]);
            let right = interval(&objects[index + 1]);
            if !almost_equals(left, right, RHYTHM_GROUPING_TOLERANCE_MS) {
                if right > left + RHYTHM_GROUPING_TOLERANCE_MS {
                    grouped.push(objects[index]);
                    index += 1;
                }
                returned_early = true;
                break;
            }
            grouped.push(objects[index]);
            index += 1;
        }
        if !returned_early
            && objects.len() > 2
            && index < objects.len()
            && almost_equals(
                interval(&objects[objects.len() - 1]),
                interval(&objects[objects.len() - 2]),
                RHYTHM_GROUPING_TOLERANCE_MS,
            )
        {
            grouped.push(objects[index]);
            index += 1;
        }
        groups.push(grouped);
    }
    groups
}

fn process_rhythm(
    objects: &[DifficultyObject],
    colour: &ColourData,
    rhythm: &RhythmData,
    hit_window: f64,
) -> (SkillResult, Vec<f64>) {
    let mut state = SkillState::new(RHYTHM_STRAIN_DECAY);
    let mut values = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        let strain_addition = if object.is_hit() {
            let rhythm_difficulty = evaluate_rhythm(index, objects, rhythm, hit_window);
            let stamina_difficulty = evaluate_stamina(index, objects, colour, false, false) - 0.5;
            rhythm_difficulty * logistic(stamina_difficulty, 1.0 / 15.0, 50.0, 1.0)
        } else {
            0.0
        };
        state.advance_sections(
            index,
            objects,
            state.current_strain,
            RHYTHM_STRAIN_DECAY,
            false,
        );
        state.current_strain *= RHYTHM_STRAIN_DECAY.powf(object.delta_time / 1000.0);
        state.current_strain += strain_addition;
        values.push(state.record(state.current_strain));
    }
    let result = state.finish();
    (result, values)
}

fn process_reading(objects: &[DifficultyObject], colour: &ColourData) -> (SkillResult, Vec<f64>) {
    let mut state = SkillState::new(RHYTHM_STRAIN_DECAY);
    let mut reading_strain = 0.0;
    let mut values = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        state.advance_sections(
            index,
            objects,
            state.current_strain,
            RHYTHM_STRAIN_DECAY,
            false,
        );
        state.current_strain *= RHYTHM_STRAIN_DECAY.powf(object.delta_time / 1000.0);
        if !object.is_hit() {
            values.push(state.record(state.current_strain));
            continue;
        }

        let mono_index = mono_streak_position(index, colour).unwrap_or(0) as f64;
        reading_strain *= logistic(mono_index, 4.0, -1.0 / 25.0, 0.5) + 0.5;
        reading_strain *= RHYTHM_STRAIN_DECAY;
        reading_strain += evaluate_reading(object) * 1.0;
        state.current_strain += reading_strain;
        values.push(state.record(state.current_strain));
    }
    let result = state.finish();
    (result, values)
}

fn process_colour(
    objects: &[DifficultyObject],
    colour: &ColourData,
    rhythm: &RhythmData,
) -> SkillResult {
    let mut state = SkillState::new(COLOUR_STRAIN_DECAY);
    for (index, object) in objects.iter().enumerate() {
        let strain_addition = evaluate_colour(index, objects, colour, rhythm) * 0.12;
        state.advance_sections(
            index,
            objects,
            state.current_strain,
            COLOUR_STRAIN_DECAY,
            false,
        );
        state.current_strain *= COLOUR_STRAIN_DECAY.powf(object.delta_time / 1000.0);
        state.current_strain += strain_addition;
        state.record(state.current_strain);
    }
    state.finish()
}

fn process_stamina(
    objects: &[DifficultyObject],
    colour: &ColourData,
    is_convert: bool,
    single_colour: bool,
) -> (SkillResult, Vec<f64>) {
    let mut state = SkillState::new(STAMINA_STRAIN_DECAY);
    let mut current_strain = 0.0;
    let mut values = Vec::with_capacity(objects.len());

    for (index, object) in objects.iter().enumerate() {
        state.advance_sections(
            index,
            objects,
            current_strain,
            STAMINA_STRAIN_DECAY,
            single_colour,
        );
        current_strain *= STAMINA_STRAIN_DECAY.powf(object.delta_time / 1000.0);
        let stamina_difficulty =
            evaluate_stamina(index, objects, colour, is_convert, single_colour);
        let mono_index = mono_streak_position(index, colour).unwrap_or(0);
        let mono_length_bonus = if is_convert {
            1.0
        } else {
            1.0 + 0.5 * reverse_lerp(mono_index as f64, 5.0, 20.0)
        };
        let mut increment = stamina_difficulty * 1.1;
        if !single_colour {
            increment *= mono_length_bonus;
        }
        current_strain += increment;

        let strain_value = if single_colour {
            logistic_exponent(-(mono_index as f64 - 10.0) / 2.0) * current_strain
        } else {
            current_strain
        };

        values.push(state.record(strain_value));
    }

    let result = state.finish();
    (result, values)
}

struct SkillState {
    current_strain: f64,
    current_section_peak: f64,
    current_section_end: f64,
    section_peaks: Vec<f64>,
    object_values: Vec<f64>,
}

impl SkillState {
    fn new(_decay_base: f64) -> Self {
        Self {
            current_strain: 0.0,
            current_section_peak: 0.0,
            current_section_end: 0.0,
            section_peaks: Vec::new(),
            object_values: Vec::new(),
        }
    }

    fn advance_sections(
        &mut self,
        index: usize,
        objects: &[DifficultyObject],
        previous_strain: f64,
        decay_base: f64,
        zero_section_initial: bool,
    ) {
        let current = &objects[index];
        if index == 0 {
            self.current_section_end =
                (current.start_time / SECTION_LENGTH_MS).ceil() * SECTION_LENGTH_MS;
        }

        while current.start_time > self.current_section_end {
            self.section_peaks.push(self.current_section_peak);
            let initial = if zero_section_initial {
                0.0
            } else {
                let previous_start = current
                    .previous
                    .map(|previous| objects[previous].start_time)
                    .unwrap_or(0.0);
                previous_strain
                    * decay_base.powf((self.current_section_end - previous_start) / 1000.0)
            };
            self.current_section_peak = initial;
            self.current_section_end += SECTION_LENGTH_MS;
        }
    }

    fn record(&mut self, strain_value: f64) -> f64 {
        self.current_section_peak = self.current_section_peak.max(strain_value);
        self.object_values.push(strain_value);
        strain_value
    }

    fn finish(self) -> SkillResult {
        let mut section_peaks = self.section_peaks;
        section_peaks.push(self.current_section_peak);
        SkillResult {
            section_peaks,
            object_values: self.object_values,
        }
    }
}

fn difficulty_value(result: &SkillResult) -> f64 {
    weighted_peaks(&result.section_peaks)
}

fn weighted_peaks(peaks: &[f64]) -> f64 {
    let mut peaks: Vec<f64> = peaks.iter().copied().filter(|peak| *peak > 0.0).collect();
    peaks.sort_by(|left, right| right.total_cmp(left));
    let mut value = 0.0;
    let mut weight = 1.0;
    for peak in peaks {
        value += peak * weight;
        weight *= SECTION_DECAY;
    }
    value
}

fn count_top_weighted_strains(values: &[f64], difficulty: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let consistent_top = difficulty * (1.0 - SECTION_DECAY);
    if consistent_top == 0.0 {
        return values.len() as f64;
    }
    values
        .iter()
        .map(|strain| logistic(strain / consistent_top, 0.88, 10.0, 1.1))
        .sum()
}

fn evaluate_stamina(
    index: usize,
    objects: &[DifficultyObject],
    colour: &ColourData,
    _is_convert: bool,
    _single_colour: bool,
) -> f64 {
    let current = &objects[index];
    if !current.is_hit() {
        return 0.0;
    }

    let Some(previous_previous) = index.checked_sub(2) else {
        return 0.5;
    };

    let previous_same_colour = current
        .hit_type
        .and_then(|hit_type| current.previous_mono[colour_index(hit_type)]);
    let fingers = available_fingers(index, objects, colour);
    let previous_mono = if fingers == 0 {
        None
    } else {
        let mut pointer = previous_same_colour;
        for _ in 1..fingers {
            pointer = pointer.and_then(|previous| {
                objects[previous].previous_mono[colour_index(current.hit_type.unwrap())]
            });
        }
        pointer
    };

    let mut object_strain = 0.5;
    if let Some(previous_mono) = previous_mono {
        object_strain += speed_bonus(current.start_time - objects[previous_mono].start_time)
            + 0.5 * speed_bonus(current.start_time - objects[previous_previous].start_time);
    }
    object_strain
}

fn available_fingers(index: usize, objects: &[DifficultyObject], colour: &ColourData) -> usize {
    let Some(streak_index) = colour.per_object[index].mono_streak else {
        return 8;
    };
    let streak = &colour.streaks[streak_index];
    let previous_colour_change = objects[streak.objects[0]].previous_note;
    let next_colour_change = objects[*streak.objects.last().unwrap()].next_note;
    if previous_colour_change
        .map(|previous| objects[index].start_time - objects[previous].start_time < 300.0)
        .unwrap_or(false)
    {
        return 2;
    }
    if next_colour_change
        .map(|next| objects[next].start_time - objects[index].start_time < 300.0)
        .unwrap_or(false)
    {
        return 2;
    }
    8
}

fn speed_bonus(interval: f64) -> f64 {
    20.0 / interval.max(1.0)
}

fn mono_streak_position(index: usize, colour: &ColourData) -> Option<usize> {
    let streak = colour.per_object[index].mono_streak?;
    colour.streaks[streak]
        .objects
        .iter()
        .position(|object| *object == index)
}

fn evaluate_rhythm(
    index: usize,
    objects: &[DifficultyObject],
    rhythm: &RhythmData,
    hit_window: f64,
) -> f64 {
    let current = &objects[index];
    if !current.is_hit() {
        return 0.0;
    }

    let mut same_rhythm = 0.0;
    let mut same_pattern = 0.0;
    let mut interval_penalty = 1.0;
    let mut gap_penalty = 1.0;

    if let Some(group_index) = rhythm.per_object[index].same_rhythm_group {
        let group = &rhythm.same_rhythm_groups[group_index];
        if group.objects.first() == Some(&index) {
            same_rhythm += 10.0 * evaluate_same_rhythm_group(group, rhythm, hit_window);
            interval_penalty =
                repeated_interval_penalty_with_window(group_index, rhythm, hit_window);
            gap_penalty = long_gap_penalty(group.previous, rhythm, objects);
        }
    }

    if let Some(group_index) = rhythm.per_object[index].same_pattern_group {
        let group = &rhythm.same_pattern_groups[group_index];
        if group
            .rhythm_groups
            .first()
            .and_then(|rhythm_group| rhythm.same_rhythm_groups[*rhythm_group].objects.first())
            == Some(&index)
        {
            same_pattern += 1.15 * ratio_difficulty(group.interval_ratio);
        }
    }

    same_rhythm.max(same_pattern) * interval_penalty * gap_penalty
}

fn evaluate_same_rhythm_group(
    group: &SameRhythmGroup,
    rhythm: &RhythmData,
    hit_window: f64,
) -> f64 {
    let mut interval_difficulty = ratio_difficulty(group.hit_object_interval_ratio);
    let previous_interval = group
        .previous
        .and_then(|previous| rhythm.same_rhythm_groups[previous].hit_object_interval);
    let group_index = rhythm
        .same_rhythm_groups
        .iter()
        .position(|candidate| std::ptr::eq(candidate, group))
        .unwrap_or(0);
    interval_difficulty *= repeated_interval_penalty_with_window(group_index, rhythm, hit_window);

    if let Some(previous_interval) = previous_interval {
        if group.objects.len() > 1 {
            let expected_duration = previous_interval * group.objects.len() as f64;
            let duration_difference = group.duration - expected_duration;
            if duration_difference > 0.0 {
                interval_difficulty *= logistic(duration_difference / hit_window, 0.35, 2.0, 1.0);
            }
        }
    }

    interval_difficulty *= logistic(group.duration / hit_window, 0.3, 2.0, 1.0);
    interval_difficulty.powf(0.75)
}

fn repeated_interval_penalty_with_window(
    group_index: usize,
    rhythm: &RhythmData,
    hit_window: f64,
) -> f64 {
    let group = &rhythm.same_rhythm_groups[group_index];
    same_interval_penalty(group, rhythm, 3).min(if group.objects.len() < 6 {
        same_interval_penalty(group, rhythm, 4)
    } else {
        1.0
    }) * (1.0 - group.duration * 2.0 / hit_window).max(0.5)
}

fn same_interval_penalty(
    group: &SameRhythmGroup,
    rhythm: &RhythmData,
    interval_count: usize,
) -> f64 {
    let mut intervals = Vec::new();
    let mut current = Some(group);
    for _ in 0..interval_count {
        let Some(value) = current else {
            break;
        };
        if let Some(interval) = value.hit_object_interval {
            intervals.push(interval);
        }
        current = value
            .previous
            .map(|previous| &rhythm.same_rhythm_groups[previous]);
    }

    if intervals.len() < interval_count {
        return 1.0;
    }
    for i in 0..intervals.len() {
        for j in i + 1..intervals.len() {
            let ratio = intervals[i] / intervals[j];
            if (1.0 - ratio).abs() <= 0.1 {
                return 0.8;
            }
        }
    }
    1.0
}

fn long_gap_penalty(
    previous_group: Option<usize>,
    rhythm: &RhythmData,
    objects: &[DifficultyObject],
) -> f64 {
    let Some(previous_group_index) = previous_group else {
        return 1.0;
    };
    let previous = &rhythm.same_rhythm_groups[previous_group_index];
    let first = objects[previous.objects[0]].start_time;
    let gap_interval = objects[previous.objects[0]].delta_time;
    let rhythm_interval = previous.hit_object_interval.unwrap_or(gap_interval);
    let gap_ratio = gap_interval / rhythm_interval.max(1.0);
    let gap_factor = logistic(gap_ratio, 1.75, 20.0, 1.0);
    let length_factor = reverse_lerp(previous.objects.len() as f64, 8.0, 2.0);
    let _ = first;
    1.0 - 0.75 * gap_factor * length_factor
}

fn ratio_difficulty(mut ratio: f64) -> f64 {
    if !ratio.is_normal() {
        ratio = 0.0;
    }
    let mut difficulty = 0.0;
    for denominator in 1..=8 {
        difficulty += -(denominator as f64 * std::f64::consts::PI * ratio)
            .cos()
            .powf(4.0);
    }
    difficulty += 8.0 / (1.0 + ratio);
    difficulty += bell_curve(ratio, 1.0, 0.5);
    difficulty -= bell_curve(ratio, 1.0, 0.3);
    difficulty.max(0.0) / 8.0_f64.sqrt()
}

fn evaluate_colour(
    index: usize,
    objects: &[DifficultyObject],
    colour: &ColourData,
    rhythm: &RhythmData,
) -> f64 {
    let data = &colour.per_object[index];
    let mut difficulty = 0.0;

    if let Some(streak_index) = data.mono_streak {
        let streak = &colour.streaks[streak_index];
        if streak.objects.first() == Some(&index) {
            let alternating = &colour.alternating_patterns[streak.parent_pattern];
            difficulty += logistic_exponent(
                std::f64::consts::E * streak.index as f64 - 2.0 * std::f64::consts::E,
            ) * evaluate_alternating_pattern(alternating, colour)
                * 0.5;
        }
    }

    if let Some(pattern_index) = data.alternating_pattern {
        let pattern = &colour.alternating_patterns[pattern_index];
        if colour.streaks[pattern.streaks[0]].objects.first() == Some(&index) {
            difficulty += evaluate_alternating_pattern(pattern, colour);
        }
    }

    if let Some(repeating_index) = data.repeating_pattern {
        let repeating = &colour.repeating_patterns[repeating_index];
        if colour.alternating_patterns[repeating.patterns[0]]
            .streaks
            .first()
            .map(|streak| colour.streaks[*streak].objects.first())
            .flatten()
            == Some(&index)
        {
            difficulty += evaluate_repeating_pattern(repeating);
        }
    }

    difficulty *= consistent_ratio_penalty(index, objects);
    let _ = rhythm;
    difficulty
}

fn evaluate_alternating_pattern(pattern: &AlternatingMonoPattern, colour: &ColourData) -> f64 {
    let repeating = &colour.repeating_patterns[pattern.parent_repeating];
    logistic_exponent(std::f64::consts::E * pattern.index as f64 - 2.0 * std::f64::consts::E)
        * evaluate_repeating_pattern(repeating)
}

fn evaluate_repeating_pattern(pattern: &RepeatingHitPattern) -> f64 {
    2.0 * (1.0
        - logistic_exponent(
            std::f64::consts::E * pattern.repetition_interval as f64 - 2.0 * std::f64::consts::E,
        ))
}

fn consistent_ratio_penalty(index: usize, objects: &[DifficultyObject]) -> f64 {
    let Some(previous_hit_object) = index.checked_sub(2) else {
        return 1.0;
    };
    let mut current_index = index;
    let mut consistent_ratio_count = 0;
    let mut total_ratio_count = 0.0;
    let mut recent_ratios = Vec::new();

    for _ in 0..64 {
        if current_index <= 1 {
            break;
        }

        let current_ratio = objects[current_index].rhythm_ratio;
        let previous_ratio = objects[previous_hit_object].rhythm_ratio;
        recent_ratios.push(current_ratio);

        if (1.0 - current_ratio / previous_ratio).abs() <= 0.01 {
            consistent_ratio_count += 1;
            total_ratio_count += current_ratio;
            break;
        }

        // Preserve the current C# preprocessor implementation's local `previousHitObject` behavior: the initial
        // previous object remains fixed while `current` is advanced to it. This is observable on irregular-colour
        // sequences and is kept for numeric parity with the pinned calculator version.
        current_index = previous_hit_object;
    }

    if consistent_ratio_count > 0 {
        return 1.0 - total_ratio_count / (consistent_ratio_count as f64 + 1.0) * 0.80;
    }
    if recent_ratios.len() <= 1 {
        return 1.0;
    }
    let average = recent_ratios.iter().sum::<f64>() / recent_ratios.len() as f64;
    let max_deviation = recent_ratios
        .iter()
        .map(|ratio| (ratio - average).abs())
        .fold(0.0, f64::max);
    0.7 + 0.3 * smootherstep(max_deviation, 0.0, 1.0)
}

fn evaluate_reading(object: &DifficultyObject) -> f64 {
    let effective_bpm = object.effective_bpm.max(1.0);
    let mid_velocity = 0.5 * logistic(effective_bpm, 420.0, 1.0 / 12.0, 1.0);
    let expected_delta_time = 21_000.0 / effective_bpm;
    let object_density = expected_delta_time / object.delta_time.max(1.0);
    let density_penalty = logistic(object_density, 0.925, 15.0, 1.0);
    let high_velocity = (1.0 - 0.33 * density_penalty)
        * logistic(
            effective_bpm,
            560.0 + 8.0 * density_penalty,
            (1.0 + 0.5 * density_penalty) / 16.0,
            1.0,
        );
    mid_velocity + high_velocity
}

fn combine_peaks(
    rhythm: &[f64],
    reading: &[f64],
    colour: &[f64],
    stamina: &[f64],
    pattern_multiplier: f64,
    strain_length_bonus: f64,
    is_convert: bool,
    relax: bool,
) -> Vec<f64> {
    let mut combined = Vec::with_capacity(colour.len());
    for index in 0..colour.len() {
        let rhythm_peak = rhythm[index] * RHYTHM_SKILL_MULTIPLIER * pattern_multiplier;
        let reading_peak = reading[index] * READING_SKILL_MULTIPLIER;
        let colour_peak = if relax {
            0.0
        } else {
            colour[index] * COLOUR_SKILL_MULTIPLIER
        };
        let mut stamina_peak = stamina[index] * STAMINA_SKILL_MULTIPLIER * strain_length_bonus;
        if is_convert || relax {
            stamina_peak /= 1.5;
        }
        let mechanical = norm(1.5, &[colour_peak, stamina_peak]);
        let peak = norm(2.0, &[mechanical, rhythm_peak, reading_peak]);
        if peak > 0.0 {
            combined.push(peak);
        }
    }
    combined
}

fn calculate_great_window(overall_difficulty: f64, clock_rate: f64) -> f64 {
    let raw = if overall_difficulty > 5.0 {
        35.0 + (20.0 - 35.0) * ((overall_difficulty - 5.0) / 5.0)
    } else if overall_difficulty < 5.0 {
        35.0 + (35.0 - 50.0) * ((overall_difficulty - 5.0) / 5.0)
    } else {
        35.0
    };
    let rounded = raw.floor() - 0.5;
    2.0 * rounded / clock_rate
}

fn rescale(star_rating: f64) -> f64 {
    if star_rating < 0.0 {
        star_rating
    } else {
        10.43 * (star_rating / 8.0 + 1.0).ln()
    }
}

fn logistic(x: f64, midpoint_offset: f64, multiplier: f64, max_value: f64) -> f64 {
    max_value / (1.0 + (multiplier * (midpoint_offset - x)).exp())
}

fn logistic_exponent(exponent: f64) -> f64 {
    1.0 / (1.0 + exponent.exp())
}

fn bell_curve(x: f64, mean: f64, width: f64) -> f64 {
    (std::f64::consts::E * -((x - mean).powi(2) / width.powi(2))).exp()
}

fn norm(p: f64, values: &[f64]) -> f64 {
    values
        .iter()
        .map(|value| value.powf(p))
        .sum::<f64>()
        .powf(1.0 / p)
}

fn reverse_lerp(value: f64, start: f64, end: f64) -> f64 {
    ((value - start) / (end - start)).clamp(0.0, 1.0)
}

fn smootherstep(value: f64, start: f64, end: f64) -> f64 {
    let value = ((value - start) / (end - start)).clamp(0.0, 1.0);
    value * value * value * (value * (value * 6.0 - 15.0) + 10.0)
}

fn almost_equals(left: f64, right: f64, tolerance: f64) -> bool {
    (left - right).abs() <= tolerance
}

fn colour_index(hit_type: TaikoHitType) -> usize {
    match hit_type {
        TaikoHitType::Centre => 0,
        TaikoHitType::Rim => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(time: f64, hit_type: TaikoHitType) -> TaikoNote {
        TaikoNote {
            start_time_ms: time,
            end_time_ms: time,
            effective_bpm: 120.0,
            object_type: TaikoObjectType::Hit,
            hit_type: Some(hit_type),
            is_strong: false,
            max_combo_contribution: 1,
        }
    }

    #[test]
    fn empty_chart_matches_empty_attribute_defaults() {
        let attributes = TaikoDifficultyCalculator::calculate(&[], &[], 5.0, false).unwrap();
        assert_eq!(attributes.star_rating, 0.0);
        assert_eq!(attributes.max_combo, 0);
        assert_eq!(attributes.mods, Vec::<Mod>::new());
        assert_eq!(attributes.consistency_factor, 0.0);
    }

    #[test]
    fn first_two_objects_are_baseline_only_and_max_combo_is_preserved() {
        let chart = [
            hit(0.0, TaikoHitType::Centre),
            hit(100.0, TaikoHitType::Rim),
            hit(200.0, TaikoHitType::Centre),
            hit(300.0, TaikoHitType::Rim),
        ];
        let two_hits = TaikoDifficultyCalculator::calculate(&chart[..2], &[], 5.0, false).unwrap();
        assert_eq!(two_hits.max_combo, 2);
        assert_eq!(two_hits.star_rating, 0.0);
        assert_eq!(two_hits.mono_stamina_factor, 1.0);

        // One difficulty object gives one section, no rhythm change and one base stamina increment (0.55).
        // The first mono streak starts all three colour groups; their combined colour increment is
        // 0.12 * 2 * (1 - logistic(15e)) * (1 + logistic(-2e) + 0.5 * logistic(-2e)²).
        let three_hits =
            TaikoDifficultyCalculator::calculate(&chart[..3], &[], 5.0, false).unwrap();
        assert_eq!(three_hits.max_combo, 3);
        assert_eq!(three_hits.consistency_factor, 1.0);
        assert!((three_hits.star_rating - 0.057_194_595_614_225_88).abs() < 1e-12);
    }

    #[test]
    fn double_time_increases_difficulty_and_records_mods() {
        let chart: Vec<_> = (0..64)
            .map(|index| {
                hit(
                    index as f64 * 180.0,
                    if index % 2 == 0 {
                        TaikoHitType::Centre
                    } else {
                        TaikoHitType::Rim
                    },
                )
            })
            .collect();
        let normal = TaikoDifficultyCalculator::calculate(&chart, &[], 5.0, false).unwrap();
        let double_time =
            TaikoDifficultyCalculator::calculate(&chart, &[Mod::DoubleTime], 5.0, false).unwrap();
        assert!(double_time.star_rating > normal.star_rating);
        assert_eq!(double_time.mods, vec![Mod::DoubleTime]);
    }

    #[test]
    fn all_four_skill_families_are_finite_and_components_sum_to_stars() {
        let chart: Vec<_> = (0..160)
            .map(|index| {
                let time = index as f64 * if index % 5 == 0 { 310.0 } else { 130.0 };
                hit(
                    time,
                    if index % 3 == 0 {
                        TaikoHitType::Rim
                    } else {
                        TaikoHitType::Centre
                    },
                )
            })
            .collect();
        let attributes = TaikoDifficultyCalculator::calculate(&chart, &[], 7.0, false).unwrap();

        for value in [
            attributes.star_rating,
            attributes.mechanical_difficulty,
            attributes.rhythm_difficulty,
            attributes.reading_difficulty,
            attributes.colour_difficulty,
            attributes.stamina_difficulty,
            attributes.mono_stamina_factor,
            attributes.consistency_factor,
            attributes.stamina_top_strains,
        ] {
            assert!(value.is_finite());
        }
        assert!(
            (attributes.mechanical_difficulty
                - attributes.colour_difficulty
                - attributes.stamina_difficulty)
                .abs()
                < 1e-12
        );
        assert!(
            (attributes.rhythm_difficulty
                + attributes.reading_difficulty
                + attributes.colour_difficulty
                + attributes.stamina_difficulty
                - attributes.star_rating)
                .abs()
                < 1e-10
        );
        assert!(attributes.colour_difficulty > 0.0);
        assert!(attributes.stamina_difficulty > 0.0);
        assert!(attributes.reading_difficulty > 0.0);
    }

    #[test]
    fn colour_strength_changes_colour_component_but_strong_flag_does_not_change_difficulty() {
        let mut chart: Vec<_> = (0..120)
            .map(|index| {
                hit(
                    index as f64 * 150.0,
                    if index % 8 < 4 {
                        TaikoHitType::Centre
                    } else {
                        TaikoHitType::Rim
                    },
                )
            })
            .collect();
        let base = TaikoDifficultyCalculator::calculate(&chart, &[], 5.0, false).unwrap();
        for note in &mut chart {
            note.is_strong = true;
        }
        let strong = TaikoDifficultyCalculator::calculate(&chart, &[], 5.0, false).unwrap();
        assert_eq!(base, strong);

        let mono: Vec<_> = (0..120)
            .map(|index| hit(index as f64 * 150.0, TaikoHitType::Centre))
            .collect();
        let mono_attributes = TaikoDifficultyCalculator::calculate(&mono, &[], 5.0, false).unwrap();
        assert_ne!(base.colour_difficulty, mono_attributes.colour_difficulty);
    }

    #[test]
    fn conversion_and_relax_adjust_mechanical_strain_policy() {
        let chart: Vec<_> = (0..96)
            .map(|index| {
                hit(
                    index as f64 * 110.0,
                    if index % 2 == 0 {
                        TaikoHitType::Centre
                    } else {
                        TaikoHitType::Rim
                    },
                )
            })
            .collect();
        let native = TaikoDifficultyCalculator::calculate(&chart, &[], 5.0, false).unwrap();
        let converted = TaikoDifficultyCalculator::calculate(&chart, &[], 5.0, true).unwrap();
        let relaxed =
            TaikoDifficultyCalculator::calculate(&chart, &[Mod::Relax], 5.0, false).unwrap();
        assert_ne!(native.star_rating, converted.star_rating);
        assert!(relaxed.star_rating < native.star_rating);
        // Relax excludes colour from combined peaks, but C# still exposes its proportional skill value.
        assert!(relaxed.colour_difficulty > 0.0);
    }

    #[test]
    fn taiko_windows_and_final_rescale_match_csharp_constants() {
        assert_eq!(calculate_great_window(5.0, 1.0), 69.0);
        assert_eq!(calculate_great_window(8.0, 1.5), 34.0);
        assert!((rescale(5.0_f64) - 10.43 * (5.0_f64 / 8.0 + 1.0).ln()).abs() < 1e-14);
    }

    #[test]
    fn drum_rolls_remain_in_timeline_but_are_not_hit_only_reading_or_stamina_events() {
        let chart = [
            hit(0.0, TaikoHitType::Centre),
            hit(100.0, TaikoHitType::Rim),
            TaikoNote {
                start_time_ms: 200.0,
                end_time_ms: 1200.0,
                effective_bpm: 120.0,
                object_type: TaikoObjectType::DrumRoll,
                hit_type: None,
                is_strong: false,
                max_combo_contribution: 1,
            },
            hit(1300.0, TaikoHitType::Centre),
        ];
        let attributes = TaikoDifficultyCalculator::calculate(&chart, &[], 5.0, false).unwrap();
        assert_eq!(attributes.max_combo, 4);
        assert!(attributes.star_rating.is_finite());
        assert!(attributes.reading_difficulty.is_finite());
    }

    #[test]
    fn rejects_invalid_converter_input() {
        assert_eq!(
            TaikoDifficultyCalculator::calculate(&[], &[Mod::RateAdjust { rate: 0.0 }], 5.0, false),
            Err(TaikoDifficultyError::InvalidClockRate)
        );
        assert_eq!(
            TaikoDifficultyCalculator::calculate(&[], &[], f64::NAN, false),
            Err(TaikoDifficultyError::InvalidOverallDifficulty)
        );

        let missing_type = TaikoNote {
            hit_type: None,
            ..hit(100.0, TaikoHitType::Centre)
        };
        assert!(matches!(
            TaikoDifficultyCalculator::calculate(
                &[
                    hit(0.0, TaikoHitType::Centre),
                    hit(50.0, TaikoHitType::Rim),
                    missing_type
                ],
                &[],
                5.0,
                false
            ),
            Err(TaikoDifficultyError::HitTypeMissing { object_index: 2 })
        ));

        let bad_order = TaikoNote {
            end_time_ms: 99.0,
            ..hit(100.0, TaikoHitType::Centre)
        };
        assert!(matches!(
            TaikoDifficultyCalculator::calculate(
                &[
                    hit(0.0, TaikoHitType::Centre),
                    hit(50.0, TaikoHitType::Rim),
                    bad_order
                ],
                &[],
                5.0,
                false
            ),
            Err(TaikoDifficultyError::EndBeforeStart { object_index: 2 })
        ));
    }
}
