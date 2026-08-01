use haneoka_cassiopeia_core::{
    CassiopeiaChart, ChartEvent, ChartNote, NoteKind, RoundingProfile, TempoEvent, TempoMap,
    TempoMapError, Tick, TimeMicros,
};
use serde::Serialize;
use std::fmt;

pub const SUMMARY_FORMAT: &str = "org.haneoka.cassiopeia.conformance-summary";
pub const SUMMARY_VERSION: u32 = 1;
pub const VALIDATION_SCOPE: &str = "ccfV1StructuralAndExactTiming";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConformanceSummary {
    pub summary_format: &'static str,
    pub summary_version: u32,
    pub validation_scope: &'static str,
    pub chart: ChartSummary,
    pub timing: TimingSummary,
    pub events: EventCounts,
    pub notes: NoteCounts,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSummary {
    pub format: String,
    pub version: u32,
    pub title: String,
    pub ppq: u32,
    pub lane_count: u16,
    pub compatibility_profile: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimingSummary {
    pub rounding_profile: RoundingProfile,
    pub tempo_segments: Vec<TempoSegmentSummary>,
    pub first_note: Option<NoteAnchor>,
    pub last_note: Option<NoteAnchor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TempoSegmentSummary {
    pub tick: Tick,
    pub micros_per_quarter: u32,
    pub time_micros: TimeMicros,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteAnchor {
    pub id: String,
    pub tick: Tick,
    pub time_micros: TimeMicros,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventCounts {
    pub total: usize,
    pub tempo: usize,
    pub meter: usize,
    pub time_scale: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteCounts {
    pub total: usize,
    pub tap: usize,
    pub flick: usize,
    pub trace: usize,
    pub hold_start: usize,
    pub hold_tick: usize,
    pub hold_end: usize,
    pub guide: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SummaryError {
    InvalidChart(String),
    InvalidTiming(TempoMapError),
}

impl fmt::Display for SummaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChart(message) => write!(formatter, "invalid chart: {message}"),
            Self::InvalidTiming(error) => write!(formatter, "invalid timing: {error}"),
        }
    }
}

impl std::error::Error for SummaryError {}

impl From<TempoMapError> for SummaryError {
    fn from(error: TempoMapError) -> Self {
        Self::InvalidTiming(error)
    }
}

pub fn summarize(
    chart: &CassiopeiaChart,
    rounding_profile: RoundingProfile,
) -> Result<ConformanceSummary, SummaryError> {
    chart.validate().map_err(SummaryError::InvalidChart)?;

    let source_tempos = chart
        .events
        .iter()
        .filter_map(|event| match event {
            ChartEvent::Tempo {
                tick,
                micros_per_quarter,
            } => Some(TempoEvent {
                tick: *tick,
                micros_per_quarter: *micros_per_quarter,
            }),
            ChartEvent::Meter { .. } | ChartEvent::TimeScale { .. } => None,
        })
        .collect::<Vec<_>>();
    let tempo_map = TempoMap::new(chart.header.ppq, source_tempos.clone(), rounding_profile)?;
    let effective_tempos = effective_tempos(source_tempos);

    let tempo_segments = effective_tempos
        .into_iter()
        .map(|event| {
            Ok(TempoSegmentSummary {
                tick: event.tick,
                micros_per_quarter: event.micros_per_quarter,
                time_micros: tempo_map.time_at_tick(event.tick)?,
            })
        })
        .collect::<Result<Vec<_>, TempoMapError>>()?;

    let first_note = chart
        .notes
        .first()
        .map(|note| note_anchor(note, &tempo_map))
        .transpose()?;
    let last_note = chart
        .notes
        .last()
        .map(|note| note_anchor(note, &tempo_map))
        .transpose()?;

    Ok(ConformanceSummary {
        summary_format: SUMMARY_FORMAT,
        summary_version: SUMMARY_VERSION,
        validation_scope: VALIDATION_SCOPE,
        chart: ChartSummary {
            format: chart.format.clone(),
            version: chart.version,
            title: chart.header.title.clone(),
            ppq: chart.header.ppq,
            lane_count: chart.header.lane_count,
            compatibility_profile: chart.header.compatibility_profile.clone(),
        },
        timing: TimingSummary {
            rounding_profile,
            tempo_segments,
            first_note,
            last_note,
        },
        events: count_events(&chart.events),
        notes: count_notes(&chart.notes),
    })
}

fn effective_tempos(mut events: Vec<TempoEvent>) -> Vec<TempoEvent> {
    if events.is_empty() || events[0].tick > Tick(0) {
        events.insert(
            0,
            TempoEvent {
                tick: Tick(0),
                micros_per_quarter: 500_000,
            },
        );
    }
    events
}

fn note_anchor(note: &ChartNote, tempo_map: &TempoMap) -> Result<NoteAnchor, TempoMapError> {
    Ok(NoteAnchor {
        id: note.id.clone(),
        tick: note.tick,
        time_micros: tempo_map.time_at_tick(note.tick)?,
    })
}

fn count_events(events: &[ChartEvent]) -> EventCounts {
    let mut counts = EventCounts {
        total: events.len(),
        ..EventCounts::default()
    };
    for event in events {
        match event {
            ChartEvent::Tempo { .. } => counts.tempo += 1,
            ChartEvent::Meter { .. } => counts.meter += 1,
            ChartEvent::TimeScale { .. } => counts.time_scale += 1,
        }
    }
    counts
}

fn count_notes(notes: &[ChartNote]) -> NoteCounts {
    let mut counts = NoteCounts {
        total: notes.len(),
        ..NoteCounts::default()
    };
    for note in notes {
        match note.kind {
            NoteKind::Tap => counts.tap += 1,
            NoteKind::Flick => counts.flick += 1,
            NoteKind::Trace => counts.trace += 1,
            NoteKind::HoldStart => counts.hold_start += 1,
            NoteKind::HoldTick => counts.hold_tick += 1,
            NoteKind::HoldEnd => counts.hold_end += 1,
            NoteKind::Guide => counts.guide += 1,
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_uses_the_shared_tempo_map() {
        let chart: CassiopeiaChart =
            serde_json::from_str(include_str!("../../../fixtures/conformance/basic.ccf.json"))
                .unwrap();

        let summary = summarize(&chart, RoundingProfile::ExactRational).unwrap();

        assert_eq!(
            summary.timing.tempo_segments[1].time_micros,
            TimeMicros(1_000_000)
        );
        assert_eq!(
            summary.timing.last_note.unwrap().time_micros,
            TimeMicros(1_800_000)
        );
        assert_eq!(summary.notes.total, 4);
    }
}
