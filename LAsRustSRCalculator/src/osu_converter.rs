//! Playable standard objects and preparation for osu! difficulty evaluators.

mod aim;
mod reading;
mod speed;

use crate::attributes::OsuDifficultyAttributes;
use crate::beatmap::{GameMode, OsuBeatmap};
use crate::calculator::{CalculationError, DifficultyCalculator};
use crate::mods::{Mod, calculate_rate_with_mods};
use crate::normalization::{NormalizedBeatmap, NormalizedHitObjectKind, normalize};
use crate::osu::{OsuBeatmapMetadata, OsuObjectDifficultyInput};
use crate::slider_events::{SliderEventGenerator, SliderEventType, TAIL_LENIENCY};
use crate::slider_path::{SliderPath, SliderPathOptions};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Circle,
    Slider,
    Spinner,
}

#[derive(Clone, Debug)]
pub struct PlayableObject {
    pub start_time: f64,
    pub end_time: f64,
    /// Playfield coordinates after stacking and positional mods.
    pub position: [f32; 2],
    pub end_position: [f32; 2],
    pub radius: f64,
    pub preempt: f64,
    pub fade_in: f64,
    pub kind: Kind,
    pub repeat_count: u32,
    great_window: f64,
    slider: Option<Slider>,
}

#[derive(Clone, Debug)]
struct Slider {
    path: SliderPath,
    span_duration: f64,
    nested: Vec<(SliderEventType, f64, [f32; 2])>,
}

#[derive(Clone, Copy)]
struct DiffObject<'a> {
    object: &'a PlayableObject,
    start_time: f64,
    delta: f64,
    adjusted_delta: f64,
    last_end_delta: f64,
    great_window: f64,
    min_jump_time: f64,
    jump: f64,
    lazy_jump: f64,
    angle: Option<f64>,
    vector_angle: Option<f64>,
    travel: f64,
    travel_time: f64,
    lazy_travel: f64,
    lazy_end: [f32; 2],
}

pub struct OsuBeatmapConverter;

impl OsuBeatmapConverter {
    pub fn convert(
        beatmap: &NormalizedBeatmap,
        stack_leniency: f32,
        mods: &[Mod],
    ) -> Result<Vec<PlayableObject>, CalculationError> {
        if !matches!(
            beatmap.mode,
            GameMode::Standard | GameMode::Catch | GameMode::Mania
        ) {
            return Err(CalculationError::WrongRuleset {
                expected: GameMode::Standard,
                actual: beatmap.mode,
            });
        }
        let mut ar = beatmap.difficulty.approach_rate as f32;
        let mut od = beatmap.difficulty.overall_difficulty as f32;
        let mut cs = beatmap.difficulty.circle_size as f32;
        let mut slider_multiplier = beatmap.difficulty.slider_multiplier;
        let mut tick_rate = beatmap.difficulty.slider_tick_rate;
        let mut hidden = false;
        let mut hard_rock = false;
        let mut mirror = false;
        for modifier in mods {
            match modifier {
                Mod::Easy => {
                    ar *= 0.5;
                    od *= 0.5;
                    cs *= 0.5;
                }
                Mod::HardRock => {
                    ar = (ar * 1.4).min(10.0);
                    od = (od * 1.4).min(10.0);
                    cs = (cs * 1.3).min(10.0);
                    hard_rock = true;
                }
                Mod::DifficultyAdjust(values) => {
                    ar = values.approach_rate.map_or(ar, |value| value as f32);
                    od = values.overall_difficulty.map_or(od, |value| value as f32);
                    cs = values.circle_size.map_or(cs, |value| value as f32);
                    slider_multiplier = values.slider_multiplier.unwrap_or(slider_multiplier);
                    tick_rate = values.slider_tick_rate.unwrap_or(tick_rate);
                }
                Mod::Hidden => hidden = true,
                Mod::Mirror => mirror = true,
                Mod::Other(_) | Mod::KeyCount { .. } | Mod::SplitStages => {
                    return Err(CalculationError::InvalidMod(modifier.acronym().to_owned()));
                }
                _ => {}
            }
        }
        if [
            f64::from(ar),
            f64::from(od),
            f64::from(cs),
            slider_multiplier,
            tick_rate,
            f64::from(stack_leniency),
        ]
        .iter()
        .any(|value| !value.is_finite())
            || slider_multiplier <= 0.0
            || tick_rate <= 0.0
        {
            return Err(CalculationError::InvalidMod(
                "invalid osu! difficulty settings".to_owned(),
            ));
        }
        let scale =
            ((1.0 - f64::from(0.7_f32) * ((f64::from(cs) - 5.0) / 5.0)) as f32 / 2.0) * 1.00041;
        if scale <= 0.0 {
            return Err(CalculationError::InvalidMod(
                "nonpositive circle scale".to_owned(),
            ));
        }
        let radius = f64::from(64.0 * scale);
        let preempt = if ar > 5.0 {
            (1200.0 - 750.0 * ((f64::from(ar) - 5.0) / 5.0)).trunc()
        } else {
            (1200.0 - 600.0 * ((f64::from(ar) - 5.0) / 5.0)).trunc()
        };
        let great_window = (50.0 - 30.0 * ((f64::from(od) - 5.0) / 5.0)).floor() - 0.5;
        let default_fade = 400.0 * (preempt / 450.0).min(1.0);
        let mut playable = Vec::with_capacity(beatmap.hit_objects.len());
        for object in &beatmap.hit_objects {
            let mut position = [object.position[0] as f32, object.position[1] as f32];
            if hard_rock {
                position[1] = 384.0 - position[1];
            }
            let (kind, end_time, end_position, repeats, slider) = match &object.kind {
                NormalizedHitObjectKind::Circle => {
                    (Kind::Circle, object.start_time, position, 0, None)
                }
                NormalizedHitObjectKind::Spinner { end_time }
                | NormalizedHitObjectKind::Hold { end_time } => {
                    (Kind::Spinner, *end_time, position, 0, None)
                }
                NormalizedHitObjectKind::Slider {
                    raw_curve,
                    path_control_points,
                    repeat_count,
                    pixel_length,
                    ..
                } => {
                    let mut points = path_control_points.clone();
                    if hard_rock {
                        for point in &mut points {
                            point[1] = -point[1];
                        }
                    }
                    let path = SliderPath::from_legacy_curve(
                        raw_curve,
                        &points,
                        *pixel_length,
                        beatmap.version,
                        SliderPathOptions {
                            optimise_catmull: true,
                        },
                    )
                    .map_err(|error| invalid_beatmap(error.to_string()))?;
                    let timing = &beatmap.control_points.timing;
                    let timing_index =
                        timing.partition_point(|point| point.time <= object.start_time);
                    let beat_length = timing
                        .get(timing_index.saturating_sub(1))
                        .map_or(1000.0, |point| point.beat_length);
                    let difficulty = &beatmap.control_points.difficulty;
                    let difficulty_index =
                        difficulty.partition_point(|point| point.time <= object.start_time);
                    let point = difficulty_index
                        .checked_sub(1)
                        .and_then(|index| difficulty.get(index));
                    let sv = point
                        .map_or(1.0, |point| point.speed_multiplier)
                        .clamp(0.1, 10.0);
                    let velocity = 100.0 * slider_multiplier
                        / (beat_length
                            * (f64::from((100.0 / sv) as f32).clamp(10.0, 1000.0) / 100.0));
                    if !velocity.is_finite() || velocity <= 0.0 {
                        return Err(invalid_beatmap("invalid osu! slider velocity"));
                    }
                    let tick_multiplier = if beatmap.version < 8 { 1.0 / sv } else { 1.0 };
                    let tick_distance = if point.is_none_or(|point| point.generate_ticks) {
                        velocity * beat_length / tick_rate * tick_multiplier
                    } else {
                        f64::INFINITY
                    };
                    let repeats = if path.distance().abs() <= 1e-7 {
                        0
                    } else {
                        *repeat_count
                    };
                    let spans = repeats + 1;
                    let duration = f64::from(spans) * path.distance() / velocity;
                    let span_duration = duration / f64::from(spans);
                    let events = if span_duration > 0.0 {
                        SliderEventGenerator::generate(
                            object.start_time,
                            span_duration,
                            velocity,
                            tick_distance,
                            path.distance(),
                            spans,
                        )
                    } else {
                        SliderEventGenerator::degenerate(object.start_time, spans).map(Vec::from)
                    }
                    .map_err(|error| invalid_beatmap(error.to_string()))?;
                    let mut nested = events
                        .into_iter()
                        .filter(|event| event.event_type != SliderEventType::LegacyLastTick)
                        .map(|event| {
                            let time = if event.event_type == SliderEventType::Repeat {
                                object.start_time + f64::from(event.span_index + 1) * span_duration
                            } else {
                                event.time
                            };
                            (
                                event.event_type,
                                time,
                                add(position, path.position_at(event.path_progress)),
                            )
                        })
                        .collect::<Vec<_>>();
                    nested.sort_by(|left, right| left.1.total_cmp(&right.1));
                    let end_position = if duration > 0.0 {
                        add(position, path.position_at(f64::from(spans % 2)))
                    } else {
                        position
                    };
                    let slider = Slider {
                        path,
                        span_duration,
                        nested,
                    };
                    (
                        Kind::Slider,
                        object.start_time + duration,
                        end_position,
                        repeats,
                        Some(slider),
                    )
                }
            };
            playable.push(PlayableObject {
                start_time: object.start_time,
                end_time,
                position,
                end_position,
                radius,
                preempt,
                fade_in: if hidden && kind != Kind::Slider {
                    preempt * 0.4
                } else {
                    default_fade
                },
                kind,
                repeat_count: repeats,
                great_window: if kind == Kind::Spinner {
                    0.0
                } else {
                    great_window
                },
                slider,
            });
        }
        if mirror {
            for object in &mut playable {
                object.position[0] = 512.0 - object.position[0];
                object.end_position[0] = 512.0 - object.end_position[0];
                if let Some(slider) = &mut object.slider {
                    slider.path.reflect_horizontally();
                    for (_, _, position) in &mut slider.nested {
                        position[0] = 512.0 - position[0];
                    }
                }
            }
        }
        apply_stacking(&mut playable, scale, stack_leniency, beatmap.version);
        Ok(playable)
    }
}

fn apply_stacking(objects: &mut [PlayableObject], scale: f32, leniency: f32, version: u32) {
    let mut heights = vec![0i32; objects.len()];
    if version >= 6 {
        for i in (1..objects.len()).rev() {
            if heights[i] != 0 || objects[i].kind == Kind::Spinner {
                continue;
            }
            let mut current = i;
            let threshold = objects[i].preempt as i32 as f32 * leniency;
            for previous in (0..i).rev() {
                if objects[previous].kind == Kind::Spinner {
                    continue;
                }
                if objects[i].kind == Kind::Circle {
                    if (objects[current].start_time as i32)
                        .saturating_sub(objects[previous].end_time as i32)
                        as f32
                        > threshold
                    {
                        break;
                    }
                    if objects[previous].kind == Kind::Slider
                        && distance(objects[previous].end_position, objects[current].position) < 3.0
                    {
                        let offset = heights[current] - heights[previous] + 1;
                        for stacked in previous + 1..=i {
                            if distance(objects[previous].end_position, objects[stacked].position)
                                < 3.0
                            {
                                heights[stacked] -= offset;
                            }
                        }
                        break;
                    }
                    if distance(objects[previous].position, objects[current].position) < 3.0 {
                        heights[previous] = heights[current] + 1;
                        current = previous;
                    }
                } else {
                    if objects[current].start_time - objects[previous].start_time
                        > f64::from(threshold)
                    {
                        break;
                    }
                    if distance(objects[previous].end_position, objects[current].position) < 3.0 {
                        heights[previous] = heights[current] + 1;
                        current = previous;
                    }
                }
            }
        }
    } else {
        for i in 0..objects.len() {
            if heights[i] != 0 && objects[i].kind != Kind::Slider {
                continue;
            }
            let mut start_time = objects[i].end_time;
            let mut slider_stack = 0;
            let threshold = objects[i].preempt as i32 as f32 * leniency;
            for j in i + 1..objects.len() {
                if objects[j].start_time - f64::from(threshold) > start_time {
                    break;
                }
                let path_end = objects[i]
                    .slider
                    .as_ref()
                    .map_or(objects[i].position, |slider| {
                        add(objects[i].position, slider.path.position_at(1.0))
                    });
                if distance(objects[j].position, objects[i].position) < 3.0 {
                    heights[i] += 1;
                    start_time = objects[j].start_time;
                } else if distance(objects[j].position, path_end) < 3.0 {
                    slider_stack += 1;
                    heights[j] -= slider_stack;
                    start_time = objects[j].start_time;
                }
            }
        }
    }
    for (object, height) in objects.iter_mut().zip(heights) {
        if object.kind == Kind::Spinner {
            continue;
        }
        let offset = height as f32 * scale * -6.4;
        object.position = add(object.position, [offset; 2]);
        object.end_position = add(object.end_position, [offset; 2]);
        if let Some(slider) = &mut object.slider {
            for (_, _, position) in &mut slider.nested {
                *position = add(*position, [offset; 2]);
            }
        }
    }
}

fn preprocess(objects: &[PlayableObject], rate: f64) -> Vec<DiffObject<'_>> {
    let mut output: Vec<DiffObject<'_>> = Vec::with_capacity(objects.len().saturating_sub(1));
    for pair in objects.windows(2) {
        let [previous, current] = [&pair[0], &pair[1]];
        let start_time = current.start_time / rate;
        let delta = (current.start_time - previous.start_time) / rate;
        let adjusted_delta = delta.max(25.0);
        let last_end_delta = output.last().map_or(adjusted_delta, |previous| {
            (start_time - previous.object.end_time / rate).max(25.0)
        });
        let (lazy_travel, lazy_time, lazy_end) = lazy_slider(current);
        let travel = if current.kind == Kind::Slider {
            lazy_travel * f64::from(current.repeat_count).powf(0.3).max(1.0)
        } else {
            0.0
        };
        let travel_time = if current.kind == Kind::Slider {
            (lazy_time / rate).max(25.0)
        } else {
            0.0
        };
        let mut object = DiffObject {
            object: current,
            start_time,
            delta,
            adjusted_delta,
            last_end_delta,
            great_window: 2.0 * current.great_window / rate,
            min_jump_time: adjusted_delta,
            jump: 0.0,
            lazy_jump: 0.0,
            angle: None,
            vector_angle: None,
            travel,
            travel_time,
            lazy_travel,
            lazy_end,
        };
        if current.kind != Kind::Spinner && previous.kind != Kind::Spinner {
            let scaling = 50.0 / current.radius as f32;
            let prev = output.last();
            let last_cursor = prev.map_or(previous.position, |previous| previous.lazy_end);
            object.jump = f64::from(distance(previous.position, current.position) * scaling);
            object.lazy_jump = f64::from(distance(current.position, last_cursor) * scaling);
            if let Some(previous) = prev.filter(|previous| previous.object.kind == Kind::Slider) {
                object.min_jump_time = (adjusted_delta - previous.travel_time).max(25.0);
            }
            if let [.., lastlast, last] = output.as_slice()
                && lastlast.object.kind != Kind::Spinner
            {
                let mut last_aim = last_cursor;
                let mut previous_angle_start = lastlast.lazy_end;
                if let Some(slider) = &last.object.slider
                    && last.travel > 0.0
                {
                    last_aim = last.object.position;
                    previous_angle_start = slider.nested[slider.nested.len() - 2].2;
                }
                let first = angle(current.position, last_aim, lastlast.lazy_end);
                let second = angle(current.position, last_cursor, previous_angle_start);
                let vector = sub(current.position, last_aim);
                object.vector_angle =
                    Some(f64::from(vector[1].abs()).atan2(f64::from(vector[0].abs())));
                object.angle = Some(first.min(second));
            }
        }
        output.push(object);
    }
    output
}

fn lazy_slider(object: &PlayableObject) -> (f64, f64, [f32; 2]) {
    let Some(slider) = &object.slider else {
        return (0.0, 0.0, object.position);
    };
    let duration = object.end_time - object.start_time;
    if slider.span_duration == 0.0 {
        return (0.0, 0.0, object.position);
    }
    let mut tracking_end =
        (object.start_time + duration + TAIL_LENIENCY).max(object.start_time + duration / 2.0);
    let last_tick = slider
        .nested
        .iter()
        .rposition(|entry| entry.0 == SliderEventType::Tick);
    let mut nested = slider.nested.clone();
    if let Some(index) = last_tick.filter(|&index| nested[index].1 > tracking_end) {
        let tick = nested.remove(index);
        tracking_end = tick.1;
        nested.push(tick);
    }
    let time = tracking_end - object.start_time;
    let mut progress = time / slider.span_duration;
    progress = if progress % 2.0 >= 1.0 {
        1.0 - progress % 1.0
    } else {
        progress % 1.0
    };
    let target = add(object.position, slider.path.position_at(progress));
    let mut cursor = object.position;
    let mut travel = 0.0;
    let scaling = 50.0 / object.radius;
    for (index, (kind, _, position)) in nested.iter().enumerate().skip(1) {
        let mut movement = sub(*position, cursor);
        let mut threshold = 90.0;
        if index == nested.len() - 1 {
            let lazy = sub(target, cursor);
            if distance(lazy, [0.0; 2]) < distance(movement, [0.0; 2]) {
                movement = lazy;
            }
        } else if *kind == SliderEventType::Repeat {
            threshold = 50.0;
        }
        let length = scaling * f64::from(distance(movement, [0.0; 2]));
        if length > threshold {
            let fraction = (length - threshold) / length;
            cursor = add(
                cursor,
                [movement[0] * fraction as f32, movement[1] * fraction as f32],
            );
            travel += length * fraction;
        }
    }
    (travel, time, cursor)
}

fn attributes_metadata(
    source: &NormalizedBeatmap,
    playable: &[PlayableObject],
) -> OsuBeatmapMetadata {
    let mut metadata = OsuBeatmapMetadata::default();
    let mut nested_total = 0.0;
    let start = source
        .hit_objects
        .first()
        .map_or(0, |object| object.start_time.round_ties_even() as i64);
    let end = source
        .hit_objects
        .last()
        .map_or(0, |object| object.start_time.round_ties_even() as i64);
    let breaks = source
        .breaks
        .iter()
        .map(|period| {
            period.end_time.round_ties_even() as i64 - period.start_time.round_ties_even() as i64
        })
        .sum::<i64>();
    let drain = (end - start - breaks) / 1000;
    let density = if drain == 0 {
        16.0
    } else {
        (8.0 * playable.len() as f64 / drain as f64).clamp(0.0, 16.0)
    };
    metadata.legacy_score_base_multiplier = ((f64::from(source.difficulty.drain_rate as f32)
        + f64::from(source.difficulty.overall_difficulty as f32)
        + f64::from(source.difficulty.circle_size as f32)
        + density)
        / 38.0
        * 5.0)
        .round_ties_even();

    for object in playable {
        match object.kind {
            Kind::Slider => {
                metadata.slider_count += 1;
                let nested = &object.slider.as_ref().expect("slider geometry").nested;
                metadata.max_combo += nested.len() as i32;
                nested_total += 30.0 * f64::from(object.repeat_count + 2)
                    + 10.0
                        * nested
                            .iter()
                            .filter(|entry| entry.0 == SliderEventType::Tick)
                            .count() as f64;
            }
            Kind::Circle => metadata.hit_circle_count += 1,
            Kind::Spinner => {
                metadata.spinner_count += 1;
                let duration = (object.end_time - object.start_time) / 1000.0;
                let half_spins = (duration * (477.0 / 60.0) * 2.0) as i64;
                let completion = (duration * 3.0) as i64 + 3;
                let full_spins = half_spins / 2;
                let bonus_spins = ((half_spins - completion) / 2 - full_spins / 2).max(0);
                nested_total += 100.0 * full_spins as f64 + 1000.0 * bonus_spins as f64;
            }
        }
        metadata.maximum_legacy_combo_score += (f64::from((metadata.max_combo - 1).max(0))
            * (12.0 * metadata.legacy_score_base_multiplier))
            .trunc();
        if object.kind != Kind::Slider {
            metadata.max_combo += 1;
        }
    }
    if !playable.is_empty() {
        metadata.nested_score_per_object = nested_total / playable.len() as f64;
    }
    metadata
}

pub struct OsuDifficultyCalculator;

impl OsuDifficultyCalculator {
    pub const VERSION: u32 = crate::osu::OsuDifficultyCalculator::VERSION;

    pub fn calculate(
        &self,
        beatmap: &OsuBeatmap,
        mods: &[Mod],
    ) -> Result<OsuDifficultyAttributes, CalculationError> {
        let stack_leniency = beatmap.general.stack_leniency.unwrap_or(0.7) as f32;
        let rate = calculate_rate_with_mods(mods)
            .map_err(|error| CalculationError::InvalidMod(error.to_string()))?;
        let normalized = normalize(beatmap).map_err(|error| invalid_beatmap(error.to_string()))?;
        let playable = OsuBeatmapConverter::convert(&normalized, stack_leniency, mods)?;
        if playable.is_empty() {
            return Ok(OsuDifficultyAttributes {
                mods: mods.to_vec(),
                ..OsuDifficultyAttributes::default()
            });
        }
        let hidden = mods.contains(&Mod::Hidden);
        let flashlight = mods.contains(&Mod::Flashlight);
        let objects = preprocess(&playable, rate);
        let input: Vec<_> = objects
            .iter()
            .enumerate()
            .map(|(index, object)| {
                let small_circle_bonus = (1.0 + (30.0 - object.object.radius) / 70.0).max(1.0);
                OsuObjectDifficultyInput {
                    start_time_ms: object.start_time,
                    previous_start_time_ms: if index == 0 {
                        playable[0].start_time / rate
                    } else {
                        objects[index - 1].start_time
                    },
                    delta_time_ms: Some(object.delta),
                    adjusted_delta_time_ms: object.adjusted_delta,
                    last_object_end_delta_time_ms: object.last_end_delta,
                    overall_difficulty: (79.5 - object.great_window * 0.5) / 6.0,
                    hit_window_great_ms: object.great_window,
                    preempt_ms: object.object.preempt / rate,
                    small_circle_bonus,
                    is_slider: object.object.kind == Kind::Slider,
                    is_spinner: object.object.kind == Kind::Spinner,
                    aim_snap_with_slider: aim::snap(&objects, index, true),
                    aim_snap_without_slider: aim::snap(&objects, index, false),
                    aim_flow_with_slider: aim::flow(&objects, index, true),
                    aim_flow_without_slider: aim::flow(&objects, index, false),
                    aim_agility: aim::agility(&objects, index),
                    speed_evaluator: speed::speed(&objects, index),
                    speed_rhythm_multiplier: speed::rhythm(&objects, index),
                    reading_evaluator: reading::reading(&objects, index, rate, hidden),
                    flashlight_evaluator: if flashlight {
                        reading::flashlight(&objects, index, hidden)
                    } else {
                        0.0
                    },
                    slider_travel_distance: object.travel,
                    slider_travel_time_ms: object.travel_time,
                    has_slider_geometry_context: object.object.slider.is_some(),
                    ..Default::default()
                }
            })
            .collect();
        crate::osu::OsuDifficultyCalculator::calculate(
            &input,
            attributes_metadata(&normalized, &playable),
            mods,
            rate,
        )
        .map_err(|error| invalid_beatmap(error.to_string()))
    }
}

impl DifficultyCalculator for OsuDifficultyCalculator {
    type Attributes = OsuDifficultyAttributes;

    fn version(&self) -> u32 {
        Self::VERSION
    }

    fn calculate(
        &self,
        beatmap: &OsuBeatmap,
        mods: &[Mod],
    ) -> Result<Self::Attributes, CalculationError> {
        OsuDifficultyCalculator::calculate(self, beatmap, mods)
    }
}

fn invalid_beatmap(message: impl Into<String>) -> CalculationError {
    CalculationError::InvalidBeatmap(message.into())
}

fn add(left: [f32; 2], right: [f32; 2]) -> [f32; 2] {
    [left[0] + right[0], left[1] + right[1]]
}

fn sub(left: [f32; 2], right: [f32; 2]) -> [f32; 2] {
    [left[0] - right[0], left[1] - right[1]]
}

fn distance(left: [f32; 2], right: [f32; 2]) -> f32 {
    let [x, y] = sub(left, right);
    (x * x + y * y).sqrt()
}

fn angle(current: [f32; 2], previous: [f32; 2], earliest: [f32; 2]) -> f64 {
    let [a, b] = sub(earliest, previous);
    let [c, d] = sub(current, previous);
    f64::from(a * d - b * c)
        .atan2(f64::from(a * c + b * d))
        .abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_reflects_slider_geometry_and_preserves_difficulty() {
        let beatmap = OsuBeatmap::parse_bytes(
            b"osu file format v14\n[General]\nMode:0\nStackLeniency:0.7\n[Difficulty]\nHPDrainRate:5\nCircleSize:4\nOverallDifficulty:5\nApproachRate:5\nSliderMultiplier:1.4\nSliderTickRate:1\n[TimingPoints]\n0,500,4,2,1,50,1,0\n[HitObjects]\n80,80,0,1,0,0:0:0:0:\n100,100,1000,2,0,L|300:100,1,200\n400,300,2000,1,0,0:0:0:0:\n256,192,3000,2,0,B|300:240|200:160,1,200\n",
        )
        .unwrap();
        let normalized = normalize(&beatmap).unwrap();
        let stack_leniency = beatmap.general.stack_leniency.unwrap_or(0.7) as f32;
        let ordinary = OsuBeatmapConverter::convert(&normalized, stack_leniency, &[]).unwrap();
        let mirrored =
            OsuBeatmapConverter::convert(&normalized, stack_leniency, &[Mod::Mirror]).unwrap();

        assert_eq!(ordinary.len(), mirrored.len());
        let mut slider_count = 0;
        for (source, reflected) in ordinary.iter().zip(&mirrored) {
            assert!((source.position[0] + reflected.position[0] - 512.0).abs() < 1e-5);
            assert_eq!(source.position[1], reflected.position[1]);
            assert!((source.end_position[0] + reflected.end_position[0] - 512.0).abs() < 1e-5);
            assert_eq!(source.end_position[1], reflected.end_position[1]);
            assert_eq!(source.kind, reflected.kind);
            assert_eq!(source.start_time, reflected.start_time);
            assert_eq!(source.end_time, reflected.end_time);

            match (&source.slider, &reflected.slider) {
                (Some(source_slider), Some(reflected_slider)) => {
                    slider_count += 1;
                    assert_eq!(
                        source_slider.path.distance(),
                        reflected_slider.path.distance()
                    );
                    assert_eq!(
                        source_slider.path.vertices().len(),
                        reflected_slider.path.vertices().len()
                    );
                    for (source_vertex, reflected_vertex) in source_slider
                        .path
                        .vertices()
                        .iter()
                        .zip(reflected_slider.path.vertices())
                    {
                        assert_eq!(source_vertex[0], -reflected_vertex[0]);
                        assert_eq!(source_vertex[1], reflected_vertex[1]);
                    }
                    assert_eq!(source_slider.nested.len(), reflected_slider.nested.len());
                    for (source_event, reflected_event) in
                        source_slider.nested.iter().zip(&reflected_slider.nested)
                    {
                        assert_eq!(source_event.0, reflected_event.0);
                        assert_eq!(source_event.1, reflected_event.1);
                        assert!((source_event.2[0] + reflected_event.2[0] - 512.0).abs() < 1e-5);
                        assert_eq!(source_event.2[1], reflected_event.2[1]);
                    }
                }
                (None, None) => {}
                _ => panic!("mirror must preserve slider geometry availability"),
            }
        }
        assert!(slider_count > 0, "the fixture must exercise slider paths");

        let ordinary_difficulty = OsuDifficultyCalculator.calculate(&beatmap, &[]).unwrap();
        let mirrored_difficulty = OsuDifficultyCalculator
            .calculate(&beatmap, &[Mod::Mirror])
            .unwrap();
        let relative_star_error =
            (ordinary_difficulty.star_rating - mirrored_difficulty.star_rating).abs()
                / ordinary_difficulty.star_rating.max(1.0);
        assert!(
            relative_star_error < 2e-5,
            "mirror star rating drift: {relative_star_error}"
        );
        assert_eq!(ordinary_difficulty.max_combo, mirrored_difficulty.max_combo);
    }

    #[test]
    fn catch_and_mania_sources_convert_to_positioned_osu_objects() {
        for fixture in [
            &include_bytes!("../tests/fixtures/catch-diffcalc-test.osu")[..],
            &include_bytes!("../tests/fixtures/mania-diffcalc-test.osu")[..],
        ] {
            let beatmap = OsuBeatmap::parse_bytes(fixture).unwrap();
            assert!(matches!(beatmap.mode, GameMode::Catch | GameMode::Mania));
            let normalized = normalize(&beatmap).unwrap();
            let playable = OsuBeatmapConverter::convert(
                &normalized,
                beatmap.general.stack_leniency.unwrap_or(0.7) as f32,
                &[],
            )
            .unwrap();

            assert_eq!(playable.len(), beatmap.hit_objects.len());
            for (source, converted) in normalized.hit_objects.iter().zip(&playable) {
                let expected_kind = match &source.kind {
                    NormalizedHitObjectKind::Circle => Kind::Circle,
                    NormalizedHitObjectKind::Slider { .. } => Kind::Slider,
                    NormalizedHitObjectKind::Spinner { .. }
                    | NormalizedHitObjectKind::Hold { .. } => Kind::Spinner,
                };
                assert_eq!(converted.kind, expected_kind);
            }
            assert!(playable.iter().all(|object| {
                object
                    .position
                    .iter()
                    .all(|component| component.is_finite())
                    && object
                        .end_position
                        .iter()
                        .all(|component| component.is_finite())
                    && object.start_time.is_finite()
                    && object.end_time.is_finite()
            }));

            let difficulty = OsuDifficultyCalculator.calculate(&beatmap, &[]).unwrap();
            assert!(difficulty.star_rating.is_finite());
            assert!(difficulty.max_combo as usize >= beatmap.hit_objects.len());
        }
    }

    #[test]
    fn taiko_sources_are_not_convertible_to_osu_position_objects() {
        let beatmap = OsuBeatmap::parse_bytes(
            b"osu file format v14\n[General]\nMode:1\n[Difficulty]\nCircleSize:4\n[HitObjects]\n64,192,1000,1,0\n",
        )
        .unwrap();
        assert!(matches!(
            OsuDifficultyCalculator.calculate(&beatmap, &[]),
            Err(CalculationError::WrongRuleset {
                expected: GameMode::Standard,
                actual: GameMode::Taiko,
            })
        ));
    }

    #[test]
    fn standard_reference_difficulty() {
        let fixture = |bytes: &[u8], mods: &[Mod], expected: f64, combo: i32| {
            let beatmap = OsuBeatmap::parse_bytes(bytes).unwrap();
            let result = OsuDifficultyCalculator.calculate(&beatmap, mods).unwrap();
            assert_eq!(result.max_combo, combo);
            let error = (result.star_rating - expected).abs() / expected;
            assert!(
                error <= 2e-5,
                "{mods:?}: {:?}, expected {expected}, relative error {error:e}",
                result
            );
        };
        fixture(
            include_bytes!("../tests/fixtures/osu-diffcalc-test.osu"),
            &[],
            6.5206729127668259,
            239,
        );
        fixture(
            include_bytes!("../tests/fixtures/osu-diffcalc-test.osu"),
            &[Mod::DoubleTime],
            9.4653010383617922,
            239,
        );
        fixture(
            include_bytes!("../tests/fixtures/osu-diffcalc-test.osu"),
            &[Mod::Classic],
            6.5206729127668259,
            239,
        );
        fixture(
            include_bytes!("../tests/fixtures/osu-zero-length-sliders.osu"),
            &[],
            1.3280410795791415,
            54,
        );
        fixture(
            include_bytes!("../tests/fixtures/osu-zero-length-sliders.osu"),
            &[Mod::DoubleTime],
            1.6856612715618886,
            54,
        );
        fixture(
            include_bytes!("../tests/fixtures/osu-very-fast-slider.osu"),
            &[],
            0.40867325147697559,
            4,
        );
        fixture(
            include_bytes!("../tests/fixtures/osu-very-fast-slider.osu"),
            &[Mod::DoubleTime],
            0.53588473186572561,
            4,
        );
        fixture(
            include_bytes!("../tests/fixtures/osu-nan-slider.osu"),
            &[],
            0.87058175794353554,
            6,
        );
        fixture(
            include_bytes!("../tests/fixtures/osu-801165.osu"),
            &[],
            6.3007396189176985,
            2359,
        );
    }
}
