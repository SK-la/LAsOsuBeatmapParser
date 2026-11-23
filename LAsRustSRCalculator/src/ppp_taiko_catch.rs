//! Ez2Lazer PP+ chart skills for Taiko and Catch.

use crate::catch::CatchObjectInput;
use crate::skills::{EzPPPCatchSkillAxis, EzPPPTaikoSkillAxis, SkillVector};
use crate::taiko::{TaikoHitType, TaikoNote, TaikoObjectType};

const TAIKO_MULT: f64 = 0.084375;
const RHYTHM_TABLE: [(f64, f64); 9] = [
    (1.0, 0.0),
    (2.0, 0.3),
    (0.5, 0.5),
    (3.0, 0.3),
    (1.0 / 3.0, 0.35),
    (1.5, 0.6),
    (2.0 / 3.0, 0.4),
    (1.25, 0.5),
    (0.8, 0.7),
];

#[derive(Clone, Copy)]
struct RhythmEntry {
    index: usize,
    rho: f64,
}

#[derive(Clone, Copy)]
struct Unit {
    start: usize,
    end: usize,
    kind: Option<TaikoHitType>,
    length: usize,
}

#[derive(Clone)]
struct Pattern {
    first: usize,
    last: usize,
    start: usize,
    lengths: Vec<usize>,
    kind: Option<TaikoHitType>,
}

#[derive(Clone, Copy)]
struct Repeating {
    first: usize,
    last: usize,
    start: usize,
    interval: usize,
}

pub struct EzPPPTaiko;

impl EzPPPTaiko {
    pub fn calculate(objects: &[TaikoNote], clock_rate: f64) -> SkillVector<EzPPPTaikoSkillAxis> {
        if objects.len() < 3 {
            return SkillVector::zeros();
        }
        let rate = if clock_rate > 0.0 { clock_rate } else { 1.0 };
        let rhythm_values = taiko_rhythm(objects, rate);
        let colour_values = taiko_colour(objects);
        let stamina_values = taiko_stamina(objects, rate);
        let mut times = Vec::with_capacity(objects.len() - 2);
        let mut rh = Vec::with_capacity(objects.len() - 2);
        let mut ch = Vec::with_capacity(objects.len() - 2);
        let mut sh = Vec::with_capacity(objects.len() - 2);
        let (mut r, mut c, mut s) = (0.0, 0.0, 0.0);
        let mut previous_time = objects[1].start_time_ms;
        for i in 2..objects.len() {
            let delta = ((objects[i].start_time_ms - previous_time) / rate).max(0.0);
            r = 0.96 * r + 10.0 * rhythm_values[i];
            c = 0.8_f64.powf(delta / 1000.0) * c + 0.12 * colour_values[i];
            s = 0.4_f64.powf(delta / 1000.0) * s + 1.1 * stamina_values[i];
            times.push(objects[i].start_time_ms / rate);
            rh.push(r);
            ch.push(c);
            sh.push(s);
            previous_time = objects[i].start_time_ms;
        }
        let rv = aggregate(&times, &rh, 0.0);
        let cv = aggregate(&times, &ch, 0.8);
        let sv = aggregate(&times, &sh, 0.4);
        let star = combined_star(&times, &ch, &rh, &sh);
        SkillVector::from_fn(|axis| match axis {
            EzPPPTaikoSkillAxis::Rhythm => finite(0.2 * TAIKO_MULT * rv),
            EzPPPTaikoSkillAxis::Colour => finite(0.375 * TAIKO_MULT * cv),
            EzPPPTaikoSkillAxis::Stamina => finite(0.375 * TAIKO_MULT * sv),
            EzPPPTaikoSkillAxis::StarRating => finite(star),
        })
    }
}

fn taiko_rhythm(objects: &[TaikoNote], rate: f64) -> Vec<f64> {
    let mut result = vec![0.0; objects.len()];
    let mut history = Vec::<RhythmEntry>::new();
    let mut rolling = 0.0;
    let mut since = 0;
    for i in 2..objects.len() {
        if objects[i].object_type != TaikoObjectType::Hit {
            rolling = 0.0;
            since = 0;
            continue;
        }
        since += 1;
        let delta = ((objects[i].start_time_ms - objects[i - 1].start_time_ms) / rate).max(0.0);
        let previous =
            ((objects[i - 1].start_time_ms - objects[i - 2].start_time_ms) / rate).max(0.0);
        let ratio = if previous > 0.0 {
            delta / previous
        } else {
            1.0
        };
        let (rho, weight) = rhythm_match(ratio);
        rolling *= 0.96;
        if delta >= 210.0 {
            rolling = 0.0;
            since = 0;
            continue;
        }
        if weight <= 0.0 {
            continue;
        }
        if history.len() == 8 {
            history.remove(0);
        }
        history.push(RhythmEntry { index: i, rho });
        let repetition = rhythm_repetition(&history, i);
        let n = since as f64;
        let length = (0.15 * n).min((2.5 - 0.15 * n).clamp(0.0, 1.0));
        let speed = if delta < 80.0 {
            1.0
        } else {
            (1.4 - 0.005 * delta).max(0.0)
        };
        rolling += weight * repetition * length * speed;
        since = 0;
        result[i] = rolling;
    }
    result
}

fn rhythm_match(ratio: f64) -> (f64, f64) {
    RHYTHM_TABLE
        .iter()
        .copied()
        .min_by(|a, b| (ratio - a.0).abs().total_cmp(&(ratio - b.0).abs()))
        .unwrap()
}

fn rhythm_repetition(history: &[RhythmEntry], object_index: usize) -> f64 {
    for length in 2..=4 {
        if history.len() < length * 2 {
            continue;
        }
        let tail = history.len() - length;
        for start in (0..=tail - length).rev() {
            if (0..length).all(|k| (history[start + k].rho - history[tail + k].rho).abs() < 1e-9) {
                return (0.032 * (object_index - history[start].index) as f64).min(1.0);
            }
        }
    }
    1.0
}

fn taiko_colour(objects: &[TaikoNote]) -> Vec<f64> {
    let mut result = vec![0.0; objects.len()];
    let mut streaks = Vec::<Unit>::new();
    let mut current = None::<Unit>;
    for i in 2..objects.len() {
        if objects[i].object_type != TaikoObjectType::Hit {
            if let Some(unit) = current.take() {
                streaks.push(unit);
            }
            streaks.push(Unit {
                start: i,
                end: i,
                kind: None,
                length: 1,
            });
            continue;
        }
        let kind = objects[i].hit_type;
        if current.is_some_and(|unit| unit.end + 1 == i && unit.kind == kind) {
            current = current.map(|unit| Unit {
                end: i,
                length: unit.length + 1,
                ..unit
            });
        } else {
            if let Some(unit) = current.take() {
                streaks.push(unit);
            }
            current = Some(Unit {
                start: i,
                end: i,
                kind,
                length: 1,
            });
        }
    }
    if let Some(unit) = current {
        streaks.push(unit);
    }

    let mut patterns = Vec::<Pattern>::new();
    let mut index = 0;
    while index < streaks.len() {
        let mut last = index;
        while last + 1 < streaks.len() && streaks[last + 1].length == streaks[index].length {
            last += 1;
        }
        patterns.push(Pattern {
            first: index,
            last,
            start: streaks[index].start,
            lengths: streaks[index..=last]
                .iter()
                .map(|unit| unit.length)
                .collect(),
            kind: streaks[index].kind,
        });
        index = last + 1;
    }

    let mut repeats = Vec::<Repeating>::new();
    let mut index = 0;
    while index < patterns.len() {
        let mut members = Vec::new();
        let coupled =
            index + 2 < patterns.len() && strict_repeat(&patterns[index], &patterns[index + 2]);
        let last;
        if !coupled {
            members.push(index);
            last = index;
            index += 1;
        } else {
            let mut continues = coupled;
            while continues {
                members.push(index);
                index += 1;
                continues = index + 2 < patterns.len()
                    && strict_repeat(&patterns[index], &patterns[index + 2]);
            }
            // The lazer implementation includes the two final alternating patterns after a coupled run.
            members.push(index);
            members.push(index + 1);
            last = index + 1;
            // The C# for-loop then increments once more after this manual increment.
            index += 2;
        }
        let interval = repeats
            .iter()
            .rev()
            .take(15)
            .position(|old| repetition_match(&patterns, members[0], last, old.first, old.last))
            .map_or(17, |back| back + 1);
        repeats.push(Repeating {
            first: members[0],
            last,
            start: patterns[members[0]].start,
            interval,
        });
    }

    for repetition in &repeats {
        let parent = 2.0 * (1.0 - color_sigmoid(repetition.interval as f64));
        result[repetition.start] += parent;
        for pattern_index in repetition.first..=repetition.last {
            let pattern = &patterns[pattern_index];
            let position = (pattern_index - repetition.first + 1) as f64;
            let alternating = color_sigmoid(position) * parent;
            result[pattern.start] += alternating;
            for streak_index in pattern.first..=pattern.last {
                let streak_position = (streak_index - pattern.first + 1) as f64;
                result[streaks[streak_index].start] +=
                    0.5 * color_sigmoid(streak_position) * alternating;
            }
        }
    }
    result
}

fn strict_repeat(a: &Pattern, b: &Pattern) -> bool {
    a.lengths.len() == b.lengths.len() && a.kind == b.kind && a.lengths[0] == b.lengths[0]
}

fn repetition_match(
    patterns: &[Pattern],
    a_first: usize,
    a_last: usize,
    b_first: usize,
    b_last: usize,
) -> bool {
    if a_last - a_first != b_last - b_first {
        return false;
    }
    let count = (a_last - a_first + 1).min(2);
    (0..count).all(|i| patterns[a_first + i].lengths[0] == patterns[b_first + i].lengths[0])
}

fn color_sigmoid(x: f64) -> f64 {
    0.5 + 0.5 * (std::f64::consts::E * -(x - 2.0) / 2.0).tanh()
}

fn taiko_stamina(objects: &[TaikoNote], rate: f64) -> Vec<f64> {
    let mut result = vec![0.0; objects.len()];
    let mut history = [Vec::<f64>::new(), Vec::<f64>::new()];
    for i in 2..objects.len() {
        let note = objects[i];
        if note.object_type != TaikoObjectType::Hit {
            continue;
        }
        let colour = match note.hit_type.unwrap() {
            TaikoHitType::Centre => 0,
            TaikoHitType::Rim => 1,
        };
        let fingers = if colour_changed_near(objects, i, rate) {
            2
        } else {
            4
        };
        let back = fingers - 1;
        if history[colour].len() >= back {
            let interval = ((note.start_time_ms - history[colour][history[colour].len() - back])
                / rate)
                .max(0.0);
            result[i] = 0.5 + 30.0 / interval.max(1.0);
        }
        history[colour].push(note.start_time_ms);
    }
    result
}

fn colour_changed_near(objects: &[TaikoNote], index: usize, rate: f64) -> bool {
    let hit = objects[index];
    (index.saturating_sub(8)..(index + 8).min(objects.len()))
        .filter(|j| *j != index)
        .any(|j| {
            objects[j].object_type == TaikoObjectType::Hit
                && objects[j].hit_type != hit.hit_type
                && ((objects[j].start_time_ms - hit.start_time_ms) / rate).max(0.0) < 300.0
        })
}

fn aggregate(times: &[f64], strains: &[f64], section_beta: f64) -> f64 {
    if times.is_empty() {
        return 0.0;
    }
    let mut peaks = Vec::new();
    let mut section_end = (times[0] / 400.0).ceil() * 400.0;
    let mut peak = 0.0;
    for i in 0..times.len() {
        while times[i] > section_end {
            peaks.push(peak);
            peak = if section_beta > 0.0 && i > 0 {
                strains[i - 1] * section_beta.powf((section_end - times[i - 1]) / 1000.0)
            } else {
                0.0
            };
            section_end += 400.0;
        }
        if strains[i].is_finite() && strains[i] > peak {
            peak = strains[i];
        }
    }
    peaks.push(peak);
    peaks.retain(|p| *p > 0.0 && p.is_finite());
    peaks.sort_by(|a, b| b.total_cmp(a));
    peaks
        .iter()
        .enumerate()
        .map(|(i, p)| p * 0.9_f64.powi(i as i32))
        .sum()
}

fn combined_star(times: &[f64], colour: &[f64], rhythm: &[f64], stamina: &[f64]) -> f64 {
    fn section_peaks(times: &[f64], strains: &[f64], beta: f64) -> Vec<f64> {
        let mut result = Vec::new();
        if times.is_empty() {
            return result;
        }
        let mut end = (times[0] / 400.0).ceil() * 400.0;
        let mut peak = 0.0;
        for i in 0..times.len() {
            while times[i] > end {
                result.push(peak);
                peak = if beta > 0.0 && i > 0 {
                    strains[i - 1] * beta.powf((end - times[i - 1]) / 1000.0)
                } else {
                    0.0
                };
                end += 400.0;
            }
            peak = peak.max(strains[i]);
        }
        result.push(peak);
        result
    }
    let c = section_peaks(times, colour, 0.8);
    let r = section_peaks(times, rhythm, 0.0);
    let s = section_peaks(times, stamina, 0.4);
    let count = c.len().max(r.len()).max(s.len());
    let mut combined = Vec::new();
    for i in 0..count {
        let c = c.get(i).copied().unwrap_or(0.0) * 0.375 * TAIKO_MULT;
        let r = r.get(i).copied().unwrap_or(0.0) * 0.2 * TAIKO_MULT;
        let s = s.get(i).copied().unwrap_or(0.0) * 0.375 * TAIKO_MULT;
        let mechanical = (c.abs().powf(1.5) + s.abs().powf(1.5)).powf(1.0 / 1.5);
        let value = (mechanical.powi(2) + r.powi(2)).sqrt();
        if value > 0.0 {
            combined.push(value);
        }
    }
    combined.sort_by(|a, b| b.total_cmp(a));
    let rating: f64 = combined
        .iter()
        .enumerate()
        .map(|(i, v)| v * 0.9_f64.powi(i as i32))
        .sum();
    let inside = 1.4 * rating / 8.0 + 1.0;
    if inside <= 0.0 {
        rating
    } else {
        10.43 * inside.ln()
    }
}

fn decay(delta: f64, beta: f64) -> f64 {
    if delta <= 0.0 {
        1.0
    } else {
        beta.powf(delta / 1000.0)
    }
}
fn finite(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

pub struct EzPPPCatch;

impl EzPPPCatch {
    pub fn calculate(
        objects: &[CatchObjectInput],
        half_width: f32,
        clock_rate: f64,
    ) -> SkillVector<EzPPPCatchSkillAxis> {
        if objects.len() < 2 {
            return SkillVector::zeros();
        }
        let rate = if clock_rate > 0.0 { clock_rate } else { 1.0 };
        let half_width = half_width.max(1.0);
        let mut state = 0.0_f64;
        let mut previous_player = objects[0].effective_x * (41.0 / half_width);
        let mut previous_delta = 0.0_f64;
        let mut previous_strain = 40.0_f64;
        let (mut times, mut strains) = (Vec::new(), Vec::new());
        for i in 1..objects.len() {
            let x = objects[i].effective_x * (41.0 / half_width);
            let mut player = previous_player.clamp(x - 25.0, x + 25.0);
            if objects[i - 1].hyper_dash {
                player = x;
            }
            let delta = f64::from(player - previous_player);
            let strain_time =
                ((objects[i].start_time_ms - objects[i - 1].start_time_ms) / rate).max(40.0);
            let time_weight = strain_time + 13.0 + 3.0 / rate;
            let mut addition = f64::from(delta.abs()).powf(1.3) / 510.0
                + 12.5 * f64::from(delta.abs().min(82.0)) / 246.0 / time_weight.sqrt();
            if delta.signum() != previous_delta.signum()
                && delta.abs() > 0.1
                && previous_delta.abs() > 0.1
            {
                addition += 21.0 / (previous_strain + 16.0).sqrt() * delta.abs().min(50.0) / 50.0
                    * (previous_delta.abs().min(70.0) / 70.0).max(0.38)
                    * (1.0 - (time_weight / 1000.0).powi(3)).max(0.0);
            }
            let edge = if objects[i - 1].distance_to_hyper_dash <= 20.0 {
                let bonus = if objects[i - 1].hyper_dash { 0.0 } else { 5.7 };
                1.0 + bonus * f64::from(20.0 - objects[i - 1].distance_to_hyper_dash) / 20.0
                    * ((strain_time * rate).min(265.0) / 265.0).powf(1.5)
            } else {
                1.0
            };
            let strain = addition / time_weight * edge;
            state = decay(strain_time, 0.2) * state + strain;
            times.push(objects[i].start_time_ms / rate);
            strains.push(state);
            previous_player = player;
            previous_delta = delta;
            previous_strain = strain_time;
        }
        let star = 4.59 * aggregate(&times, &strains, 0.2).max(0.0).sqrt();
        SkillVector::from_fn(|axis| match axis {
            EzPPPCatchSkillAxis::Movement => finite(star),
            EzPPPCatchSkillAxis::StarRating => finite(star),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn note(time: f64, kind: Option<TaikoHitType>) -> TaikoNote {
        TaikoNote {
            start_time_ms: time,
            end_time_ms: time,
            effective_bpm: 120.0,
            object_type: if kind.is_some() {
                TaikoObjectType::Hit
            } else {
                TaikoObjectType::DrumRoll
            },
            hit_type: kind,
            is_strong: false,
            max_combo_contribution: 1,
        }
    }
    #[test]
    fn taiko_empty_chart_returns_zero() {
        let v = EzPPPTaiko::calculate(&[], 1.0);
        assert_eq!(v.get(EzPPPTaikoSkillAxis::StarRating), 0.0);
    }
    #[test]
    fn taiko_rate_changes_a_coloured_pattern() {
        let c: Vec<_> = (0..24)
            .map(|i| {
                note(
                    i as f64 * if i % 4 == 0 { 160.0 } else { 100.0 },
                    Some(if i % 2 == 0 {
                        TaikoHitType::Centre
                    } else {
                        TaikoHitType::Rim
                    }),
                )
            })
            .collect();
        let a = EzPPPTaiko::calculate(&c, 1.0);
        let b = EzPPPTaiko::calculate(&c, 1.5);
        assert!(a.get(EzPPPTaikoSkillAxis::Rhythm) > 0.0);
        assert!(a.get(EzPPPTaikoSkillAxis::Colour) > 0.0);
        assert!(a.get(EzPPPTaikoSkillAxis::Stamina) > 0.0);
        assert!(a.get(EzPPPTaikoSkillAxis::StarRating) > 0.0);
        assert_ne!(
            a.get(EzPPPTaikoSkillAxis::StarRating),
            b.get(EzPPPTaikoSkillAxis::StarRating)
        );
    }
    #[test]
    fn taiko_rolls_break_colours_and_rhythm() {
        let mut c: Vec<_> = (0..14)
            .map(|i| note(i as f64 * 100.0, Some(TaikoHitType::Centre)))
            .collect();
        c[5] = note(500.0, None);
        let v = EzPPPTaiko::calculate(&c, 1.0);
        assert!(v.is_complete());
    }
    #[test]
    fn catch_single_movement_matches_hand_formula() {
        let c = [
            CatchObjectInput {
                start_time_ms: 0.0,
                effective_x: 0.0,
                hyper_dash: false,
                distance_to_hyper_dash: 100.0,
            },
            CatchObjectInput {
                start_time_ms: 100.0,
                effective_x: 41.0,
                hyper_dash: false,
                distance_to_hyper_dash: 100.0,
            },
        ];
        let v = EzPPPCatch::calculate(&c, 41.0, 1.0);
        let d = 16.0_f64;
        let w = 116.0_f64;
        let strain = (d.powf(1.3) / 510.0 + 12.5 * d / 246.0 / w.sqrt()) / w;
        assert!((v.get(EzPPPCatchSkillAxis::StarRating) - 4.59 * strain.sqrt()).abs() < 1e-14);
    }
}
