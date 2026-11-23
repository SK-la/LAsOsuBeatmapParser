//! Legacy slider scoring events shared by the osu! and Catch rulesets.

use std::fmt;

pub const TAIL_LENIENCY: f64 = -36.0;
const MAX_TICK_PATH_LENGTH: f64 = 100_000.0;
const MAX_EVENTS: usize = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliderEventType {
    Tick,
    LegacyLastTick,
    Head,
    Tail,
    Repeat,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderEventDescriptor {
    pub event_type: SliderEventType,
    pub time: f64,
    pub span_index: u32,
    pub span_start_time: f64,
    pub path_progress: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliderEventError {
    InvalidStartTime,
    InvalidSpanDuration,
    InvalidVelocity,
    InvalidTickDistance,
    InvalidTotalDistance,
    InvalidEndMargin,
    InvalidSpanCount,
    TimeOverflow,
    TooManyEvents,
}

impl fmt::Display for SliderEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStartTime => f.write_str("slider start time must be finite"),
            Self::InvalidSpanDuration => {
                f.write_str("slider span duration must be finite and nonnegative")
            }
            Self::InvalidVelocity => f.write_str("slider velocity must be finite and nonnegative"),
            Self::InvalidTickDistance => f.write_str("slider tick distance must not be NaN"),
            Self::InvalidTotalDistance => {
                f.write_str("slider total distance must be finite and nonnegative")
            }
            Self::InvalidEndMargin => {
                f.write_str("slider end margin must be finite and nonnegative")
            }
            Self::InvalidSpanCount => f.write_str("a slider must have at least one span"),
            Self::TimeOverflow => f.write_str("slider event time calculation overflowed"),
            Self::TooManyEvents => write!(f, "a slider can generate at most {MAX_EVENTS} events"),
        }
    }
}

impl std::error::Error for SliderEventError {}

pub struct SliderEventGenerator;

impl SliderEventGenerator {
    /// Generates events using the default end margin of ten milliseconds of travel.
    pub fn generate(
        start_time: f64,
        span_duration: f64,
        velocity: f64,
        tick_distance: f64,
        total_distance: f64,
        span_count: u32,
    ) -> Result<Vec<SliderEventDescriptor>, SliderEventError> {
        if !velocity.is_finite() || velocity < 0.0 {
            return Err(SliderEventError::InvalidVelocity);
        }

        Self::generate_with_end_margin(
            start_time,
            span_duration,
            tick_distance,
            total_distance,
            span_count,
            velocity * 10.0,
            false,
        )
    }

    /// Generates ticks in traversal order, followed by the legacy last tick and tail.
    ///
    /// The legacy last tick is appended after the span events even when its timestamp precedes
    /// some of them. On a zero-duration slider its progress is NaN, as in Catch conversion.
    /// Positive infinite tick distance disables ticks. Inputs that would produce over one
    /// million events return an error.
    pub fn generate_with_end_margin(
        start_time: f64,
        span_duration: f64,
        tick_distance: f64,
        total_distance: f64,
        span_count: u32,
        min_distance_from_end: f64,
        skip_repeats_on_short_spans: bool,
    ) -> Result<Vec<SliderEventDescriptor>, SliderEventError> {
        validate_start_and_span_count(start_time, span_count)?;
        if !span_duration.is_finite() || span_duration < 0.0 {
            return Err(SliderEventError::InvalidSpanDuration);
        }
        if tick_distance.is_nan() {
            return Err(SliderEventError::InvalidTickDistance);
        }
        if !total_distance.is_finite() || total_distance < 0.0 {
            return Err(SliderEventError::InvalidTotalDistance);
        }
        if !min_distance_from_end.is_finite() || min_distance_from_end < 0.0 {
            return Err(SliderEventError::InvalidEndMargin);
        }

        let final_span_index = span_count - 1;
        let total_duration = f64::from(span_count) * span_duration;
        let final_span_start_time = start_time + f64::from(final_span_index) * span_duration;
        let final_span_end_time = final_span_start_time + span_duration;
        let tail_time = start_time + total_duration;
        if !tail_time.is_finite() || !final_span_end_time.is_finite() {
            return Err(SliderEventError::TimeOverflow);
        }

        let length = total_distance.min(MAX_TICK_PATH_LENGTH);
        let tick_distance = tick_distance.clamp(0.0, length);
        let include_repeats =
            !skip_repeats_on_short_spans || length > tick_distance + min_distance_from_end;
        let non_tick_count = 3 + if include_repeats {
            u64::from(final_span_index)
        } else {
            0
        };
        if non_tick_count > MAX_EVENTS as u64 {
            return Err(SliderEventError::TooManyEvents);
        }
        let mut progress_values = Vec::new();
        if tick_distance != 0.0 {
            let max_ticks_per_span = (MAX_EVENTS as u64 - non_tick_count) / u64::from(span_count);
            let tick_end = length - min_distance_from_end;
            let mut distance = tick_distance;
            while distance < tick_end {
                if progress_values.len() as u64 >= max_ticks_per_span {
                    return Err(SliderEventError::TooManyEvents);
                }
                progress_values.push(distance / length);
                distance += tick_distance;
            }
        }

        let total_count = non_tick_count + progress_values.len() as u64 * u64::from(span_count);
        let mut events = Vec::with_capacity(total_count as usize);
        events.push(SliderEventDescriptor {
            event_type: SliderEventType::Head,
            time: start_time,
            span_index: 0,
            span_start_time: start_time,
            path_progress: 0.0,
        });

        if !progress_values.is_empty() || (include_repeats && final_span_index != 0) {
            for span_index in 0..span_count {
                let span_start_time = start_time + f64::from(span_index) * span_duration;
                let reversed = span_index % 2 == 1;
                let tick = |path_progress| SliderEventDescriptor {
                    event_type: SliderEventType::Tick,
                    time: span_start_time
                        + if reversed {
                            1.0 - path_progress
                        } else {
                            path_progress
                        } * span_duration,
                    span_index,
                    span_start_time,
                    path_progress,
                };
                if reversed {
                    events.extend(progress_values.iter().rev().copied().map(tick));
                } else {
                    events.extend(progress_values.iter().copied().map(tick));
                }

                if include_repeats && span_index < final_span_index {
                    events.push(SliderEventDescriptor {
                        event_type: SliderEventType::Repeat,
                        time: span_start_time + span_duration,
                        span_index,
                        span_start_time,
                        path_progress: f64::from((span_index + 1) % 2),
                    });
                }
            }
        }

        let legacy_last_tick_time =
            (start_time + total_duration / 2.0).max(final_span_end_time + TAIL_LENIENCY);
        let mut legacy_progress = (legacy_last_tick_time - final_span_start_time) / span_duration;
        if span_count % 2 == 0 {
            legacy_progress = 1.0 - legacy_progress;
        }
        events.push(SliderEventDescriptor {
            event_type: SliderEventType::LegacyLastTick,
            time: legacy_last_tick_time,
            span_index: final_span_index,
            span_start_time: final_span_start_time,
            path_progress: legacy_progress,
        });
        events.push(SliderEventDescriptor {
            event_type: SliderEventType::Tail,
            time: tail_time,
            span_index: final_span_index,
            span_start_time: final_span_start_time,
            path_progress: f64::from(span_count % 2),
        });
        Ok(events)
    }

    /// Generates the head and tail used by osu! when slider duration collapses to zero.
    pub fn degenerate(
        start_time: f64,
        span_count: u32,
    ) -> Result<[SliderEventDescriptor; 2], SliderEventError> {
        validate_start_and_span_count(start_time, span_count)?;
        Ok([
            SliderEventDescriptor {
                event_type: SliderEventType::Head,
                time: start_time,
                span_index: 0,
                span_start_time: start_time,
                path_progress: 0.0,
            },
            SliderEventDescriptor {
                event_type: SliderEventType::Tail,
                time: start_time,
                span_index: span_count - 1,
                span_start_time: start_time,
                path_progress: 0.0,
            },
        ])
    }
}

fn validate_start_and_span_count(start_time: f64, span_count: u32) -> Result<(), SliderEventError> {
    if !start_time.is_finite() {
        return Err(SliderEventError::InvalidStartTime);
    }
    if span_count == 0 {
        return Err(SliderEventError::InvalidSpanCount);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use SliderEventType::{Head, LegacyLastTick, Repeat, Tail, Tick};

    #[test]
    fn reverse_spans_keep_tick_positions_and_reverse_their_traversal_order() {
        let events = SliderEventGenerator::generate(100.0, 100.0, 1.0, 25.0, 100.0, 2).unwrap();
        let shape: Vec<_> = events
            .iter()
            .map(|event| (event.event_type, event.span_index, event.time))
            .collect();
        assert_eq!(
            shape,
            [
                (Head, 0, 100.0),
                (Tick, 0, 125.0),
                (Tick, 0, 150.0),
                (Tick, 0, 175.0),
                (Repeat, 0, 200.0),
                (Tick, 1, 225.0),
                (Tick, 1, 250.0),
                (Tick, 1, 275.0),
                (LegacyLastTick, 1, 264.0),
                (Tail, 1, 300.0),
            ]
        );
        assert_eq!(events[4].path_progress, 1.0);
        assert_eq!(events[5].path_progress, events[3].path_progress);
        assert_eq!(events[6].path_progress, events[2].path_progress);
        assert_eq!(events[7].path_progress, events[1].path_progress);
        assert!((events[8].path_progress - 0.36).abs() < 1e-15);
        assert_eq!(events[9].path_progress, 0.0);
        assert!(
            events[5..]
                .iter()
                .all(|event| event.span_start_time == 200.0)
        );
    }

    #[test]
    fn exact_end_margin_boundary_excludes_ticks_and_can_exclude_repeats() {
        let default = SliderEventGenerator::generate(0.0, 100.0, 1.0, 90.0, 100.0, 2).unwrap();
        let shape: Vec<_> = default.iter().map(|event| event.event_type).collect();
        assert_eq!(shape, [Head, Repeat, LegacyLastTick, Tail]);

        let stable =
            SliderEventGenerator::generate_with_end_margin(0.0, 100.0, 90.0, 100.0, 2, 10.0, true)
                .unwrap();
        let shape: Vec<_> = stable.iter().map(|event| event.event_type).collect();
        assert_eq!(shape, [Head, LegacyLastTick, Tail]);

        let below_boundary =
            SliderEventGenerator::generate_with_end_margin(0.0, 100.0, 90.0, 100.0, 2, 9.0, true)
                .unwrap();
        let shape: Vec<_> = below_boundary
            .iter()
            .map(|event| event.event_type)
            .collect();
        assert_eq!(shape, [Head, Tick, Repeat, Tick, LegacyLastTick, Tail]);
    }

    #[test]
    fn short_sliders_limit_the_legacy_offset_to_half_the_duration() {
        let events =
            SliderEventGenerator::generate(100.0, 20.0, 1.0, f64::INFINITY, 20.0, 3).unwrap();
        assert_eq!(events[2].event_type, Repeat);
        assert_eq!(events[2].time, 140.0);
        let last_tick = events[3];
        assert_eq!(last_tick.event_type, LegacyLastTick);
        assert_eq!(last_tick.time, 130.0);
        assert_eq!(last_tick.span_start_time, 140.0);
        assert_eq!(last_tick.path_progress, -0.5);
    }

    #[test]
    fn zero_duration_retains_shared_generator_and_osu_degenerate_semantics() {
        let events = SliderEventGenerator::generate(123.0, 0.0, 1.0, 20.0, 0.0, 3).unwrap();
        let shape: Vec<_> = events.iter().map(|event| event.event_type).collect();
        assert_eq!(shape, [Head, Repeat, Repeat, LegacyLastTick, Tail]);
        assert!(events.iter().all(|event| event.time == 123.0));
        assert!(events[3].path_progress.is_nan());

        let [head, tail] = SliderEventGenerator::degenerate(123.0, 3).unwrap();
        assert_eq!(head.event_type, Head);
        assert_eq!(tail.event_type, Tail);
        assert_eq!(head.time, tail.time);
        assert_eq!(tail.span_index, 2);
        assert_eq!(tail.path_progress, 0.0);
    }

    #[test]
    fn caps_path_length_and_rejects_pathological_finite_event_counts_and_times() {
        let events =
            SliderEventGenerator::generate(0.0, 100.0, 1.0, 50_000.0, f64::MAX, 1).unwrap();
        assert_eq!(events[1].event_type, Tick);
        assert_eq!(events[1].time, 50.0);
        assert_eq!(events[1].path_progress, 0.5);

        assert_eq!(
            SliderEventGenerator::generate(0.0, 100.0, 1.0, f64::MIN_POSITIVE, 100.0, 9_000),
            Err(SliderEventError::TooManyEvents)
        );
        assert_eq!(
            SliderEventGenerator::generate(0.0, 100.0, 1.0, 0.0, 0.0, u32::MAX),
            Err(SliderEventError::TooManyEvents)
        );
        assert_eq!(
            SliderEventGenerator::generate(f64::MAX, f64::MAX, 1.0, 0.0, 0.0, 2),
            Err(SliderEventError::TimeOverflow)
        );

        let collapsed =
            SliderEventGenerator::generate_with_end_margin(0.0, 0.0, 0.0, 0.0, u32::MAX, 0.0, true)
                .unwrap();
        assert_eq!(collapsed.len(), 3);
        assert_eq!(collapsed[2].span_index, u32::MAX - 1);
    }
}
