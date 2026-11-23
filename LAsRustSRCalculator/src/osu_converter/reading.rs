// Copyright (c) 2025 ppy Pty Ltd <contact@ppy.sh>.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

use std::f64::consts::PI;

use super::{DiffObject, Kind, PlayableObject};

const READING_WINDOW: f64 = 3000.0;
const READING_DISTANCE: f64 = 150.0;
// .NET's DegreesToRadians(30) rounds up here. One ULP changes the sign of cos(3 * angle)
// at the cap, which determines which side of the clamp its reciprocal lands on.
const MAX_REPEATED_ANGLE: f64 = 0.5235987755982989;

pub(super) fn reading(objects: &[DiffObject<'_>], index: usize, rate: f64, hidden: bool) -> f64 {
    let current = &objects[index];
    if current.object.kind == Kind::Spinner || index == 0 {
        return 0.0;
    }

    let preempt = current.object.preempt / rate;
    let velocity = (current.lazy_jump / current.adjusted_delta).max(1.0);
    let visible_density = current_visible_density(current, &objects[index + 1..], rate);
    let past_difficulty = past_visible_influence(current, &objects[..index], preempt);
    let angle_nerf = repeated_angle_factor(objects, index);

    let future_difficulty = match objects.get(index + 1) {
        Some(next) => visible_density.sqrt() * smootherstep(next.lazy_jump, 15.0, READING_DISTANCE),
        None => visible_density.sqrt(),
    };
    let density = (past_difficulty + future_difficulty).powf(1.7) * 0.4 * angle_nerf * velocity;
    let density_difficulty = (density - 2.5).max(0.0).powf(0.45) * 2.4;

    let hidden_difficulty = if hidden {
        let preempt_factor = preempt.powf(2.2) * 0.01;
        let density_factor = (visible_density + past_difficulty).powf(3.3) * 3.0;
        let hidden_value = (preempt_factor + density_factor) * angle_nerf * velocity * 0.01;
        let mut difficulty = hidden_value.powf(0.4) * 0.28;

        let previous = &objects[index - 1];
        if current.lazy_jump == 0.0
            && opacity_at(current.object, previous.object.start_time, true) == 0.0
            && previous.start_time > current.start_time - preempt
        {
            difficulty += 0.28 * 2500.0 / current.adjusted_delta.powf(1.5);
        }
        difficulty
    } else {
        0.0
    };

    let preempt_difficulty =
        ((500.0 - preempt + (preempt - 500.0).abs()) / 2.0).powf(2.5) / 140_000.0;
    let preempt_difficulty = preempt_difficulty * (angle_nerf * velocity);
    let difficulty =
        (preempt_difficulty.powf(1.5) + hidden_difficulty.powf(1.5) + density_difficulty.powf(1.5))
            .powf(1.0 / 1.5);
    difficulty * (1.0 / (1.0 - 0.8_f64.powf(current.adjusted_delta / 1000.0)))
}

fn current_visible_density(
    current: &DiffObject<'_>,
    following: &[DiffObject<'_>],
    rate: f64,
) -> f64 {
    let mut density = 0.0;
    for next in following {
        let gap = next.start_time - current.start_time;
        if gap > READING_WINDOW || current.start_time < next.start_time - next.object.preempt / rate
        {
            break;
        }
        density += opacity_at(next.object, current.object.start_time, false) * time_factor(gap);
    }
    density
}

fn past_visible_influence(current: &DiffObject<'_>, past: &[DiffObject<'_>], preempt: f64) -> f64 {
    let mut influence = 0.0;
    for previous in past.iter().rev() {
        let gap = current.start_time - previous.start_time;
        if gap > READING_WINDOW || previous.start_time < current.start_time - preempt {
            break;
        }
        let difficulty = opacity_at(current.object, previous.object.start_time, false)
            * smootherstep(previous.lazy_jump, 15.0, READING_DISTANCE);
        influence += difficulty * time_factor(gap);
    }
    influence
}

fn repeated_angle_factor(objects: &[DiffObject<'_>], index: usize) -> f64 {
    let current = &objects[index];
    let Some(current_angle) = current.angle else {
        return 1.0;
    };
    let mut count = 0.0;
    let mut gap = 0.0;
    for previous_index in (0..index).rev() {
        if gap >= 2000.0 {
            break;
        }
        let previous = &objects[previous_index];
        if let Some(angle) = previous.angle {
            let previous_gap_factor = 1.0 - reverse_lerp(previous.adjusted_delta, 200.0, 2000.0);
            let direct_difference = (current_angle - angle).abs();
            let mut alternating_difference = PI;
            if previous_index + 3 <= index
                && let (Some(angle_1), Some(angle_2), Some(angle_3)) = (
                    objects[previous_index + 1].angle,
                    objects[previous_index + 2].angle,
                    objects[previous_index + 3].angle,
                )
            {
                let difference = (angle_2 - angle).abs() + (angle_3 - angle_1).abs();
                let mut weight = 1.0;
                weight *= reverse_lerp(angle.min(angle_1) * 180.0 / PI, 20.0, 5.0);
                weight *= reverse_lerp(angle.max(angle_1) * 180.0 / PI, 60.0, 120.0);
                alternating_difference = PI * (1.0 - weight) + (0.1 * difference) * weight;
            }

            let stack_factor = smootherstep(previous.lazy_jump, 0.0, 50.0);
            let difference = direct_difference.min(alternating_difference) * stack_factor;
            count += (3.0 * MAX_REPEATED_ANGLE.min(difference)).cos() * previous_gap_factor;
        }
        gap = current.start_time - previous.start_time;
    }
    (2.0_f64 / count).clamp(0.2, 1.0)
}

pub(super) fn flashlight(objects: &[DiffObject<'_>], index: usize, hidden: bool) -> f64 {
    let current = &objects[index];
    if current.object.kind == Kind::Spinner {
        return 0.0;
    }

    let scale = 52.0 / current.object.radius;
    let mut small_distance_nerf = 1.0;
    let mut cumulative_strain_time = 0.0;
    let mut angle_repeats = 0.0;
    let mut difficulty = 0.0;
    let mut last = current;

    for (back, previous) in objects[..index].iter().rev().take(10).enumerate() {
        cumulative_strain_time += last.adjusted_delta;
        if previous.object.kind != Kind::Spinner {
            let [x, y] = current.object.position;
            let [previous_x, previous_y] = previous.object.end_position;
            let dx = x - previous_x;
            let dy = y - previous_y;
            let distance = f64::from((dx * dx + dy * dy).sqrt());

            if back == 0 {
                small_distance_nerf = (distance / 75.0).min(1.0);
            }
            let stack_nerf = (previous.lazy_jump / scale / 25.0).min(1.0);
            let opacity = opacity_at(current.object, previous.object.start_time, hidden);
            let opacity_bonus = 1.0 + 0.4 * (1.0 - opacity);
            difficulty += stack_nerf * opacity_bonus * scale * distance / cumulative_strain_time;

            if let (Some(current_angle), Some(previous_angle)) = (current.angle, previous.angle)
                && (current_angle - previous_angle).abs() < 0.02
            {
                angle_repeats += (1.0 - 0.1 * back as f64).max(0.0);
            }
        }
        last = previous;
    }

    let difficulty_with_nerf = small_distance_nerf * difficulty;
    difficulty = difficulty_with_nerf * difficulty_with_nerf;
    if hidden {
        difficulty *= 1.2;
    }
    difficulty *= 0.2 + (1.0 - 0.2) / (angle_repeats + 1.0);

    if current.object.kind == Kind::Slider {
        let pixel_distance = current.lazy_travel / scale;
        let velocity_bonus = (pixel_distance / current.travel_time - 0.5)
            .max(0.0)
            .powf(0.5);
        let mut slider_bonus = velocity_bonus * pixel_distance;
        if current.object.repeat_count > 0 {
            slider_bonus /= f64::from(current.object.repeat_count) + 1.0;
        }
        difficulty += slider_bonus * 1.3;
    }
    difficulty
}

fn opacity_at(object: &PlayableObject, time: f64, hidden: bool) -> f64 {
    if time > object.start_time {
        return 0.0;
    }

    let fade_in_start = object.start_time - object.preempt;
    let fade_in_duration = 400.0 * (object.preempt / 450.0).min(1.0);
    let fade_in = ((time - fade_in_start) / fade_in_duration).clamp(0.0, 1.0);
    if !hidden {
        return fade_in;
    }

    let fade_out_start = fade_in_start + object.fade_in;
    let fade_out_duration = object.preempt * 0.3;
    let fade_out = 1.0 - ((time - fade_out_start) / fade_out_duration).clamp(0.0, 1.0);
    fade_in.min(fade_out)
}

fn time_factor(gap: f64) -> f64 {
    (2.0 - gap / (READING_WINDOW / 2.0)).clamp(0.0, 1.0)
}

fn reverse_lerp(value: f64, start: f64, end: f64) -> f64 {
    ((value - start) / (end - start)).clamp(0.0, 1.0)
}

fn smootherstep(value: f64, start: f64, end: f64) -> f64 {
    let t = reverse_lerp(value, start, end);
    t * t * t * (t * (6.0 * t - 15.0) + 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle(start_time: f64, x: f32) -> PlayableObject {
        PlayableObject {
            start_time,
            end_time: start_time,
            position: [x, 0.0],
            end_position: [x, 0.0],
            radius: 52.0,
            preempt: 500.0,
            fade_in: 200.0,
            kind: Kind::Circle,
            repeat_count: 0,
            great_window: 50.0,
            slider: None,
        }
    }

    fn difficulty_object(object: &PlayableObject, angle: Option<f64>) -> DiffObject<'_> {
        DiffObject {
            object,
            start_time: object.start_time,
            delta: 100.0,
            adjusted_delta: 100.0,
            last_end_delta: 100.0,
            great_window: 100.0,
            min_jump_time: 100.0,
            jump: 100.0,
            lazy_jump: 100.0,
            angle,
            vector_angle: None,
            travel: 0.0,
            travel_time: 0.0,
            lazy_travel: 0.0,
            lazy_end: object.end_position,
        }
    }

    #[test]
    fn capped_reading_angle_matches_dotnet_rounding() {
        let first = circle(0.0, 0.0);
        let second = circle(100.0, 100.0);
        let objects = [
            difficulty_object(&first, Some(2.0)),
            difficulty_object(&second, Some(0.0)),
        ];
        assert_eq!(repeated_angle_factor(&objects, 1), 0.2);
    }

    #[test]
    fn hidden_flashlight_uses_fade_out_and_the_hidden_bonus() {
        let first = circle(0.0, 0.0);
        let second = circle(100.0, 100.0);
        let objects = [
            difficulty_object(&first, None),
            difficulty_object(&second, None),
        ];
        assert_eq!(flashlight(&objects, 1, false), 1.0);
        assert!((flashlight(&objects, 1, true) - 2.352).abs() < 1e-14);
    }
}
