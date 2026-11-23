use super::{DiffObject, Kind};

fn reverse_lerp(x: f64, start: f64, end: f64) -> f64 {
    ((x - start) / (end - start)).clamp(0.0, 1.0)
}

fn doubletap(object: &DiffObject<'_>, next: Option<&DiffObject<'_>>) -> f64 {
    let Some(next) = next else {
        return 0.0;
    };
    let current_delta = object.delta.max(1.0);
    let next_delta = next.delta.max(1.0);
    let difference = (next_delta - current_delta).abs();
    let speed_ratio = current_delta / current_delta.max(difference);
    let window_ratio = (current_delta / object.great_window).min(1.0).powi(5);
    let distance_factor = reverse_lerp(object.lazy_jump, 100.0, 50.0).powi(2);
    1.0 - speed_ratio.powf(distance_factor * (1.0 - window_ratio))
}

pub(super) fn speed(objects: &[DiffObject<'_>], index: usize) -> f64 {
    let current = &objects[index];
    if current.object.kind == Kind::Spinner {
        return 0.0;
    }
    let mut time = current.adjusted_delta;
    let feasibility = 1.0 - doubletap(current, objects.get(index + 1));
    time /= ((time / current.great_window) / 0.93).clamp(0.92, 1.0);
    let speed_bonus = if time < 75.0 {
        0.75 * ((75.0 - time) / 40.0).powi(2)
    } else {
        0.0
    };
    let difficulty = (1.0 + speed_bonus) * 1000.0 / time;
    difficulty / (1.0 - 0.3_f64.powf(current.adjusted_delta / 1000.0)) * feasibility
}

#[derive(Clone, Copy)]
struct Island {
    delta: i32,
    count: u32,
    occurrences: u32,
}

impl Island {
    fn new(delta: i32) -> Self {
        Self {
            delta: delta.max(25),
            count: 1,
            occurrences: 1,
        }
    }

    fn equal(self, other: Self, epsilon: f64) -> bool {
        f64::from((self.delta - other.delta).abs()) < epsilon && self.count == other.count
    }

    fn similar_polarity(self, other: Self, epsilon: f64) -> bool {
        self.count > 1
            && other.count > 1
            && f64::from((self.delta - other.delta).abs()) < epsilon
            && self.count % 2 == other.count % 2
    }
}

pub(super) fn rhythm(objects: &[DiffObject<'_>], index: usize) -> f64 {
    let current = &objects[index];
    if current.object.kind == Kind::Spinner {
        return 0.0;
    }
    let historical_notes = index.min(32);
    if historical_notes < 3 {
        return 1.0;
    }
    let mut rhythm_start = 0;
    while rhythm_start < historical_notes - 2
        && current.start_time - objects[index - rhythm_start - 1].start_time < 5000.0
    {
        rhythm_start += 1;
    }
    if rhythm_start == 0 {
        return 1.0;
    }
    let epsilon = current.great_window * 0.3;
    let mut island = Island::new(i32::MAX);
    let mut previous_island = Island::new(i32::MAX);
    // Track existing island identities by index because C# Island values are reference types.
    let mut islands = Vec::<Island>::new();
    let mut start_difficulty = 0.0;
    let mut switched = false;
    let mut complexity = 0.0;
    let mut previous = &objects[index - rhythm_start - 1];
    let mut previous_previous = &objects[index - rhythm_start - 2];
    for i in (1..=rhythm_start).rev() {
        let object = &objects[index - i];
        if object.object.kind == Kind::Spinner {
            continue;
        }
        let time_decay = (5000.0 - (current.start_time - object.start_time)) / 5000.0;
        let note_decay = (historical_notes - i) as f64 / historical_notes as f64;
        let historical_decay = note_decay.min(time_decay);
        let delta = object.delta.max(1e-7);
        let previous_delta = previous.delta.max(1e-7);
        let difference = (previous_delta - delta).abs();
        if island.delta == i32::MAX {
            island = Island::new(delta as i32);
        }
        let ratio = previous_delta.max(delta) / previous_delta.min(delta);
        let multiplier = (2.0 - ratio / 8.0).clamp(0.0, 1.0);
        let window_penalty = ((difference - epsilon) / epsilon).clamp(0.0, 1.0);
        let mut effective = ratio_difficulty(ratio) * window_penalty * multiplier;
        if previous.object.kind == Kind::Slider {
            let lazy = object.min_jump_time;
            let real = object.last_end_delta;
            let lazy_ratio = lazy.max(delta) / lazy.min(delta);
            let real_ratio = real.max(delta) / real.min(delta);
            effective =
                effective.min(ratio_difficulty(lazy_ratio).min(ratio_difficulty(real_ratio)));
        }
        if difference < epsilon {
            island.count += 1;
        }
        if switched {
            if difference > epsilon {
                if object.object.kind == Kind::Slider {
                    effective *= 0.5;
                }
                if island.similar_polarity(previous_island, epsilon) {
                    effective *= 0.5;
                }
                if previous_previous.delta.max(1e-7) > previous_delta + epsilon
                    && previous_delta > delta + epsilon
                {
                    effective *= 0.125;
                }
                if previous_island.count == island.count {
                    effective *= 0.5;
                }
                if previous_delta > delta + epsilon {
                    effective *= 0.65;
                }
                if let Some(existing) = islands
                    .iter_mut()
                    .find(|existing| existing.equal(island, epsilon))
                {
                    if previous_island.equal(island, epsilon) {
                        existing.occurrences += 1;
                    }
                    let occurrences = f64::from(existing.occurrences);
                    let exponent = 2.75 / (1.0 + (0.24 * (58.33 - f64::from(island.delta))).exp());
                    effective *= (3.0 / occurrences).min((1.0 / occurrences).powf(exponent));
                } else {
                    islands.push(island);
                }
                effective *= 1.0 - doubletap(previous, Some(object)) * 0.75;
                complexity += if island.count > 1 {
                    (effective * start_difficulty).sqrt() * historical_decay
                } else {
                    0.7 * historical_decay
                };
                start_difficulty = effective;
                if previous_delta + epsilon < delta {
                    switched = false;
                }
                previous_island = island;
                island = Island::new(delta as i32);
            }
        } else if previous_delta > delta + epsilon {
            switched = true;
            if object.object.kind == Kind::Slider {
                effective *= 0.6;
            }
            if previous.object.kind == Kind::Slider {
                effective *= 0.6;
            }
            start_difficulty = effective;
            island = Island::new(delta as i32);
        }
        previous_previous = previous;
        previous = object;
    }
    complexity *= reverse_lerp(f64::from(island.count), 22.0, 3.0);
    (4.0 + 0.95 * complexity).sqrt() / 2.0
}

fn ratio_difficulty(ratio: f64) -> f64 {
    let value = (1.0 - 2.0 * (ratio.fract() - 0.5).abs()).clamp(0.0, 1.0);
    1.0 + 26.0 * (value * value * (3.0 - 2.0 * value)).min(0.5)
}
