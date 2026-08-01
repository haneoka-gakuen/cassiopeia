use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tick(pub i64);

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimeMicros(pub i64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RoundingProfile {
    ExactRational,
    CompatibilityFloat32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TempoEvent {
    pub tick: Tick,
    pub micros_per_quarter: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TempoSegment {
    event: TempoEvent,
    start: TimeMicros,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TempoMap {
    ppq: u32,
    profile: RoundingProfile,
    segments: Vec<TempoSegment>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TempoMapError {
    InvalidPpq,
    InvalidTempo(usize),
    Unsorted(usize),
    Overflow,
}

impl fmt::Display for TempoMapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPpq => formatter.write_str("PPQ must be between 1 and 15360"),
            Self::InvalidTempo(index) => write!(formatter, "tempo event {index} is invalid"),
            Self::Unsorted(index) => write!(formatter, "tempo event {index} is out of order"),
            Self::Overflow => formatter.write_str("tempo conversion overflowed"),
        }
    }
}

impl std::error::Error for TempoMapError {}

impl TempoMap {
    pub fn new(
        ppq: u32,
        mut events: Vec<TempoEvent>,
        profile: RoundingProfile,
    ) -> Result<Self, TempoMapError> {
        if ppq == 0 || ppq > 15_360 {
            return Err(TempoMapError::InvalidPpq);
        }
        if events.is_empty() {
            events.push(TempoEvent {
                tick: Tick(0),
                micros_per_quarter: 500_000,
            });
        }
        for (index, event) in events.iter().enumerate() {
            if event.micros_per_quarter == 0 || event.micros_per_quarter > 60_000_000 {
                return Err(TempoMapError::InvalidTempo(index));
            }
            if index > 0 && event.tick < events[index - 1].tick {
                return Err(TempoMapError::Unsorted(index));
            }
        }
        if events[0].tick > Tick(0) {
            events.insert(
                0,
                TempoEvent {
                    tick: Tick(0),
                    micros_per_quarter: 500_000,
                },
            );
        }

        let mut segments = Vec::with_capacity(events.len());
        let mut current = TimeMicros(0);
        for event in events {
            if let Some(previous) = segments.last().copied() {
                current = add_delta(previous, event.tick, ppq, profile)?;
            }
            segments.push(TempoSegment {
                event,
                start: current,
            });
        }

        let origin_index = segments.partition_point(|segment| segment.event.tick <= Tick(0));
        let origin_segment = segments[origin_index.saturating_sub(1)];
        let origin_time = add_delta(origin_segment, Tick(0), ppq, profile)?;
        if origin_time != TimeMicros(0) {
            for segment in &mut segments {
                segment.start = TimeMicros(
                    segment
                        .start
                        .0
                        .checked_sub(origin_time.0)
                        .ok_or(TempoMapError::Overflow)?,
                );
            }
        }
        Ok(Self {
            ppq,
            profile,
            segments,
        })
    }

    pub fn time_at_tick(&self, tick: Tick) -> Result<TimeMicros, TempoMapError> {
        let index = self
            .segments
            .partition_point(|segment| segment.event.tick <= tick);
        let segment = self.segments[index.saturating_sub(1)];
        add_delta(segment, tick, self.ppq, self.profile)
    }
}

fn add_delta(
    segment: TempoSegment,
    tick: Tick,
    ppq: u32,
    profile: RoundingProfile,
) -> Result<TimeMicros, TempoMapError> {
    let delta = tick
        .0
        .checked_sub(segment.event.tick.0)
        .ok_or(TempoMapError::Overflow)?;
    let micros = match profile {
        RoundingProfile::ExactRational => {
            let numerator = i128::from(delta) * i128::from(segment.event.micros_per_quarter);
            let rounded = numerator.div_euclid(i128::from(ppq));
            i64::try_from(rounded).map_err(|_| TempoMapError::Overflow)?
        }
        RoundingProfile::CompatibilityFloat32 => {
            let value = (delta as f64 * f64::from(segment.event.micros_per_quarter as f32))
                / f64::from(ppq as f32);
            if !value.is_finite() || value < i64::MIN as f64 || value > i64::MAX as f64 {
                return Err(TempoMapError::Overflow);
            }
            value.floor() as i64
        }
    };
    Ok(TimeMicros(
        segment
            .start
            .0
            .checked_add(micros)
            .ok_or(TempoMapError::Overflow)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrates_multiple_tempo_segments_without_wall_clock_state() {
        let map = TempoMap::new(
            480,
            vec![
                TempoEvent {
                    tick: Tick(0),
                    micros_per_quarter: 500_000,
                },
                TempoEvent {
                    tick: Tick(480),
                    micros_per_quarter: 1_000_000,
                },
            ],
            RoundingProfile::ExactRational,
        )
        .unwrap();
        assert_eq!(map.time_at_tick(Tick(480)).unwrap(), TimeMicros(500_000));
        assert_eq!(map.time_at_tick(Tick(960)).unwrap(), TimeMicros(1_500_000));
    }

    #[test]
    fn tempo_before_zero_is_anchored_to_tick_zero() {
        let map = TempoMap::new(
            480,
            vec![TempoEvent {
                tick: Tick(-480),
                micros_per_quarter: 500_000,
            }],
            RoundingProfile::ExactRational,
        )
        .unwrap();

        assert_eq!(map.time_at_tick(Tick(-480)).unwrap(), TimeMicros(-500_000));
        assert_eq!(map.time_at_tick(Tick(0)).unwrap(), TimeMicros(0));
    }
}
