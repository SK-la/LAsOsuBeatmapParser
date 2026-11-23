use super::{DiffObject, Kind, distance};

fn lerp_range(value: f64, start: f64, end: f64) -> f64 {
    ((value - start) / (end - start)).clamp(0.0, 1.0)
}

fn smooth(value: f64, start: f64, end: f64) -> f64 {
    let x = lerp_range(value, start, end);
    x * x * (3.0 - 2.0 * x)
}

fn smoother(value: f64, start: f64, end: f64) -> f64 {
    let x = lerp_range(value, start, end);
    x * x * x * (x * (6.0 * x - 15.0) + 10.0)
}

fn small_circle_bonus(object: &DiffObject<'_>) -> f64 {
    (1.0 + (30.0 - object.object.radius) / 70.0).max(1.0)
}

fn acuteness(angle: f64) -> f64 {
    smooth(angle, 140_f64.to_radians(), 40_f64.to_radians())
}

fn wideness(angle: f64) -> f64 {
    smooth(angle, 40_f64.to_radians(), 140_f64.to_radians())
}

pub(super) fn agility(objects: &[DiffObject<'_>], index: usize) -> f64 {
    let current = &objects[index];
    if current.object.kind == Kind::Spinner {
        return 0.0;
    }
    let last_distance = index
        .checked_sub(1)
        .map_or(0.0, |previous| objects[previous].lazy_travel);
    let distance = (last_distance + current.lazy_jump).min(120.0) / 120.0;
    let raw = distance * 1000.0 / current.adjusted_delta;
    raw * small_circle_bonus(current).powf(1.5)
        / (1.0 - 0.2_f64.powf(current.adjusted_delta / 1000.0))
}

pub(super) fn snap(objects: &[DiffObject<'_>], index: usize, sliders: bool) -> f64 {
    let current = &objects[index];
    if current.object.kind == Kind::Spinner || index <= 1 {
        return 0.0;
    }
    let previous = &objects[index - 1];
    if previous.object.kind == Kind::Spinner {
        return 0.0;
    }
    let current_distance = if sliders {
        current.lazy_jump
    } else {
        current.jump
    };
    let previous_distance = if sliders {
        previous.lazy_jump
    } else {
        previous.jump
    };
    let current_time = current.adjusted_delta;
    let previous_time = previous.adjusted_delta;
    let slider_exit = sliders && previous.object.kind == Kind::Slider;
    let normalized_slider_exit = previous.lazy_travel + current.lazy_jump;
    let mut current_velocity = current_distance / current_time;
    if slider_exit {
        current_velocity = current_velocity.max(normalized_slider_exit / current_time);
    }
    let previous_velocity = previous_distance / previous_time;
    let min_time = current_time.min(previous_time);
    let max_time = current_time.max(previous_time);
    let mut value = current_velocity * vector_angle_repetition(objects, index);
    if let (Some(current_angle), Some(previous_angle)) = (current.angle, previous.angle) {
        let velocity_influence = current_velocity.min(previous_velocity);
        let mut acute = 0.0;
        if max_time < 1.25 * min_time {
            acute = acuteness(current_angle);
            acute *= 0.08 + 0.92 * (1.0 - acute.min(acuteness(previous_angle).powi(3)));
            acute *= velocity_influence
                * smoother(60_000.0 / (2.0 * current_time), 300.0, 400.0)
                * smoother(current_distance, 0.0, 200.0);
        }
        let mut wide = wideness(current_angle);
        wide *= 0.25 + 0.75 * (1.0 - wide.min(wideness(previous_angle).powi(3)));
        let wide_velocity = current_distance / current_time.powf(1.45);
        let wide_velocity = if slider_exit {
            wide_velocity.max(normalized_slider_exit / current_time.powf(1.45))
        } else {
            wide_velocity
        };
        wide *= wide_velocity.min(previous_distance / previous_time.powf(1.45));
        if index >= 3 {
            let backtrack = distance(objects[index - 3].object.position, previous.object.position);
            if backtrack < 1.0 {
                wide *= 1.0 - 0.55 * (1.0 - f64::from(backtrack));
            }
        }
        value += (acute * 2.41).max(wide * 9.67);
        let reversal = |distance: f64, angle: f64| {
            smoother(distance, 50.0, 100.0)
                * lerp_range(distance, 300.0, 100.0).powf(1.8)
                * smoother(angle, 110_f64.to_radians(), 60_f64.to_radians())
        };
        value += velocity_influence
            * reversal(current_distance, current_angle)
            * reversal(previous_distance, previous_angle)
            * 1.02;
    }
    if previous_velocity.max(current_velocity) != 0.0 {
        if sliders {
            current_velocity = current_distance / current_time;
        }
        let difference = (previous_velocity - current_velocity).abs();
        let ratio = smooth(
            difference / previous_velocity.max(current_velocity),
            0.0,
            1.0,
        );
        let bonus = (125.0 / min_time).min(difference) * ratio * (min_time / max_time).powi(2);
        value += bonus * 0.9;
    }
    if current.object.kind == Kind::Slider && sliders {
        let velocity = current.travel / current.travel_time;
        value += if velocity < 1.0 {
            velocity
        } else {
            velocity.powf(0.75)
        } * 1.5;
    }
    value * small_circle_bonus(current) / (1.0 - 0.03_f64.powf((current_time / 1000.0).powf(0.65)))
}

fn vector_angle_repetition(objects: &[DiffObject<'_>], index: usize) -> f64 {
    let current = &objects[index];
    let previous = &objects[index - 1];
    let (Some(current_angle), Some(previous_angle)) = (current.angle, previous.angle) else {
        return 1.0;
    };
    let mut repetition = 0.0;
    for previous in objects[..index].iter().rev().take(6) {
        if current.adjusted_delta.max(previous.adjusted_delta)
            > 1.1 * current.adjusted_delta.min(previous.adjusted_delta)
        {
            break;
        }
        if let (Some(angle), Some(prior_angle)) = (current.vector_angle, previous.vector_angle) {
            repetition += (8.0 * (angle - prior_angle).abs().min(11.25_f64.to_radians())).cos();
        }
    }
    let repetition = (0.5_f64 / repetition).min(1.0).powi(2);
    let stack = smoother(current.lazy_jump, 0.0, 100.0);
    let angle_difference =
        2.0 * ((current_angle - previous_angle).abs() * stack).min(45_f64.to_radians());
    let base = 1.0 - 0.15 * acuteness(previous_angle) * angle_difference.cos();
    (base + (1.0 - base) * repetition * 0.5 * stack).powi(2)
}

pub(super) fn flow(objects: &[DiffObject<'_>], index: usize, sliders: bool) -> f64 {
    let current = &objects[index];
    if current.object.kind == Kind::Spinner || index <= 1 {
        return 0.0;
    }
    let previous = &objects[index - 1];
    if previous.object.kind == Kind::Spinner {
        return 0.0;
    }
    let current_distance = if sliders {
        current.lazy_jump
    } else {
        current.jump
    };
    let previous_distance = if sliders {
        previous.lazy_jump
    } else {
        previous.jump
    };
    let current_time = current.adjusted_delta;
    let previous_time = previous.adjusted_delta;
    let mut current_velocity = current_distance / current_time;
    if sliders && previous.object.kind == Kind::Slider {
        current_velocity =
            current_velocity.max((previous.lazy_travel + current.lazy_jump) / current_time);
    }
    let previous_velocity = previous_distance / previous_time;
    let mut value = current_velocity * small_circle_bonus(current).sqrt();
    value *= 1.0 + 0.25_f64.min(((current_time - previous_time).abs() / 50.0).powi(4));
    if let (Some(current_angle), Some(previous_angle)) = (current.angle, previous.angle) {
        let angle_difference = ((current_angle - previous_angle).abs() / 2.0).sin() * 180.0;
        let angular_velocity = angle_difference / (current_time * 0.1);
        value *= 0.8 + (angular_velocity / 270.0).sqrt();
    }
    let mut overlap_weight = 1.0;
    if index > 2 {
        let before = &objects[index - 2];
        overlap_weight -=
            overlap(current, previous) * overlap(current, before) * overlap(previous, before);
    }
    if let Some(angle) = current.angle {
        value += current_velocity * acuteness(angle) * overlap_weight;
    }
    if previous_velocity.max(current_velocity) != 0.0 {
        if sliders {
            current_velocity = current_distance / current_time;
        }
        let difference = (previous_velocity - current_velocity).abs();
        let ratio = smooth(
            difference / previous_velocity.max(current_velocity),
            0.0,
            1.0,
        );
        let bonus = (125.0 / current_time.min(previous_time)).min(difference);
        value += bonus * ratio * overlap_weight * 0.52;
    }
    if current.object.kind == Kind::Slider && sliders {
        value += current.travel / current.travel_time;
    }
    value.powf(1.45) * smoother(current_distance, 0.0, 50.0)
}

fn overlap(first: &DiffObject<'_>, second: &DiffObject<'_>) -> f64 {
    let radius = first.object.radius;
    let distance = f64::from(distance(first.object.position, second.object.position));
    (1.0 - ((distance - radius).max(0.0) / radius).powi(2)).clamp(0.0, 1.0)
}
