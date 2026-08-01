use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::timing::Tick;

pub const FORMAT_ID: &str = "org.haneoka.cassiopeia.chart";
pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CassiopeiaChart {
    pub format: String,
    pub version: u32,
    pub header: ChartHeader,
    #[serde(default)]
    pub events: Vec<ChartEvent>,
    #[serde(default)]
    pub notes: Vec<ChartNote>,
}

impl CassiopeiaChart {
    pub fn validate(&self) -> Result<(), String> {
        if self.format != FORMAT_ID {
            return Err(format!("unsupported chart format: {}", self.format));
        }
        if self.version != FORMAT_VERSION {
            return Err(format!("unsupported chart version: {}", self.version));
        }
        if self.header.ppq == 0 || self.header.ppq > 15_360 {
            return Err("header.ppq must be between 1 and 15360".into());
        }
        if self.header.lane_count == 0 || self.header.lane_count > 256 {
            return Err("header.laneCount must be between 1 and 256".into());
        }

        let mut previous_event = Tick(i64::MIN);
        for event in &self.events {
            let tick = match event {
                ChartEvent::Tempo {
                    tick,
                    micros_per_quarter,
                } => {
                    if *micros_per_quarter == 0 || *micros_per_quarter > 60_000_000 {
                        return Err("tempo microsPerQuarter must be between 1 and 60000000".into());
                    }
                    *tick
                }
                ChartEvent::Meter {
                    tick,
                    numerator,
                    denominator,
                } => {
                    if *numerator == 0 || *denominator == 0 {
                        return Err("meter numerator and denominator must be positive".into());
                    }
                    *tick
                }
                ChartEvent::TimeScale { tick, .. } => *tick,
            };
            if tick < previous_event {
                return Err("events must be sorted by tick".into());
            }
            previous_event = tick;
        }

        let mut note_ids = HashSet::with_capacity(self.notes.len());
        let mut previous = Tick(i64::MIN);
        for note in &self.notes {
            if note.tick < previous {
                return Err("notes must be sorted by tick".into());
            }
            if note.id.is_empty() {
                return Err("note IDs must not be empty".into());
            }
            if !note_ids.insert(note.id.as_str()) {
                return Err(format!("duplicate note ID: {}", note.id));
            }
            if note.width == 0 {
                return Err(format!("note {} has zero width", note.id));
            }
            let note_end = u32::from(note.lane) + u32::from(note.width);
            if note_end > u32::from(self.header.lane_count) {
                return Err(format!("note {} is outside the lane range", note.id));
            }
            previous = note.tick;
        }
        for note in &self.notes {
            for link in &note.links {
                if !note_ids.contains(link.as_str()) {
                    return Err(format!("note {} links to unknown note {link}", note.id));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChartHeader {
    pub title: String,
    pub artist: String,
    pub author: String,
    pub ppq: u32,
    pub lane_count: u16,
    #[serde(default)]
    pub compatibility_profile: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ChartEvent {
    Tempo {
        tick: Tick,
        micros_per_quarter: u32,
    },
    Meter {
        tick: Tick,
        numerator: u16,
        denominator: u16,
    },
    TimeScale {
        tick: Tick,
        scale_milli: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChartNote {
    pub id: String,
    pub tick: Tick,
    pub lane: u16,
    pub width: u16,
    pub kind: NoteKind,
    #[serde(default)]
    pub critical: bool,
    #[serde(default)]
    pub links: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoteKind {
    Tap,
    Flick,
    Trace,
    HoldStart,
    HoldTick,
    HoldEnd,
    Guide,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_notes_outside_the_lane_range() {
        let chart = CassiopeiaChart {
            format: FORMAT_ID.into(),
            version: FORMAT_VERSION,
            header: ChartHeader {
                title: "Fixture".into(),
                artist: "Fixture".into(),
                author: "Cassiopeia".into(),
                ppq: 480,
                lane_count: 24,
                compatibility_profile: Some("our-notes-2026".into()),
            },
            events: vec![],
            notes: vec![ChartNote {
                id: "n1".into(),
                tick: Tick(0),
                lane: 23,
                width: 2,
                kind: NoteKind::Tap,
                critical: false,
                links: vec![],
            }],
        };

        assert!(chart.validate().is_err());
    }

    #[test]
    fn rejects_overflowing_note_ranges_without_panicking() {
        let chart = CassiopeiaChart {
            format: FORMAT_ID.into(),
            version: FORMAT_VERSION,
            header: ChartHeader {
                title: "Fixture".into(),
                artist: "Fixture".into(),
                author: "Cassiopeia".into(),
                ppq: 480,
                lane_count: 24,
                compatibility_profile: None,
            },
            events: vec![],
            notes: vec![ChartNote {
                id: "overflow".into(),
                tick: Tick(0),
                lane: u16::MAX,
                width: u16::MAX,
                kind: NoteKind::Tap,
                critical: false,
                links: vec![],
            }],
        };

        assert!(chart.validate().is_err());
    }

    #[test]
    fn json_contract_is_explicitly_versioned() {
        let value = serde_json::json!({
            "format": FORMAT_ID,
            "version": 1,
            "header": {
                "title": "Fixture",
                "artist": "Fixture",
                "author": "Cassiopeia",
                "ppq": 480,
                "laneCount": 24
            },
            "events": [],
            "notes": []
        });
        let chart: CassiopeiaChart = serde_json::from_value(value).unwrap();
        assert_eq!(chart.version, FORMAT_VERSION);
        assert!(chart.validate().is_ok());
    }

    #[test]
    fn chart_event_fields_use_the_public_camel_case_contract() {
        let event: ChartEvent = serde_json::from_value(serde_json::json!({
            "type": "tempo",
            "tick": 0,
            "microsPerQuarter": 500_000
        }))
        .unwrap();

        assert_eq!(
            event,
            ChartEvent::Tempo {
                tick: Tick(0),
                micros_per_quarter: 500_000
            }
        );
    }

    #[test]
    fn rejects_unknown_event_fields() {
        let value = serde_json::json!({
            "type": "tempo",
            "tick": 0,
            "microsPerQuarter": 500_000,
            "unexpected": true
        });
        assert!(serde_json::from_value::<ChartEvent>(value).is_err());
    }

    #[test]
    fn rejects_zero_width_duplicate_ids_and_unknown_links() {
        let mut chart = fixture_chart();
        chart.notes[0].width = 0;
        assert_eq!(chart.validate().unwrap_err(), "note n1 has zero width");

        chart.notes[0].width = 1;
        chart.notes.push(chart.notes[0].clone());
        assert_eq!(chart.validate().unwrap_err(), "duplicate note ID: n1");

        chart.notes.pop();
        chart.notes[0].links.push("missing".into());
        assert_eq!(
            chart.validate().unwrap_err(),
            "note n1 links to unknown note missing"
        );
    }

    fn fixture_chart() -> CassiopeiaChart {
        CassiopeiaChart {
            format: FORMAT_ID.into(),
            version: FORMAT_VERSION,
            header: ChartHeader {
                title: "Fixture".into(),
                artist: "Fixture".into(),
                author: "Cassiopeia".into(),
                ppq: 480,
                lane_count: 24,
                compatibility_profile: None,
            },
            events: vec![],
            notes: vec![ChartNote {
                id: "n1".into(),
                tick: Tick(0),
                lane: 0,
                width: 1,
                kind: NoteKind::Tap,
                critical: false,
                links: vec![],
            }],
        }
    }
}
