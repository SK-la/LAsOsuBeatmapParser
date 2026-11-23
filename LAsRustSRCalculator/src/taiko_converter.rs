//! Native Taiko beatmap conversion and difficulty calculation.

use crate::attributes::TaikoDifficultyAttributes;
use crate::beatmap::{GameMode, OsuBeatmap};
use crate::calculator::{CalculationError, DifficultyCalculator};
use crate::mods::{Mod, calculate_rate_with_mods};
use crate::normalization::{NormalizedBeatmap, NormalizedHitObjectKind, normalize};
use crate::slider_path::{SliderPath, SliderPathOptions};
use crate::taiko::{
    TaikoDifficultyCalculator as TaikoStrainCalculator, TaikoHitType, TaikoNote, TaikoObjectType,
};

/// Converts normalized osu!standard or native Taiko objects into the Taiko difficulty core input.
pub struct TaikoBeatmapConverter;

impl TaikoBeatmapConverter {
    pub fn convert(
        beatmap: &NormalizedBeatmap,
        mods: &[Mod],
    ) -> Result<Vec<TaikoNote>, CalculationError> {
        if !matches!(beatmap.mode, GameMode::Taiko | GameMode::Standard) {
            return Err(CalculationError::WrongRuleset {
                expected: GameMode::Taiko,
                actual: beatmap.mode,
            });
        }

        let rate = calculate_rate_with_mods(mods)
            .map_err(|error| CalculationError::InvalidMod(error.to_string()))?;
        let (_, adjusted_slider_multiplier) = adjusted_difficulty(beatmap, mods)?;
        let timing = &beatmap.control_points.timing;
        let difficulty = &beatmap.control_points.difficulty;
        let effects = &beatmap.control_points.effect;
        let mut notes = Vec::with_capacity(beatmap.hit_objects.len());
        let is_native_taiko = beatmap.mode == GameMode::Taiko;
        let mut converted_scroll_changes = Vec::<(f64, f64)>::new();
        let mut last_scroll_speed = 1.0;

        for object in &beatmap.hit_objects {
            let start = object.start_time;
            // The C# difficulty object looks up the rate-adjusted time converted back to source time.
            let lookup_time = (start / rate) * rate;
            let beat_length = |time| {
                let index = timing.partition_point(|point| point.time <= time);
                timing
                    .get(index.saturating_sub(1))
                    .map_or(1000.0, |point| point.beat_length)
            };
            let scroll_speed = if is_native_taiko {
                let effect_index = effects.partition_point(|point| point.time <= lookup_time);
                effect_index
                    .checked_sub(1)
                    .and_then(|index| effects[index].scroll_speed)
                    .unwrap_or(1.0)
                    .clamp(0.01, 10.0)
            } else {
                if matches!(&object.kind, NormalizedHitObjectKind::Slider { .. }) {
                    let sv_index = difficulty.partition_point(|point| point.time <= start);
                    let sv = sv_index.checked_sub(1).map_or(1.0, |index| {
                        difficulty[index].speed_multiplier.clamp(0.1, 10.0)
                    });
                    if (last_scroll_speed - sv).abs() > 0.001 {
                        converted_scroll_changes.push((start, sv.clamp(0.01, 10.0)));
                        last_scroll_speed = sv;
                    }
                }
                converted_scroll_changes
                    .iter()
                    .rev()
                    .find(|(time, _)| *time <= lookup_time)
                    .map_or(1.0, |(_, value)| *value)
            };
            let effective_bpm =
                (60_000.0 / beat_length(lookup_time)) * (adjusted_slider_multiplier * scroll_speed);

            let (object_type, end, hit_type) = match &object.kind {
                NormalizedHitObjectKind::Circle => {
                    let hit_type = if object.hitsound & (2 | 8) != 0 {
                        TaikoHitType::Rim
                    } else {
                        TaikoHitType::Centre
                    };
                    (TaikoObjectType::Hit, start, Some(hit_type))
                }
                NormalizedHitObjectKind::Hold { end_time }
                | NormalizedHitObjectKind::Spinner { end_time } => {
                    (TaikoObjectType::Swell, *end_time, None)
                }
                NormalizedHitObjectKind::Slider {
                    raw_curve,
                    path_control_points,
                    repeat_count,
                    pixel_length,
                    edge_sounds,
                    ..
                } => {
                    let sv_index = difficulty.partition_point(|point| point.time <= start);
                    let sv = sv_index.checked_sub(1).map_or(1.0, |index| {
                        difficulty[index].speed_multiplier.clamp(0.1, 10.0)
                    });
                    let adjusted_length = beat_length(start)
                        * (((100.0 / sv) as f32).clamp(10.0, 10_000.0) as f64 / 100.0);
                    let path = SliderPath::from_legacy_curve(
                        raw_curve,
                        path_control_points,
                        *pixel_length,
                        beatmap.version,
                        SliderPathOptions::default(),
                    )
                    .map_err(|error| CalculationError::InvalidBeatmap(error.to_string()))?;
                    let velocity_multiplier = f64::from(1.4_f32);
                    // The taiko converter uses ExpectedDistance, not the realized curve distance.
                    let mut distance = pixel_length.unwrap_or(0.0) * velocity_multiplier;
                    let span_count = if path.distance().abs() <= 1e-7 {
                        1
                    } else {
                        repeat_count.saturating_add(1)
                    };
                    distance *= f64::from(span_count);
                    let scoring_distance = 100.0
                        * (beatmap.difficulty.slider_multiplier * velocity_multiplier)
                        / beatmap.difficulty.slider_tick_rate;
                    let velocity = scoring_distance * beatmap.difficulty.slider_tick_rate;
                    let duration = (distance / velocity * adjusted_length).trunc();
                    if is_native_taiko {
                        (TaikoObjectType::DrumRoll, start + duration, None)
                    } else {
                        let osu_velocity = velocity * (1000.0 / adjusted_length);
                        let mut conversion_beat_length = adjusted_length;
                        if beatmap.version >= 8 {
                            conversion_beat_length = beat_length(start);
                        }
                        let tick_spacing = (conversion_beat_length
                            / beatmap.difficulty.slider_tick_rate)
                            .min(duration / f64::from(span_count));
                        let convert_to_hits = tick_spacing > 0.0
                            && distance / osu_velocity * 1000.0 < 2.0 * conversion_beat_length;

                        if convert_to_hits {
                            let node_count = (span_count as usize).saturating_add(1);
                            let mut generated = Vec::new();
                            let mut time = start;
                            let end = start + duration + tick_spacing / 8.0;
                            let mut node = 0usize;
                            while time <= end {
                                let sound =
                                    edge_sounds.get(node).copied().unwrap_or(object.hitsound);
                                generated.push(TaikoNote {
                                    start_time_ms: time,
                                    end_time_ms: time,
                                    effective_bpm,
                                    object_type: TaikoObjectType::Hit,
                                    hit_type: Some(if sound & (2 | 8) != 0 {
                                        TaikoHitType::Rim
                                    } else {
                                        TaikoHitType::Centre
                                    }),
                                    is_strong: sound & 4 != 0,
                                    max_combo_contribution: 1,
                                });
                                node = (node + 1) % node_count;
                                time += tick_spacing;
                                if !time.is_finite() || generated.len() > 1_000_000 {
                                    return Err(CalculationError::InvalidBeatmap(
                                        "Taiko slider conversion exceeded one million hits"
                                            .to_owned(),
                                    ));
                                }
                            }
                            notes.extend(generated);
                            continue;
                        }
                        (TaikoObjectType::DrumRoll, start + duration, None)
                    }
                }
            };

            notes.push(TaikoNote {
                start_time_ms: start,
                end_time_ms: end,
                effective_bpm,
                object_type,
                hit_type,
                is_strong: object_type != TaikoObjectType::Swell && object.hitsound & 4 != 0,
                max_combo_contribution: i32::from(object_type == TaikoObjectType::Hit),
            });
        }

        Ok(notes)
    }
}

pub(crate) fn adjusted_difficulty(
    beatmap: &NormalizedBeatmap,
    mods: &[Mod],
) -> Result<(f64, f64), CalculationError> {
    let mut od = beatmap.difficulty.overall_difficulty as f32;
    let mut slider_multiplier = beatmap.difficulty.slider_multiplier;
    for modifier in mods {
        match modifier {
            Mod::Easy => {
                od *= 0.5;
                slider_multiplier *= 0.8;
            }
            Mod::HardRock => {
                od = (od * 1.4).min(10.0);
                slider_multiplier *= 1.4 * 4.0 / 3.0;
            }
            Mod::DifficultyAdjust(settings) if settings.overall_difficulty.is_some() => {
                od = settings.overall_difficulty.unwrap() as f32;
            }
            Mod::Other(id) => return Err(CalculationError::InvalidMod(id.clone())),
            _ => {}
        }
    }
    if !od.is_finite() || !slider_multiplier.is_finite() {
        return Err(CalculationError::InvalidMod(
            "non-finite Taiko difficulty".to_owned(),
        ));
    }
    Ok((f64::from(od), slider_multiplier))
}

pub struct TaikoBeatmapDifficultyCalculator;

impl DifficultyCalculator for TaikoBeatmapDifficultyCalculator {
    type Attributes = TaikoDifficultyAttributes;

    fn version(&self) -> u32 {
        20260706
    }

    fn calculate(
        &self,
        beatmap: &OsuBeatmap,
        mods: &[Mod],
    ) -> Result<Self::Attributes, CalculationError> {
        let normalized = normalize(beatmap)
            .map_err(|error| CalculationError::InvalidBeatmap(error.to_string()))?;
        let notes = TaikoBeatmapConverter::convert(&normalized, mods)?;
        let (od, _) = adjusted_difficulty(&normalized, mods)?;

        TaikoStrainCalculator::calculate(&notes, mods, od, beatmap.mode != GameMode::Taiko)
            .map_err(|error| CalculationError::InvalidBeatmap(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_taiko_fixture_converts_hits_rolls_and_swells() {
        let beatmap =
            OsuBeatmap::parse_bytes(include_bytes!("../tests/fixtures/taiko-diffcalc-test.osu"))
                .unwrap();
        let normalized = normalize(&beatmap).unwrap();
        let notes = TaikoBeatmapConverter::convert(&normalized, &[]).unwrap();
        assert_eq!(
            notes
                .iter()
                .filter(|note| note.object_type == TaikoObjectType::Hit)
                .count(),
            200
        );
        assert_eq!(
            notes
                .iter()
                .filter(|note| note.object_type == TaikoObjectType::DrumRoll)
                .count(),
            30
        );
        assert_eq!(
            notes
                .iter()
                .filter(|note| note.object_type == TaikoObjectType::Swell)
                .count(),
            8
        );
        assert_eq!(
            notes
                .iter()
                .map(|note| note.max_combo_contribution)
                .sum::<i32>(),
            200
        );
    }

    #[test]
    fn official_taiko_fixture_matches_lazer_with_and_without_double_time() {
        let beatmap =
            OsuBeatmap::parse_bytes(include_bytes!("../tests/fixtures/taiko-diffcalc-test.osu"))
                .unwrap();
        for (mods, expected) in [
            ([].as_slice(), 3.3190848563395079_f64),
            ([Mod::DoubleTime].as_slice(), 4.4551414906554987),
        ] {
            let result = TaikoBeatmapDifficultyCalculator
                .calculate(&beatmap, mods)
                .unwrap();
            assert_eq!(result.max_combo, 200);
            let relative_error = (result.star_rating - expected).abs() / expected;
            assert!(
                relative_error <= 2e-5,
                "{mods:?}: {result:?} ({relative_error:e} relative error)"
            );
        }

        for (mods, expected) in [
            ([].as_slice(), 3.3190848563395079_f64),
            ([Mod::DoubleTime].as_slice(), 4.4551414906554987),
        ] {
            let result = TaikoBeatmapDifficultyCalculator
                .calculate(&beatmap, mods)
                .unwrap();
            assert_eq!(result.max_combo, 200);
            let relative_error = (result.star_rating - expected).abs() / expected;
            assert!(
                relative_error <= 2e-5,
                "{mods:?}: expected {expected}, got {} ({relative_error:e})",
                result.star_rating
            );
        }
    }

    #[test]
    fn official_osu_slider_fixture_matches_taiko_drumroll_conversion_golden() {
        let beatmap = OsuBeatmap::parse_bytes(include_bytes!(
            "../tests/fixtures/taiko-slider-generating-drumroll.osu"
        ))
        .unwrap();
        let normalized = normalize(&beatmap).unwrap();
        let notes = TaikoBeatmapConverter::convert(&normalized, &[]).unwrap();

        for (time, expected_end) in [
            (6590.0, 8320.0),
            (8436.0, 10166.0),
            (10282.0, 12012.0),
            (12128.0, 13858.0),
        ] {
            let roll = notes
                .iter()
                .find(|note| note.start_time_ms == time)
                .unwrap();
            assert_eq!(roll.object_type, TaikoObjectType::DrumRoll);
            assert!((roll.end_time_ms - expected_end).abs() < 2.0);
        }
    }
}
