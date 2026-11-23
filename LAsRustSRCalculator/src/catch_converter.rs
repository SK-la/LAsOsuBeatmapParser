//! Native Catch conversion and the ruleset difficulty entry point.

use crate::attributes::CatchDifficultyAttributes;
use crate::beatmap::{GameMode, OsuBeatmap};
use crate::calculator::{CalculationError, DifficultyCalculator};
use crate::catch::{CatchObjectInput, calculate_catch_difficulty};
use crate::mods::{Mod, calculate_rate_with_mods};
use crate::normalization::{NormalizedBeatmap, NormalizedHitObjectKind, normalize};
use crate::slider_events::{SliderEventDescriptor, SliderEventGenerator, SliderEventType};
use crate::slider_path::{SliderPath, SliderPathOptions};

const BASE_CATCHER_SIZE: f32 = 106.75;
const ALLOWED_CATCH_RANGE: f32 = 0.8;

pub struct CatchBeatmapConverter;

impl CatchBeatmapConverter {
    pub fn convert(
        beatmap: &NormalizedBeatmap,
        mods: &[Mod],
    ) -> Result<(Vec<CatchObjectInput>, i32, f32), CalculationError> {
        if !matches!(
            beatmap.mode,
            GameMode::Catch | GameMode::Mania | GameMode::Standard
        ) {
            return Err(CalculationError::WrongRuleset {
                expected: GameMode::Catch,
                actual: beatmap.mode,
            });
        }

        if let Some(id) = mods.iter().find_map(|modifier| match modifier {
            Mod::Other(id) => Some(id.clone()),
            Mod::KeyCount { .. } | Mod::SplitStages => Some(modifier.acronym().to_owned()),
            _ => None,
        }) {
            return Err(CalculationError::InvalidMod(format!(
                "Catch conversion mod {id} is not implemented"
            )));
        }

        let difficulty = adjusted_difficulty(beatmap, mods)?;
        let half_width = catch_half_width(difficulty.circle_size);
        let mut objects = Vec::with_capacity(beatmap.hit_objects.len());
        let mut max_combo = 0i32;
        let use_hr_offsets = mods
            .iter()
            .any(|modifier| matches!(modifier, Mod::HardRock));
        let mirror = mods.iter().any(|modifier| matches!(modifier, Mod::Mirror));
        let mut rng = LegacyRandom::new(1337);
        let mut last_hr_position = None;
        let mut last_hr_time = 0.0;

        for source in &beatmap.hit_objects {
            match &source.kind {
                NormalizedHitObjectKind::Circle => {
                    let original_x = source.position[0] as f32;
                    let position = if use_hr_offsets {
                        apply_hard_rock_offset(
                            original_x,
                            source.start_time,
                            &mut last_hr_position,
                            &mut last_hr_time,
                            &mut rng,
                        )
                    } else {
                        original_x
                    };
                    objects.push(CatchObjectInput {
                        start_time_ms: source.start_time,
                        effective_x: effective_x(position, mirror),
                        hyper_dash: false,
                        distance_to_hyper_dash: 0.0,
                    });
                    max_combo += 1;
                }
                NormalizedHitObjectKind::Hold { end_time } => {
                    // A duration object becomes a BananaShower; its bananas do not contribute to combo.
                    if use_hr_offsets {
                        advance_banana_random(source.start_time, *end_time, &mut rng)?;
                    }
                }
                NormalizedHitObjectKind::Spinner { end_time } => {
                    if use_hr_offsets {
                        advance_banana_random(source.start_time, *end_time, &mut rng)?;
                    }
                }
                NormalizedHitObjectKind::Slider {
                    raw_curve,
                    path_control_points,
                    repeat_count,
                    pixel_length,
                    ..
                } => {
                    let path = SliderPath::from_legacy_curve(
                        raw_curve,
                        path_control_points,
                        *pixel_length,
                        beatmap.version,
                        SliderPathOptions::default(),
                    )
                    .map_err(|error| CalculationError::InvalidBeatmap(error.to_string()))?;
                    let sv = speed_multiplier_at(beatmap, source.start_time);
                    let timing = timing_point_at(beatmap, source.start_time);
                    let adjusted_beat = precision_adjusted_beat_length(timing, sv);
                    let velocity = 100.0 * difficulty.slider_multiplier / adjusted_beat;
                    if !velocity.is_finite() || velocity <= 0.0 {
                        return Err(CalculationError::InvalidBeatmap(
                            "Catch slider has invalid timing or velocity".to_owned(),
                        ));
                    }

                    let spans = if path.distance().abs() <= 1e-7 {
                        1
                    } else {
                        repeat_count.saturating_add(1)
                    };
                    let duration = spans as f64 * path.distance() / velocity;
                    let span_duration = duration / spans as f64;
                    let scoring_distance = velocity * timing;
                    let tick_multiplier = if beatmap.version < 8 { 1.0 / sv } else { 1.0 };
                    let tick_distance =
                        scoring_distance / difficulty.slider_tick_rate * tick_multiplier;
                    let events = SliderEventGenerator::generate(
                        source.start_time,
                        span_duration,
                        velocity,
                        tick_distance,
                        path.distance(),
                        spans,
                    )
                    .map_err(|error| CalculationError::InvalidBeatmap(error.to_string()))?;
                    if use_hr_offsets {
                        last_hr_position = Some(
                            source.position[0] as f32
                                + path_control_points
                                    .last()
                                    .map_or(0.0, |point| point[0] as f32),
                        );
                        last_hr_time = source.start_time;
                    }
                    let mut previous_event: Option<SliderEventDescriptor> = None;
                    for event in events {
                        if let Some(previous) = previous_event {
                            let elapsed = event.time.trunc() - previous.time.trunc();
                            if elapsed > 80.0 {
                                let mut spacing = elapsed;
                                while spacing > 100.0 {
                                    spacing /= 2.0;
                                }
                                let mut time = spacing;
                                while time < elapsed {
                                    let progress = previous.path_progress
                                        + time / elapsed
                                            * (event.path_progress - previous.path_progress);
                                    let x =
                                        source.position[0] as f32 + path.position_at(progress)[0];
                                    let _tiny_droplet_x = effective_x(x, mirror);
                                    if use_hr_offsets {
                                        rng.next_range(-20, 20);
                                    }
                                    time += spacing;
                                }
                            }
                        }
                        previous_event = Some(event);

                        match event.event_type {
                            SliderEventType::Head
                            | SliderEventType::Tail
                            | SliderEventType::Repeat
                            | SliderEventType::Tick => {
                                let x = source.position[0] as f32
                                    + path.position_at(event.path_progress)[0];
                                objects.push(CatchObjectInput {
                                    start_time_ms: event.time,
                                    effective_x: effective_x(x, mirror),
                                    hyper_dash: false,
                                    distance_to_hyper_dash: 0.0,
                                });
                                max_combo += 1;
                                if use_hr_offsets && event.event_type == SliderEventType::Tick {
                                    rng.next_int();
                                }
                            }
                            SliderEventType::LegacyLastTick => {}
                        }
                    }
                }
            }
        }

        objects.sort_by(|a, b| a.start_time_ms.total_cmp(&b.start_time_ms));
        // Hyperdash preprocessing uses the full catch width, while movement difficulty uses the playable half-width.
        calculate_hyper_dashes(&mut objects, half_width / ALLOWED_CATCH_RANGE);
        Ok((objects, max_combo, half_width))
    }
}

pub struct CatchDifficultyCalculator;

impl CatchDifficultyCalculator {
    pub const VERSION: u32 = 20260706;

    pub fn calculate(
        &self,
        beatmap: &OsuBeatmap,
        mods: &[Mod],
    ) -> Result<CatchDifficultyAttributes, CalculationError> {
        let normalized = normalize(beatmap)
            .map_err(|error| CalculationError::InvalidBeatmap(error.to_string()))?;
        let (objects, max_combo, half_width) = CatchBeatmapConverter::convert(&normalized, mods)?;
        let rate = calculate_rate_with_mods(mods)
            .map_err(|error| CalculationError::InvalidMod(error.to_string()))?;
        calculate_catch_difficulty(&objects, half_width, rate, max_combo, mods)
            .map_err(|error| CalculationError::InvalidBeatmap(error.to_string()))
    }
}

impl DifficultyCalculator for CatchDifficultyCalculator {
    type Attributes = CatchDifficultyAttributes;

    fn version(&self) -> u32 {
        Self::VERSION
    }

    fn calculate(
        &self,
        beatmap: &OsuBeatmap,
        mods: &[Mod],
    ) -> Result<Self::Attributes, CalculationError> {
        CatchDifficultyCalculator::calculate(self, beatmap, mods)
    }
}

fn adjusted_difficulty(
    beatmap: &NormalizedBeatmap,
    mods: &[Mod],
) -> Result<crate::normalization::NormalizedDifficulty, CalculationError> {
    let mut difficulty = beatmap.difficulty;
    let mut drain = difficulty.drain_rate;
    let mut circle = difficulty.circle_size;
    let mut od = difficulty.overall_difficulty;
    let mut ar = difficulty.approach_rate;

    for modifier in mods {
        match modifier {
            Mod::Easy => {
                drain = ((drain as f32) * 0.5) as f64;
                circle = ((circle as f32) * 0.5) as f64;
                ar = ((ar as f32) * 0.5) as f64;
                od = ((od as f32) * 0.5) as f64;
            }
            Mod::HardRock => {
                drain = (((drain as f32) * 1.4).min(10.0)) as f64;
                od = (((od as f32) * 1.4).min(10.0)) as f64;
                circle = (((circle as f32) * 1.3).min(10.0)) as f64;
                ar = (((ar as f32) * 1.4).min(10.0)) as f64;
            }
            Mod::DifficultyAdjust(settings) => {
                if let Some(value) = settings.hp_drain_rate {
                    drain = value as f32 as f64;
                }
                if let Some(value) = settings.circle_size {
                    circle = value as f32 as f64;
                }
                if let Some(value) = settings.overall_difficulty {
                    od = value as f32 as f64;
                }
                if let Some(value) = settings.approach_rate {
                    ar = value as f32 as f64;
                }
            }
            _ => {}
        }
    }

    if [drain, circle, od, ar]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err(CalculationError::InvalidMod(
            "Catch difficulty settings must be finite".to_owned(),
        ));
    }
    difficulty.drain_rate = drain;
    difficulty.circle_size = circle;
    difficulty.overall_difficulty = od;
    difficulty.approach_rate = ar;
    Ok(difficulty)
}

fn catch_half_width(circle_size: f64) -> f32 {
    let cs = circle_size as f32;
    let difficulty_range = (f64::from(cs) - 5.0) / 5.0;
    let scale = (1.0 - 0.7 * difficulty_range) as f32 / 2.0;
    let scale = scale * 2.0;
    let width = BASE_CATCHER_SIZE * scale.abs() * ALLOWED_CATCH_RANGE;
    let mut half_width = width * 0.5;
    if cs > 5.5 {
        half_width *= 1.0 - (cs - 5.5).max(0.0) * 0.0625;
    }
    half_width
}

fn timing_point_at(beatmap: &NormalizedBeatmap, time: f64) -> f64 {
    let index = beatmap
        .control_points
        .timing
        .partition_point(|point| point.time <= time);
    beatmap
        .control_points
        .timing
        .get(index.saturating_sub(1))
        .or_else(|| beatmap.control_points.timing.first())
        .map_or(1000.0, |point| point.beat_length.clamp(0.6, 600_000.0))
}

fn speed_multiplier_at(beatmap: &NormalizedBeatmap, time: f64) -> f64 {
    let index = beatmap
        .control_points
        .difficulty
        .partition_point(|point| point.time <= time);
    beatmap
        .control_points
        .difficulty
        .get(index.saturating_sub(1))
        .map_or(1.0, |point| point.speed_multiplier.clamp(0.1, 10.0))
}

fn precision_adjusted_beat_length(beat_length: f64, sv: f64) -> f64 {
    beat_length * (((100.0 / sv) as f32).clamp(10.0, 1000.0) as f64 / 100.0)
}

fn effective_x(x: f32, mirror: bool) -> f32 {
    let x = if mirror { 512.0 - x } else { x };
    x.clamp(0.0, 512.0)
}

fn advance_banana_random(
    start_time: f64,
    end_time: f64,
    rng: &mut LegacyRandom,
) -> Result<(), CalculationError> {
    let mut time = start_time.trunc() as f32;
    let end = end_time.trunc() as f32;
    let mut spacing = (end_time - start_time) as f32;
    while spacing > 100.0 {
        spacing /= 2.0;
    }
    if spacing <= 0.0 {
        return Ok(());
    }
    let mut count = 0usize;
    while time <= end {
        count += 1;
        if count > 1_000_000 {
            return Err(CalculationError::InvalidBeatmap(
                "Catch BananaShower exceeds one million bananas".to_owned(),
            ));
        }
        rng.next_double();
        rng.next_int();
        rng.next_int();
        rng.next_int();
        time += spacing;
    }
    Ok(())
}

fn apply_hard_rock_offset(
    original_x: f32,
    start_time: f64,
    last_position: &mut Option<f32>,
    last_start_time: &mut f64,
    rng: &mut LegacyRandom,
) -> f32 {
    let mut position = original_x;
    if last_position.is_none() || *last_position == Some(0.0) {
        *last_position = Some(position);
        *last_start_time = start_time;
        return position;
    }
    let position_difference = position - last_position.unwrap();
    let time_difference = (start_time - *last_start_time).trunc() as i32;
    if time_difference > 1000 {
        *last_position = Some(position);
        *last_start_time = start_time;
        return position;
    }
    if position_difference == 0.0 {
        let right = rng.next_bool();
        let upper = ((time_difference as f64 / 4.0).max(0.0) as i32).max(0);
        let random_offset = (rng.next_range(0, upper) as f32).min(20.0);
        if right {
            if position + random_offset <= 512.0 {
                position += random_offset;
            } else {
                position -= random_offset;
            }
        } else if position - random_offset >= 0.0 {
            position -= random_offset;
        } else {
            position += random_offset;
        }
        return position;
    }
    if position_difference.abs() < time_difference as f32 / 3.0 {
        if position_difference > 0.0 {
            if position + position_difference < 512.0 {
                position += position_difference;
            }
        } else if position + position_difference > 0.0 {
            position += position_difference;
        }
    }
    *last_position = Some(position);
    *last_start_time = start_time;
    position
}

struct LegacyRandom {
    x: u32,
    y: u32,
    z: u32,
    w: u32,
    bit_buffer: u32,
    bit_index: u8,
}

impl LegacyRandom {
    fn new(seed: i32) -> Self {
        Self {
            x: seed as u32,
            y: 842_502_087,
            z: 3_579_807_591,
            w: 273_326_509,
            bit_buffer: 0,
            bit_index: 32,
        }
    }

    fn next_u32(&mut self) -> u32 {
        let t = self.x ^ self.x.wrapping_shl(11);
        self.x = self.y;
        self.y = self.z;
        self.z = self.w;
        self.w = self.w ^ (self.w >> 19) ^ t ^ (t >> 8);
        self.w
    }

    fn next_int(&mut self) -> i32 {
        (self.next_u32() & 0x7fff_ffff) as i32
    }

    fn next_double(&mut self) -> f64 {
        self.next_int() as f64 / 2_147_483_648.0
    }

    fn next_range(&mut self, lower: i32, upper: i32) -> i32 {
        (lower as f64 + self.next_double() * (upper - lower) as f64) as i32
    }

    fn next_bool(&mut self) -> bool {
        if self.bit_index == 32 {
            self.bit_buffer = self.next_u32();
            self.bit_index = 1;
            return self.bit_buffer & 1 == 1;
        }
        self.bit_index += 1;
        self.bit_buffer >>= 1;
        self.bit_buffer & 1 == 1
    }
}

fn calculate_hyper_dashes(objects: &mut [CatchObjectInput], half_width: f32) {
    let mut previous_direction = 0;
    let mut last_excess = f64::from(half_width);
    for index in 0..objects.len().saturating_sub(1) {
        let next_direction = if objects[index + 1].effective_x > objects[index].effective_x {
            1
        } else {
            -1
        };
        let time = objects[index + 1].start_time_ms.trunc()
            - objects[index].start_time_ms.trunc()
            - 1000.0 / 60.0 / 4.0;
        let distance =
            f64::from((objects[index + 1].effective_x - objects[index].effective_x).abs())
                - if previous_direction == next_direction {
                    last_excess
                } else {
                    f64::from(half_width)
                };
        let distance_to_hyper = (time - distance) as f32;
        if distance_to_hyper < 0.0 {
            objects[index].hyper_dash = true;
            last_excess = f64::from(half_width);
        } else {
            objects[index].distance_to_hyper_dash = distance_to_hyper;
            last_excess = f64::from(distance_to_hyper).clamp(0.0, f64::from(half_width));
        }
        previous_direction = next_direction;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalization::normalize;

    #[test]
    fn official_catch_fixture_matches_lazer_golden() {
        let beatmap =
            OsuBeatmap::parse_bytes(include_bytes!("../tests/fixtures/catch-diffcalc-test.osu"))
                .unwrap();
        let result = CatchDifficultyCalculator.calculate(&beatmap, &[]).unwrap();
        assert_eq!(result.max_combo, 127);
        let expected = 4.039861734717169;
        let relative_error = (result.star_rating - expected).abs() / expected;
        assert!(
            relative_error <= 2e-5,
            "expected {expected}, got {} ({relative_error:e} relative error)",
            result.star_rating
        );

        let dt = CatchDifficultyCalculator
            .calculate(&beatmap, &[Mod::DoubleTime])
            .unwrap();
        assert_eq!(dt.max_combo, 127);
        let expected_dt = 5.1527173897800873;
        let dt_relative_error = (dt.star_rating - expected_dt).abs() / expected_dt;
        assert!(
            dt_relative_error <= 2e-5,
            "DoubleTime: expected {expected_dt}, got {} ({dt_relative_error:e})",
            dt.star_rating
        );
    }

    #[test]
    fn standard_maps_convert_to_catch_fruits_and_slider_juice() {
        let beatmap =
            OsuBeatmap::parse_bytes(include_bytes!("../tests/fixtures/osu-diffcalc-test.osu"))
                .unwrap();
        assert_eq!(beatmap.mode, GameMode::Standard);

        let normalized = normalize(&beatmap).unwrap();
        let (objects, max_combo, _) = CatchBeatmapConverter::convert(&normalized, &[]).unwrap();
        assert!(!objects.is_empty());
        assert_eq!(objects.len(), max_combo as usize);
        assert!(max_combo > beatmap.hit_objects.len() as i32);
        assert!(objects.iter().all(|object| {
            object.start_time_ms.is_finite()
                && object.effective_x.is_finite()
                && (0.0..=512.0).contains(&object.effective_x)
        }));

        let difficulty = CatchDifficultyCalculator.calculate(&beatmap, &[]).unwrap();
        assert_eq!(difficulty.max_combo, max_combo);
        assert!(difficulty.star_rating.is_finite() && difficulty.star_rating > 0.0);
    }

    #[test]
    fn mania_maps_convert_using_their_x_position_contract() {
        let mania =
            OsuBeatmap::parse_bytes(include_bytes!("../tests/fixtures/mania-diffcalc-test.osu"))
                .unwrap();
        assert_eq!(mania.mode, GameMode::Mania);
        let result = CatchDifficultyCalculator.calculate(&mania, &[]).unwrap();
        assert!(result.star_rating.is_finite());
        assert!(result.max_combo > 0);
    }

    #[test]
    fn taiko_conversion_is_rejected_without_x_position_support() {
        let bytes = b"osu file format v14\n[General]\nMode:1\n[Difficulty]\nCircleSize:4\n[HitObjects]\n64,192,1000,1,0\n";
        let taiko = OsuBeatmap::parse_bytes(bytes).unwrap();
        assert_eq!(taiko.mode, GameMode::Taiko);
        assert!(matches!(
            CatchDifficultyCalculator.calculate(&taiko, &[]),
            Err(CalculationError::WrongRuleset {
                expected: GameMode::Catch,
                actual: GameMode::Taiko,
            })
        ));
    }
}
