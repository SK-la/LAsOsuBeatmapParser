//! Ez2Lazer osu! PP+ chart skills over playable osu objects.

use crate::mods::Mod;
use crate::skills::{EzPPPOSUSkillMetric, SkillVector};

const NORMALISED_RADIUS: f64 = 52.0;
const SECTION_LENGTH: f64 = 400.0;
const GAMMA: f64 = 0.9;
const RATING_SCALE: f64 = 0.0675;
const FIELD_WIDTH: f32 = 512.0;
const FIELD_HEIGHT: f32 = 384.0;
const RATIO_TOLERANCE_MS: f64 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OsuPppObjectKind {
    Circle,
    Slider,
    Spinner,
    Other,
}

/// Playable osu object fields consumed by EzPPP. Slider samples are relative path offsets at i/24 progress.
#[derive(Clone, Debug, PartialEq)]
pub struct OsuPppObject {
    pub kind: OsuPppObjectKind,
    pub start_time_ms: f64,
    pub end_time_ms: f64,
    pub radius: f64,
    pub time_preempt_ms: f64,
    pub stacked_position: [f32; 2],
    pub stacked_end_position: [f32; 2],
    pub slider_positions_at_samples: Vec<[f32; 2]>,
}

impl OsuPppObject {
    pub fn circle(time: f64, position: [f32; 2], radius: f64, preempt: f64) -> Self {
        Self {
            kind: OsuPppObjectKind::Circle,
            start_time_ms: time,
            end_time_ms: time,
            radius,
            time_preempt_ms: preempt,
            stacked_position: position,
            stacked_end_position: position,
            slider_positions_at_samples: Vec::new(),
        }
    }
}

pub struct EzPPPOsu;

impl EzPPPOsu {
    /// Calculate osu chart metrics in EzPPP catalog order.
    ///
    /// Input objects must already be converter-sorted and have mod-adjusted radius and preempt, stacked
    /// positions, slider end time and slider path samples. Equal-time objects retain the supplied order.
    pub fn calculate(notes: &[OsuPppObject], mods: &[Mod]) -> SkillVector<EzPPPOSUSkillMetric> {
        if notes.len() < 3 || !valid_inputs(notes) {
            return SkillVector::zeros();
        }
        let hidden = mods.iter().any(is_hidden_mod);
        let flashlight = mods
            .iter()
            .any(|modifier| matches!(modifier, Mod::Flashlight));
        let count = notes.len();
        let mut start = vec![0.0; count];
        let mut end = vec![0.0; count];
        let mut strain = vec![0.0; count];
        let mut gap = vec![0.0; count];
        let mut delta = vec![0.0; count];
        let mut delta2 = vec![0.0; count];
        let mut raw_distance = vec![0.0; count];
        let mut distance = vec![0.0; count];
        let mut angle = vec![f64::NAN; count];
        let mut flow = vec![0.0; count];
        let mut base_flow = vec![0.0; count];
        let mut leniency = vec![0.0; count];
        let mut radius = vec![0.0; count];
        let mut preempt = vec![0.0; count];
        let mut cursor_end = vec![[0.0_f32; 2]; count];

        for (i, current) in notes.iter().enumerate() {
            radius[i] = current.radius.max(1.0);
            preempt[i] = current.time_preempt_ms.max(1.0);
            cursor_end[i] = cursor_end_of(current);
            start[i] = current.start_time_ms;
            end[i] = current.end_time_ms;
            if i == 0 {
                strain[i] = start[i].max(50.0);
                delta2[i] = 100.0;
                continue;
            }
            delta[i] = (start[i] - start[i - 1]).max(0.0);
            strain[i] = delta[i].max(50.0);
            gap[i] = if matches!(
                notes[i - 1].kind,
                OsuPppObjectKind::Slider | OsuPppObjectKind::Spinner
            ) {
                (start[i] - end[i - 1]).max(50.0)
            } else {
                strain[i]
            };
            delta2[i] = if i >= 2 {
                (start[i] - start[i - 2]).max(100.0)
            } else {
                100.0
            };
            if current.kind != OsuPppObjectKind::Spinner {
                raw_distance[i] = distance_f32(cursor_end[i - 1], current.stacked_position) as f64;
                distance[i] = raw_distance[i] * scaling_factor(radius[i]);
            }
            if i >= 2 {
                angle[i] = angle_of(
                    cursor_end[i - 2],
                    notes[i - 1].stacked_position,
                    cursor_end[i - 1],
                    current.stacked_position,
                );
            }
        }

        for i in 0..count {
            // The PPP flow state machine compares against the object two steps back.
            let previous2 = i.checked_sub(2);
            let bpm = 15000.0 / strain[i];
            let speed_factor = t2t(bpm, 90.0, 30.0);
            base_flow[i] = match previous2 {
                None => speed_factor * distance_factor(distance[i], bpm, 1.0),
                Some(p) if strain[i] - 5.0 < (2.0 / 3.0) * strain[p] => {
                    speed_factor * distance_factor(distance[i], bpm, 1.0)
                }
                Some(p) if coarse_equal(strain[i], strain[p]) => {
                    let a = angle_scale(angle[i]);
                    speed_factor * distance_factor(distance[i], bpm, a + (1.0 - a) * leniency[p])
                }
                _ => 0.0,
            };
            let irregular = if let Some(p) = previous2 {
                let mut value = t2f(distance[i], wide_distance(bpm), wide_distance(bpm));
                if coarse_equal(strain[i], strain[p]) {
                    value *= base_flow[p];
                } else {
                    value = 0.0;
                }
                if coarse_equal(strain[i], strain[i - 1]) {
                    value *= base_flow[i - 1];
                } else {
                    value = 0.0;
                }
                value
            } else {
                0.0
            };
            flow[i] = base_flow[i].max(irregular);
            leniency[i] = if previous2.is_some() {
                (1.0 - base_flow[i]) * irregular
            } else {
                0.0
            };
        }

        let (mut aim_state, mut raw_state, mut jump_state, mut flow_state) = (0.0, 0.0, 0.0, 0.0);
        let (mut speed_state, mut stamina_state) = (0.0, 0.0);
        let (mut times, mut aim_strain, mut raw_strain) = (
            Vec::with_capacity(count),
            Vec::with_capacity(count),
            Vec::with_capacity(count),
        );
        let (mut jump_strain, mut flow_strain, mut speed_strain, mut stamina_strain) = (
            Vec::with_capacity(count),
            Vec::with_capacity(count),
            Vec::with_capacity(count),
            Vec::with_capacity(count),
        );
        let mut rhythm = RhythmComplexity::default();
        let circles = notes
            .iter()
            .filter(|note| note.kind == OsuPppObjectKind::Circle)
            .count();

        for i in 0..count {
            let read = reading(
                i,
                &start,
                &base_flow,
                &distance,
                preempt[i],
                hidden,
                flashlight,
                raw_distance[i],
                radius[i],
            );
            let previous_position = if i > 0 {
                notes[i - 1].stacked_position
            } else {
                notes[i].stacked_position
            };
            let location = location_weight(previous_position, notes[i].stacked_position);
            let requirement = distance_requirement(i, &distance, &strain);
            let circle_bonus = 1.0 + 120.0 / radius[i].powi(2);
            let jump = jump_component(
                i,
                &distance,
                &strain,
                &angle,
                requirement,
                location,
                flow[i],
            );
            let aim_flow = flow_component(i, &distance, &strain, &angle, &flow, location);
            let jump_raw = jump_component(
                i,
                &raw_distance,
                &strain,
                &angle,
                requirement,
                location,
                flow[i],
            );
            let aim_decay = decay(delta[i], 0.15);
            aim_state = aim_decay * aim_state + 1059.0 * (jump + aim_flow) * circle_bonus * read;
            raw_state = aim_decay * raw_state + 1059.0 * (jump_raw + aim_flow) * read;
            jump_state = aim_decay * jump_state + 1059.0 * jump * circle_bonus * read;
            flow_state = aim_decay * flow_state + 1059.0 * aim_flow * circle_bonus * read;
            let half = delta2[i] / 2.0;
            let half_error = half - 20.0;
            let tap = 30.0 / (half_error * half_error) + 2.0 / half;
            let stream = 12.5 / (half_error * half_error) + 0.25 / half + 0.005;
            speed_state = decay(delta[i], 0.1) * speed_state
                + 2600.0 * ((1.0 - flow[i]) * tap + flow[i] * stream);
            let stamina_add = (1.0 - flow[i]) * (2.0 / half_error) + flow[i] * (1.0 / half_error);
            stamina_state = decay(delta[i], 0.45) * stamina_state + 780.0 * stamina_add;
            times.push(start[i]);
            aim_strain.push(aim_state);
            raw_strain.push(raw_state);
            jump_strain.push(jump_state);
            flow_strain.push(flow_state);
            speed_strain.push(speed_state);
            stamina_strain.push(stamina_state);
            if notes[i].kind == OsuPppObjectKind::Circle {
                rhythm.add(
                    i,
                    i.checked_sub(1).map(|p| notes[p].kind),
                    &gap,
                    &strain,
                    flow[i],
                    distance[i],
                );
            } else {
                rhythm.reset();
            }
        }

        let aim_value = aggregate(&times, &aim_strain, 0.15);
        let raw_value = aggregate(&times, &raw_strain, 0.15);
        let jump_value = aggregate(&times, &jump_strain, 0.15);
        let flow_value = aggregate(&times, &flow_strain, 0.15);
        let speed_value = aggregate(&times, &speed_strain, 0.1);
        let stamina_value = aggregate(&times, &stamina_strain, 0.45);
        let aim = RATING_SCALE * aim_value.max(0.0).sqrt();
        let raw = RATING_SCALE * raw_value.max(0.0).sqrt();
        let jump = RATING_SCALE * jump_value.max(0.0).sqrt();
        let flow_rating = RATING_SCALE * flow_value.max(0.0).sqrt();
        let precision = RATING_SCALE * (aim_value - raw_value).max(0.0).sqrt();
        let speed = RATING_SCALE * speed_value.max(0.0).sqrt();
        let stamina = RATING_SCALE * stamina_value.max(0.0).sqrt();
        let accuracy = rhythm.value(circles);
        let star = 1.6 * (aim.powi(3) + speed.max(stamina).powi(3)).powf(1.0 / 3.0);
        SkillVector::from_fn(|axis| {
            finite(match axis {
                EzPPPOSUSkillMetric::Aim => aim,
                EzPPPOSUSkillMetric::RawAim => raw,
                EzPPPOSUSkillMetric::JumpAim => jump,
                EzPPPOSUSkillMetric::FlowAim => flow_rating,
                EzPPPOSUSkillMetric::Precision => precision,
                EzPPPOSUSkillMetric::Speed => speed,
                EzPPPOSUSkillMetric::Stamina => stamina,
                EzPPPOSUSkillMetric::Accuracy | EzPPPOSUSkillMetric::AccuracyDifficulty => accuracy,
                EzPPPOSUSkillMetric::StarRating => star,
            })
        })
    }
}

fn finite(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

fn valid_inputs(notes: &[OsuPppObject]) -> bool {
    notes.iter().all(|note| {
        note.start_time_ms.is_finite()
            && note.end_time_ms.is_finite()
            && note.radius.is_finite()
            && note.time_preempt_ms.is_finite()
            && note.stacked_position.iter().all(|v| v.is_finite())
            && note.stacked_end_position.iter().all(|v| v.is_finite())
            && note
                .slider_positions_at_samples
                .iter()
                .flatten()
                .all(|v| v.is_finite())
    })
}

fn is_hidden_mod(modifier: &Mod) -> bool {
    match modifier {
        Mod::Hidden => true,
        Mod::Other(id) => id == "BL" || id == "TC",
        _ => false,
    }
}

fn decay(delta: f64, beta: f64) -> f64 {
    if delta <= 0.0 {
        1.0
    } else if beta <= 0.0 {
        0.0
    } else {
        beta.powf(delta / 1000.0)
    }
}

fn t2t(value: f64, start: f64, length: f64) -> f64 {
    if length <= 0.0 {
        return if value >= start { 1.0 } else { 0.0 };
    }
    if value <= start {
        0.0
    } else if value >= start + length {
        1.0
    } else {
        (1.0 - ((value - start) * std::f64::consts::PI / length).cos()) / 2.0
    }
}

fn t2f(value: f64, start: f64, length: f64) -> f64 {
    1.0 - t2t(value, start, length)
}
fn ratio_equal(current: f64, previous: f64, expected: f64) -> bool {
    current + 5.0 > expected * previous && current - 5.0 < expected * previous
}
fn ratio_greater(current: f64, previous: f64, expected: f64) -> bool {
    current + 5.0 > expected * previous
}
fn ratio_less(current: f64, previous: f64, expected: f64) -> bool {
    current - 5.0 < expected * previous
}
fn coarse_equal(a: f64, b: f64) -> bool {
    a * 1.25 > b && a / 1.25 < b
}

fn aggregate(times: &[f64], strain: &[f64], beta: f64) -> f64 {
    let mut section_end = (times[0] / SECTION_LENGTH).ceil() * SECTION_LENGTH;
    let mut peak = 0.0;
    let mut peaks = Vec::new();
    for (i, &time) in times.iter().enumerate() {
        while time > section_end {
            peaks.push(peak);
            peak = if i > 0 {
                strain[i - 1] * decay(section_end - times[i - 1], beta)
            } else {
                0.0
            };
            section_end += SECTION_LENGTH;
        }
        if strain[i].is_finite() && strain[i] > peak {
            peak = strain[i];
        }
    }
    peaks.push(peak);
    peaks.retain(|value| *value > 0.0 && value.is_finite());
    peaks.sort_by(|a, b| b.total_cmp(a));
    peaks
        .iter()
        .enumerate()
        .map(|(i, value)| value * GAMMA.powf(i as f64))
        .sum()
}

fn scaling_factor(radius: f64) -> f64 {
    let radius = radius.max(1.0);
    let mut factor = NORMALISED_RADIUS / radius;
    if radius < 30.0 {
        factor *= 1.0 + (30.0 - radius).min(5.0) / 50.0;
    }
    factor
}

fn wide_distance(bpm: f64) -> f64 {
    ((bpm - 140.0) / 20.0).tanh() * 1.75 * NORMALISED_RADIUS + 2.75 * NORMALISED_RADIUS
}

fn distance_factor(distance: f64, bpm: f64, alpha: f64) -> f64 {
    let threshold = (((bpm - 140.0) / 20.0).tanh() + 2.0) * NORMALISED_RADIUS;
    t2f(distance, threshold * alpha, threshold)
}

fn angle_scale(angle: f64) -> f64 {
    if angle.is_nan() {
        0.5
    } else {
        (-(angle.cos() * std::f64::consts::PI / 2.0).sin() + 3.0) / 4.0
    }
}

fn distance_requirement(i: usize, distance: &[f64], strain: &[f64]) -> f64 {
    if i == 0 || !(strain[i] + RATIO_TOLERANCE_MS > strain[i - 1]) {
        0.0
    } else {
        t2t(
            distance[i - 1],
            0.0,
            strain[i - 1] / strain[i] * 2.0 * NORMALISED_RADIUS,
        )
    }
}

fn jump_basis(distance: f64, strain: f64) -> f64 {
    distance / NORMALISED_RADIUS / strain
}

fn jump_component(
    i: usize,
    distance: &[f64],
    strain: &[f64],
    angle: &[f64],
    requirement: f64,
    location: f64,
    flow: f64,
) -> f64 {
    if i == 0 {
        return 0.0;
    }
    let basis = jump_basis(distance[i], strain[i]);
    let angle_weight = if angle[i].is_nan() {
        1.0
    } else {
        1.0 + (-(angle[i].cos() * std::f64::consts::PI / 2.0).sin() + 1.0) / 10.0 * requirement
    };
    let mut pattern = 1.0;
    for back in 1..=2 {
        if i < back {
            break;
        }
        let j = i - back;
        let ratio = jump_basis(distance[i], strain[i]) / jump_basis(distance[j], strain[j]) - 1.0;
        let distance_weight = if distance[j] <= 0.0 {
            1.05
        } else {
            ratio_weight(ratio)
        };
        pattern *=
            (distance_weight * angle_step_weight(i, j, strain, angle)).powi((3 - back) as i32);
    }
    pattern = 1.0 + (pattern - 1.0) * requirement;
    basis * angle_weight * pattern * location * (1.0 - flow)
}

fn ratio_weight(ratio: f64) -> f64 {
    if ratio <= 0.0 {
        1.0 + ratio * ratio / 2.0
    } else if ratio < 1.0 {
        1.0 + (-(ratio * std::f64::consts::PI).cos() + 1.0) / 40.0
    } else {
        1.05
    }
}

fn angle_step_weight(i: usize, j: usize, strain: &[f64], angle: &[f64]) -> f64 {
    if !ratio_equal(strain[i], strain[j], 1.0) || angle[i].is_nan() || angle[j].is_nan() {
        return 1.0;
    }
    let delta = angle[i].abs() - angle[j].abs();
    if delta.abs() >= std::f64::consts::PI / 1.5 {
        return 1.05;
    }
    1.0 + (-((delta * 1.5).cos() * std::f64::consts::PI / 2.0).sin() + 1.0) / 40.0
}

fn flow_component(
    i: usize,
    distance: &[f64],
    strain: &[f64],
    angle: &[f64],
    flow: &[f64],
    location: f64,
) -> f64 {
    if i == 0 {
        return 0.0;
    }
    let basis = (((distance[i] / NORMALISED_RADIUS - 2.0).tanh() + 1.0) * 2.5
        + distance[i] / (5.0 * NORMALISED_RADIUS))
        / strain[i];
    let angle_weight = 1.0
        + (if angle[i].is_nan() {
            0.0
        } else {
            angle[i].cos()
        } + 1.0)
            / 10.0;
    basis
        * angle_weight
        * flow_pattern(i, distance, angle, flow)
        * (1.0 + (location - 1.0) / 2.0)
        * flow[i]
}

fn flow_pattern(i: usize, distance: &[f64], angle: &[f64], flow: &[f64]) -> f64 {
    let ratio = if distance[i - 1] > 0.0 {
        distance[i] / distance[i - 1] - 1.0
    } else {
        1.0
    };
    let distance_bonus = if ratio <= 0.0 {
        ratio * ratio
    } else if ratio < 1.0 {
        (-(ratio * std::f64::consts::PI).cos() + 1.0) / 2.0
    } else {
        1.0
    };
    let mut angle_bonus = 0.0;
    if !angle[i].is_nan() && !angle[i - 1].is_nan() && angle[i].abs() < angle[i - 1].abs() {
        angle_bonus =
            (-(((angle[i] - angle[i - 1]) / 2.0).sin() * std::f64::consts::PI).cos() + 1.0) / 2.0;
        if angle_bonus > 0.0 {
            let reversed = (-(((angle[i].abs() - angle[i - 1].abs()) / 2.0).sin()
                * std::f64::consts::PI)
                .cos()
                + 1.0)
                / 2.0;
            angle_bonus = angle_bonus.min(reversed);
        }
    }
    let blend = t2t(ratio, 0.0, 1.0);
    let jump = ((distance[i] / NORMALISED_RADIUS - 2.0).tanh() + 1.0) * 2.5
        + distance[i] / (5.0 * NORMALISED_RADIUS);
    let single = if distance[i] <= 0.0 {
        1.0
    } else {
        (1.0 - blend) + blend * (distance[i] / NORMALISED_RADIUS) / jump
    };
    let distance_weight = (1.0 + distance_bonus) * single;
    let angle_weight = 1.0 + angle_bonus * (1.0 - blend);
    1.0 + (distance_weight * angle_weight - 1.0) * flow[i - 1]
}

fn location_weight(previous: [f32; 2], current: [f32; 2]) -> f64 {
    let centre = [
        ((previous[0] + current[0]) * 0.5 - FIELD_WIDTH / 2.0) as f64,
        ((previous[1] + current[1]) * 0.5 - FIELD_HEIGHT / 2.0) as f64,
    ];
    let rotation = std::f64::consts::PI / 3.0;
    let a = (centre[0] * rotation.cos() + centre[1] * rotation.sin()) / 750.0;
    let b = (centre[0] * rotation.sin() - centre[1] * rotation.cos()) / 1000.0;
    1.0 + a * a + b * b
}

fn reading(
    i: usize,
    start: &[f64],
    base_flow: &[f64],
    distance: &[f64],
    preempt: f64,
    hidden: bool,
    flashlight: bool,
    raw_distance: f64,
    radius: f64,
) -> f64 {
    let mut reading_strain = 0.0;
    let mut k = i;
    while k > 0 {
        k -= 1;
        if start[i] - start[k] > preempt {
            break;
        }
        reading_strain += (1.0 - 0.75 * base_flow[k])
            * (1.0 + 0.5 * base_flow[k] * distance[k] / NORMALISED_RADIUS);
    }
    let bonus = reading_strain.max(0.0).powf(1.5) / 100.0;
    let mut value = if hidden {
        1.05 + 1.5 * bonus
    } else {
        1.0 + bonus
    };
    value *= 1.0 + (-((preempt - 325.0) / 30.0).tanh() + 1.0) / 15.0;
    if flashlight {
        value *= 1.0 + 0.3 * t2t(raw_distance, FIELD_HEIGHT as f64 / 4.0, radius);
    }
    value
}

fn cursor_end_of(note: &OsuPppObject) -> [f32; 2] {
    if note.kind != OsuPppObjectKind::Slider {
        return note.stacked_end_position;
    }
    let follow = (note.radius as f32).max(1.0) * 3.0;
    let mut cursor = note.stacked_position;
    for &sample in note.slider_positions_at_samples.iter().take(24) {
        let point = [
            note.stacked_position[0] + sample[0],
            note.stacked_position[1] + sample[1],
        ];
        let delta = [point[0] - cursor[0], point[1] - cursor[1]];
        let length = (delta[0] * delta[0] + delta[1] * delta[1]).sqrt();
        if length > follow && length > 0.001 {
            let factor = (length - follow) / length;
            cursor = [cursor[0] + delta[0] * factor, cursor[1] + delta[1] * factor];
        }
    }
    cursor
}

fn distance_f32(a: [f32; 2], b: [f32; 2]) -> f32 {
    let x = b[0] - a[0];
    let y = b[1] - a[1];
    (x * x + y * y).sqrt()
}

fn angle_of(
    previous_cursor: [f32; 2],
    previous_object: [f32; 2],
    current_cursor: [f32; 2],
    current: [f32; 2],
) -> f64 {
    let left = [
        previous_object[0] - previous_cursor[0],
        previous_object[1] - previous_cursor[1],
    ];
    let right = [
        current[0] - current_cursor[0],
        current[1] - current_cursor[1],
    ];
    let left_len = (left[0] * left[0] + left[1] * left[1]).sqrt();
    let right_len = (right[0] * right[0] + right[1] * right[1]).sqrt();
    if left_len < 0.001 || right_len < 0.001 {
        return f64::NAN;
    }
    ((left[0] * right[0] + left[1] * right[1]) / (left_len * right_len))
        .clamp(-1.0, 1.0)
        .acos() as f64
}

#[derive(Default)]
struct RhythmComplexity {
    queue: Vec<i32>,
    sum: f64,
    offbeat: bool,
}

impl RhythmComplexity {
    fn add(
        &mut self,
        i: usize,
        previous: Option<OsuPppObjectKind>,
        gap: &[f64],
        strain: &[f64],
        flow: f64,
        distance: f64,
    ) {
        let current = gap[i];
        let mut bonus = 0.05 * flow;
        let previous_gap = if previous.is_some() { gap[i - 1] } else { 0.0 };

        if previous == Some(OsuPppObjectKind::Circle) {
            if self.offbeat && ratio_greater(current, previous_gap, 1.5) {
                let mut value = 5.0;
                for &entry in &self.queue {
                    if entry > 0 {
                        value *= 1.0 - 0.5 * 0.9_f64.powi(i as i32 - entry);
                    } else if entry == -1 {
                        value = 5.0;
                    }
                }
                self.enqueue(i as i32);
                bonus += value;
            } else if ratio_equal(current, previous_gap, 2.0 / 3.0) {
                bonus += 4.0 + 8.0 * flow;
                if flow > 0.8 {
                    self.enqueue(-1);
                }
            } else if ratio_equal(current, previous_gap, 1.0 / 3.0) {
                bonus += 0.4 + 0.8 * flow;
            } else if ratio_equal(current, previous_gap, 0.5)
                || ratio_less(current, previous_gap, 0.25)
            {
                bonus += 0.1 + 0.2 * flow;
            }

            if ratio_less(current, previous_gap, 2.0 / 3.0) && flow > 0.8 {
                self.offbeat = true;
            } else if ratio_equal(current, previous_gap, 1.0) && flow > 0.8 {
                self.offbeat = !self.offbeat;
            } else {
                self.offbeat = false;
            }
        } else if previous == Some(OsuPppObjectKind::Slider) {
            let body = strain[i] - current;
            if body > 0.0 && (ratio_equal(current, body, 0.5) || ratio_equal(current, body, 0.25)) {
                let density = 15000.0 / current.max(1.0);
                let threshold = ((density - 140.0) / 20.0)
                    .tanh()
                    .mul_add(NORMALISED_RADIUS, 2.0 * NORMALISED_RADIUS);
                let expectation =
                    t2t(density, 120.0, 30.0) * t2f(distance, threshold, NORMALISED_RADIUS);
                bonus += 0.3 * expectation;
                self.offbeat = expectation > 0.8;
            } else {
                self.offbeat = false;
            }
        } else {
            self.offbeat = false;
        }
        self.sum += bonus;
    }

    fn reset(&mut self) {
        self.offbeat = false;
    }

    fn enqueue(&mut self, entry: i32) {
        if self.queue.len() == 10 {
            self.queue.remove(0);
        }
        self.queue.push(entry);
    }

    fn value(&self, circles: usize) -> f64 {
        if circles > 0 {
            1.0 + self.sum / circles as f64 * (circles as f64 / 50.0).tanh()
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle_with_params(time: f64, x: f32, y: f32, radius: f64, preempt: f64) -> OsuPppObject {
        OsuPppObject::circle(time, [x, y], radius, preempt)
    }

    fn circle(time: f64, x: f32, y: f32) -> OsuPppObject {
        circle_with_params(time, x, y, 40.0, 900.0)
    }

    fn slider(time: f64, end: f64, x: f32, y: f32, radius: f64, preempt: f64) -> OsuPppObject {
        OsuPppObject {
            kind: OsuPppObjectKind::Slider,
            start_time_ms: time,
            end_time_ms: end,
            radius,
            time_preempt_ms: preempt,
            stacked_position: [x, y],
            stacked_end_position: [x, y],
            slider_positions_at_samples: vec![[30.0, 0.0]; 24],
        }
    }

    fn spinner(time: f64, end: f64, x: f32, y: f32, radius: f64, preempt: f64) -> OsuPppObject {
        OsuPppObject {
            kind: OsuPppObjectKind::Spinner,
            start_time_ms: time,
            end_time_ms: end,
            radius,
            time_preempt_ms: preempt,
            stacked_position: [x, y],
            stacked_end_position: [256.0, 192.0],
            slider_positions_at_samples: Vec::new(),
        }
    }

    fn assert_golden(notes: &[OsuPppObject], mods: &[Mod], expected: [f64; 10]) {
        let actual = EzPPPOsu::calculate(notes, mods);
        assert!(actual.is_complete());
        for (index, (axis, value)) in actual.iter().enumerate() {
            assert!(
                (value - expected[index]).abs() <= 2e-5 * expected[index].abs().max(1e-12),
                "{axis:?}: expected {}, got {value}",
                expected[index]
            );
        }
    }

    #[test]
    fn fewer_than_three_objects_return_all_catalog_axes_as_zero() {
        let actual = EzPPPOsu::calculate(
            &[circle(0.0, 100.0, 100.0), circle(100.0, 200.0, 100.0)],
            &[],
        );
        assert_eq!(actual.as_slice(), &[0.0; 10]);
        assert_eq!(actual.as_slice().len(), 10);
    }

    #[test]
    fn straight_three_circle_sample_has_finite_ordered_axis_values() {
        let notes = [
            circle(0.0, 64.0, 192.0),
            circle(250.0, 256.0, 192.0),
            circle(500.0, 448.0, 192.0),
        ];
        let actual = EzPPPOsu::calculate(&notes, &[]);
        assert!(actual.is_complete());
        assert!(actual.get(EzPPPOSUSkillMetric::Aim) > 0.0);
        assert!(actual.get(EzPPPOSUSkillMetric::RawAim) <= actual.get(EzPPPOSUSkillMetric::Aim));
        assert_eq!(
            actual.get(EzPPPOSUSkillMetric::Accuracy),
            actual.get(EzPPPOSUSkillMetric::AccuracyDifficulty)
        );
        assert!(actual.get(EzPPPOSUSkillMetric::StarRating) > 0.0);
    }

    #[test]
    fn hidden_and_flashlight_only_change_reading_and_flashlight_metrics() {
        let notes = [
            circle(0.0, 64.0, 192.0),
            circle(125.0, 256.0, 100.0),
            circle(250.0, 448.0, 192.0),
        ];
        let normal = EzPPPOsu::calculate(&notes, &[]);
        let hidden = EzPPPOsu::calculate(&notes, &[Mod::Hidden]);
        let flashlight = EzPPPOsu::calculate(&notes, &[Mod::Flashlight]);
        assert!(hidden.get(EzPPPOSUSkillMetric::Aim) > normal.get(EzPPPOSUSkillMetric::Aim));
        assert!(flashlight.get(EzPPPOSUSkillMetric::Aim) > normal.get(EzPPPOSUSkillMetric::Aim));
    }

    #[test]
    fn invalid_numeric_input_returns_a_complete_zero_vector() {
        let notes = [
            circle(0.0, 64.0, 192.0),
            circle(100.0, 256.0, 192.0),
            circle(f64::NAN, 448.0, 192.0),
        ];
        assert_eq!(EzPPPOsu::calculate(&notes, &[]).as_slice(), &[0.0; 10]);
    }

    // Values captured from Ez2Lazer's C# EzPPPOsu / EzPPPMath implementation.
    #[test]
    fn csharp_golden_straight_and_rhythm_charts() {
        let straight = [
            circle(0.0, 64.0, 192.0),
            circle(250.0, 256.0, 192.0),
            circle(500.0, 448.0, 192.0),
        ];
        assert_golden(
            &straight,
            &[],
            [
                0.5105548186634796,
                0.43188335970944364,
                0.5105548186634796,
                0.0,
                0.2722920976939688,
                1.3827486598322658,
                0.8389774409370776,
                1.0009988017254858,
                2.2489145000959883,
                1.0009988017254858,
            ],
        );

        let rhythm = [
            circle(0.0, 64.0, 192.0),
            circle(500.0, 200.0, 100.0),
            circle(750.0, 400.0, 192.0),
            circle(875.0, 100.0, 300.0),
            circle(1000.0, 400.0, 192.0),
            circle(1500.0, 200.0, 100.0),
            circle(1750.0, 400.0, 192.0),
        ];
        assert_golden(
            &rhythm,
            &[],
            [
                1.32536621337321,
                1.1211403597667182,
                1.32536621337321,
                0.0,
                0.7068521013998575,
                1.5577251821646272,
                1.0797077116050167,
                1.006954622393923,
                2.9247280359387693,
                1.006954622393923,
            ],
        );
    }

    #[test]
    fn csharp_golden_slider_spinner_hidden_and_flashlight_charts() {
        let mixed = [
            circle_with_params(0.0, 64.0, 192.0, 24.0, 700.0),
            slider(250.0, 900.0, 200.0, 192.0, 30.0, 800.0),
            circle_with_params(950.0, 400.0, 100.0, 32.0, 600.0),
            spinner(1100.0, 1500.0, 256.0, 192.0, 20.0, 500.0),
            circle_with_params(1550.0, 100.0, 300.0, 40.0, 450.0),
        ];
        assert_golden(
            &mixed,
            &[],
            [
                0.5586896646291309,
                0.4139357357018521,
                0.558395113369024,
                0.040793706356812266,
                0.3752217318764696,
                1.4999346801739664,
                0.9931432706958848,
                1.0009988017254858,
                2.440542724976181,
                1.0009988017254858,
            ],
        );

        let visibility = [
            circle(0.0, 64.0, 192.0),
            circle(125.0, 256.0, 100.0),
            circle(250.0, 448.0, 192.0),
        ];
        assert_golden(
            &visibility,
            &[Mod::Hidden],
            [
                0.6647117790942212,
                0.5622862538936775,
                0.6647117790942212,
                0.0,
                0.35450799419592716,
                1.1556966511166662,
                0.6969596750896698,
                1.0009988017254858,
                1.959651779997383,
                1.0009988017254858,
            ],
        );
        assert_golden(
            &visibility,
            &[Mod::Flashlight],
            [
                0.7382830958729758,
                0.6245209568230011,
                0.7382830958729758,
                0.0,
                0.39374548142254134,
                1.1556966511166662,
                0.6969596750896698,
                1.0009988017254858,
                1.997564563075894,
                1.0009988017254858,
            ],
        );
        assert_golden(
            &visibility,
            &[Mod::Hidden, Mod::Flashlight],
            [
                0.757888035297158,
                0.6411049685606256,
                0.757888035297158,
                0.0,
                0.40420130298338397,
                1.1556966511166662,
                0.6969596750896698,
                1.0009988017254858,
                2.0087645745299283,
                1.0009988017254858,
            ],
        );
    }
}
