use serde::{Deserialize, Serialize};

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

        let mut previous = Tick(i64::MIN);
        for note in &self.notes {
            if note.tick < previous {
                return Err("notes must be sorted by tick".into());
            }
            let note_end = u32::from(note.lane) + u32::from(note.width);
            if note_end > u32::from(self.header.lane_count) {
                return Err(format!("note {} is outside the lane range", note.id));
            }
            previous = note.tick;
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
#[serde(tag = "type", rename_all = "camelCase")]
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
}
