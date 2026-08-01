use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::assist::AssistLevel;
use crate::judgement::NoteJudgementType;
use crate::scoring::NoteOperateType;
use crate::timing::TimeMicros;

pub const RUNTIME_CHART_FORMAT: &str = "org.haneoka.cassiopeia.runtime";
pub const RUNTIME_CHART_VERSION: u32 = 1;
pub const RUNTIME_LANE_COUNT: u16 = 24;
pub const LANE_UNITS_PER_LANE: i32 = 1_000_000;
pub const AREA_OFFSETS_PER_LEVEL: usize = 9;

/// Fixed-point lane coordinate. One lane is one million units.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LanePosition(pub i32);

impl LanePosition {
    pub const HALF_LANE: Self = Self(LANE_UNITS_PER_LANE / 2);

    pub fn from_integer_lanes(lanes: i32) -> Result<Self, RuntimeChartError> {
        lanes
            .checked_mul(LANE_UNITS_PER_LANE)
            .map(Self)
            .ok_or(RuntimeChartError::CoordinateOverflow)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(i8)]
#[serde(rename_all = "camelCase")]
pub enum JudgementAreaOffsetType {
    Default = 0,
    Slide = 1,
    SlideBegin = 2,
    SlideEnd = 3,
    Flick = 4,
    Trace = 5,
    SlideMin = 6,
    SlideMax = 7,
    EasyDefault = 8,
    EasySlideBegin = 9,
    EnumMax = 10,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(i8)]
#[serde(rename_all = "camelCase")]
pub enum NoteDirection {
    Normal = 0,
    Left = 1,
    Right = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeLineKind {
    Long,
    Guide,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RuntimeNoteV1 {
    pub id: u32,
    pub time: TimeMicros,
    pub position: LanePosition,
    pub size: LanePosition,
    pub operate_type: NoteOperateType,
    pub judgement_type: NoteJudgementType,
    pub judgement_area_offset_type: JudgementAreaOffsetType,
    pub direction: NoteDirection,
    pub judged: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RuntimeLineV1 {
    pub id: u32,
    pub kind: RuntimeLineKind,
    pub note_ids: Vec<u32>,
}

/// Renderer-independent, versioned input chart for the gameplay kernel.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RuntimeChartV1 {
    pub format: String,
    pub version: u32,
    pub lane_count: u16,
    #[serde(default)]
    pub assist_level: AssistLevel,
    #[serde(default)]
    pub notes: Vec<RuntimeNoteV1>,
    #[serde(default)]
    pub lines: Vec<RuntimeLineV1>,
}

impl RuntimeChartV1 {
    pub fn validate(&self) -> Result<(), RuntimeChartError> {
        if self.format != RUNTIME_CHART_FORMAT {
            return Err(RuntimeChartError::UnsupportedFormat(self.format.clone()));
        }
        if self.version != RUNTIME_CHART_VERSION {
            return Err(RuntimeChartError::UnsupportedVersion(self.version));
        }
        if self.lane_count != RUNTIME_LANE_COUNT {
            return Err(RuntimeChartError::UnsupportedLaneCount(self.lane_count));
        }

        let lane_end = i64::from(self.lane_count) * i64::from(LANE_UNITS_PER_LANE);
        let mut note_times = BTreeMap::new();
        let mut previous_time = TimeMicros(i64::MIN);
        for (index, note) in self.notes.iter().enumerate() {
            if note.time < previous_time {
                return Err(RuntimeChartError::UnsortedNotes(index));
            }
            if note_times.insert(note.id, note.time).is_some() {
                return Err(RuntimeChartError::DuplicateNoteId(note.id));
            }
            if note.size.0 <= 0 {
                return Err(RuntimeChartError::InvalidNoteSize(note.id));
            }
            let start = i64::from(note.position.0);
            let end = start
                .checked_add(i64::from(note.size.0))
                .ok_or(RuntimeChartError::CoordinateOverflow)?;
            if start < 0 || end > lane_end {
                return Err(RuntimeChartError::NoteOutsideLaneRange(note.id));
            }
            previous_time = note.time;
        }

        let mut line_ids = BTreeSet::new();
        for line in &self.lines {
            if !line_ids.insert(line.id) {
                return Err(RuntimeChartError::DuplicateLineId(line.id));
            }
            if line.note_ids.is_empty() {
                return Err(RuntimeChartError::EmptyLine(line.id));
            }
            let mut members = BTreeSet::new();
            let mut previous_member_time = TimeMicros(i64::MIN);
            for note_id in &line.note_ids {
                if !members.insert(*note_id) {
                    return Err(RuntimeChartError::DuplicateLineNote {
                        line_id: line.id,
                        note_id: *note_id,
                    });
                }
                let time = *note_times
                    .get(note_id)
                    .ok_or(RuntimeChartError::UnknownLineNote {
                        line_id: line.id,
                        note_id: *note_id,
                    })?;
                if time < previous_member_time {
                    return Err(RuntimeChartError::UnsortedLine(line.id));
                }
                previous_member_time = time;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeChartError {
    UnsupportedFormat(String),
    UnsupportedVersion(u32),
    UnsupportedLaneCount(u16),
    UnsortedNotes(usize),
    DuplicateNoteId(u32),
    InvalidNoteSize(u32),
    NoteOutsideLaneRange(u32),
    DuplicateLineId(u32),
    EmptyLine(u32),
    UnknownLineNote { line_id: u32, note_id: u32 },
    DuplicateLineNote { line_id: u32, note_id: u32 },
    UnsortedLine(u32),
    CoordinateOverflow,
}

impl fmt::Display for RuntimeChartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormat(format) => {
                write!(formatter, "unsupported runtime format: {format}")
            }
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported runtime version: {version}")
            }
            Self::UnsupportedLaneCount(count) => {
                write!(
                    formatter,
                    "RuntimeChartV1 requires {RUNTIME_LANE_COUNT} lanes, got {count}"
                )
            }
            Self::UnsortedNotes(index) => {
                write!(formatter, "runtime note {index} is out of time order")
            }
            Self::DuplicateNoteId(id) => write!(formatter, "duplicate runtime note ID: {id}"),
            Self::InvalidNoteSize(id) => {
                write!(formatter, "runtime note {id} has a non-positive size")
            }
            Self::NoteOutsideLaneRange(id) => {
                write!(formatter, "runtime note {id} is outside the lane range")
            }
            Self::DuplicateLineId(id) => write!(formatter, "duplicate runtime line ID: {id}"),
            Self::EmptyLine(id) => write!(formatter, "runtime line {id} has no notes"),
            Self::UnknownLineNote { line_id, note_id } => {
                write!(
                    formatter,
                    "runtime line {line_id} refers to unknown note {note_id}"
                )
            }
            Self::DuplicateLineNote { line_id, note_id } => {
                write!(formatter, "runtime line {line_id} repeats note {note_id}")
            }
            Self::UnsortedLine(id) => write!(formatter, "runtime line {id} is out of time order"),
            Self::CoordinateOverflow => formatter.write_str("runtime lane coordinate overflowed"),
        }
    }
}

impl std::error::Error for RuntimeChartError {}

pub const AREA_OFFSETS: [LanePosition; crate::assist::ASSIST_LEVEL_COUNT * AREA_OFFSETS_PER_LEVEL] = [
    // Assist level 0.
    LanePosition(1000000),
    LanePosition(2000000),
    LanePosition(3000000),
    LanePosition(2000000),
    LanePosition(1000000),
    LanePosition(3000000),
    LanePosition(2800000),
    LanePosition(2800000),
    LanePosition(2800000),
    // Assist level 1.
    LanePosition(1100000),
    LanePosition(2100000),
    LanePosition(3100000),
    LanePosition(2100000),
    LanePosition(1100000),
    LanePosition(3100000),
    LanePosition(2850000),
    LanePosition(2850000),
    LanePosition(2850000),
    // Assist level 2.
    LanePosition(1200000),
    LanePosition(2200000),
    LanePosition(3200000),
    LanePosition(2200000),
    LanePosition(1200000),
    LanePosition(3200000),
    LanePosition(2900000),
    LanePosition(2900000),
    LanePosition(2900000),
    // Assist level 3.
    LanePosition(1300000),
    LanePosition(2300000),
    LanePosition(3300000),
    LanePosition(2300000),
    LanePosition(1300000),
    LanePosition(3300000),
    LanePosition(2950000),
    LanePosition(2950000),
    LanePosition(2950000),
    // Assist level 4.
    LanePosition(1400000),
    LanePosition(2400000),
    LanePosition(3400000),
    LanePosition(2400000),
    LanePosition(1400000),
    LanePosition(3400000),
    LanePosition(3000000),
    LanePosition(3000000),
    LanePosition(3000000),
    // Assist level 5.
    LanePosition(1500000),
    LanePosition(2500000),
    LanePosition(3500000),
    LanePosition(2500000),
    LanePosition(1500000),
    LanePosition(3500000),
    LanePosition(3100000),
    LanePosition(3100000),
    LanePosition(3100000),
];

pub fn area_offset(
    assist: AssistLevel,
    area_type: JudgementAreaOffsetType,
) -> Option<LanePosition> {
    let offset = match area_type {
        JudgementAreaOffsetType::Default => 0,
        JudgementAreaOffsetType::SlideBegin => 1,
        JudgementAreaOffsetType::SlideEnd => 2,
        JudgementAreaOffsetType::SlideMin => 3,
        JudgementAreaOffsetType::SlideMax => 4,
        JudgementAreaOffsetType::Flick => 5,
        JudgementAreaOffsetType::Trace => 6,
        JudgementAreaOffsetType::EasyDefault => 7,
        JudgementAreaOffsetType::EasySlideBegin => 8,
        JudgementAreaOffsetType::Slide | JudgementAreaOffsetType::EnumMax => return None,
    };
    Some(AREA_OFFSETS[assist.index() * AREA_OFFSETS_PER_LEVEL + offset])
}

pub fn judgement_area_offset(
    area_type: JudgementAreaOffsetType,
    note_size: LanePosition,
) -> LanePosition {
    judgement_area_offset_with_assist(AssistLevel::Level0, area_type, note_size)
}

pub fn judgement_area_offset_with_assist(
    assist: AssistLevel,
    area_type: JudgementAreaOffsetType,
    note_size: LanePosition,
) -> LanePosition {
    match area_type {
        JudgementAreaOffsetType::Slide => {
            let minimum = area_offset(assist, JudgementAreaOffsetType::SlideMin)
                .expect("every assist profile defines SlideMin");
            let maximum = area_offset(assist, JudgementAreaOffsetType::SlideMax)
                .expect("every assist profile defines SlideMax");
            let scale = i64::from(LANE_UNITS_PER_LANE);
            let progress = (i64::from(note_size.0) - 4 * scale).clamp(0, scale);
            let decrease = i64::from(minimum.0 - maximum.0) * progress / scale;
            LanePosition(minimum.0 - i32::try_from(decrease).expect("slide offset fits i32"))
        }
        JudgementAreaOffsetType::EnumMax => area_offset(assist, JudgementAreaOffsetType::Default)
            .expect("every assist profile defines Default"),
        _ => area_offset(assist, area_type).expect("every direct area type has a profile row"),
    }
}

pub fn is_target_lane(note: &RuntimeNoteV1, lane: LanePosition) -> bool {
    is_target_lane_with_assist(AssistLevel::Level0, note, lane)
}

pub fn is_target_lane_with_assist(
    assist: AssistLevel,
    note: &RuntimeNoteV1,
    lane: LanePosition,
) -> bool {
    if lane.0 < 0 || lane.0 > (i32::from(RUNTIME_LANE_COUNT) - 1) * LANE_UNITS_PER_LANE {
        return false;
    }
    let extra = i64::from(
        judgement_area_offset_with_assist(assist, note.judgement_area_offset_type, note.size).0,
    );
    let half_lane = i64::from(LanePosition::HALF_LANE.0);
    let lane = i64::from(lane.0);
    let start = i64::from(note.position.0) - extra - half_lane;
    let end = i64::from(note.position.0) + i64::from(note.size.0) + extra - half_lane;
    start <= lane && lane <= end
}

pub fn notes_overlap(left: &RuntimeNoteV1, right: &RuntimeNoteV1) -> bool {
    let left_start = i64::from(left.position.0);
    let left_end = left_start + i64::from(left.size.0);
    let right_start = i64::from(right.position.0);
    let right_end = right_start + i64::from(right.size.0);
    left_start <= right_end && right_start <= left_end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: u32, time: i64, position: i32, size: i32) -> RuntimeNoteV1 {
        RuntimeNoteV1 {
            id,
            time: TimeMicros(time),
            position: LanePosition(position * LANE_UNITS_PER_LANE),
            size: LanePosition(size * LANE_UNITS_PER_LANE),
            operate_type: NoteOperateType::Normal,
            judgement_type: NoteJudgementType::Normal,
            judgement_area_offset_type: JudgementAreaOffsetType::Default,
            direction: NoteDirection::Normal,
            judged: true,
        }
    }

    fn chart() -> RuntimeChartV1 {
        RuntimeChartV1 {
            format: RUNTIME_CHART_FORMAT.into(),
            version: RUNTIME_CHART_VERSION,
            lane_count: RUNTIME_LANE_COUNT,
            assist_level: AssistLevel::Level0,
            notes: vec![note(0, 1_000, 8, 4), note(1, 2_000, 8, 4)],
            lines: vec![RuntimeLineV1 {
                id: 0,
                kind: RuntimeLineKind::Long,
                note_ids: vec![0, 1],
            }],
        }
    }

    #[test]
    fn versioned_runtime_chart_round_trips_and_validates() {
        let chart = chart();
        chart.validate().unwrap();
        let value = serde_json::to_value(&chart).unwrap();
        assert_eq!(value["format"], RUNTIME_CHART_FORMAT);
        assert_eq!(value["version"], 1);
        assert_eq!(value["laneCount"], 24);
        assert_eq!(value["assistLevel"], 0);
        assert_eq!(
            serde_json::from_value::<RuntimeChartV1>(value).unwrap(),
            chart
        );
    }

    #[test]
    fn missing_assist_level_deserializes_as_level_zero() {
        let chart = chart();
        let mut value = serde_json::to_value(&chart).unwrap();
        value.as_object_mut().unwrap().remove("assistLevel");
        let decoded: RuntimeChartV1 = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.assist_level, AssistLevel::Level0);
        assert_eq!(decoded, chart);
    }

    #[test]
    fn validation_rejects_unstable_note_and_line_order() {
        let mut runtime = chart();
        runtime.notes.swap(0, 1);
        assert_eq!(runtime.validate(), Err(RuntimeChartError::UnsortedNotes(1)));

        let mut runtime = chart();
        runtime.lines[0].note_ids.reverse();
        assert_eq!(runtime.validate(), Err(RuntimeChartError::UnsortedLine(0)));
    }

    #[test]
    fn lane_bounds_are_inclusive_after_fixed_point_expansion() {
        let mut runtime_note = note(0, 0, 8, 4);
        runtime_note.judgement_area_offset_type = JudgementAreaOffsetType::Default;
        assert!(is_target_lane(&runtime_note, LanePosition(6_500_000)));
        assert!(is_target_lane(&runtime_note, LanePosition(12_500_000)));
        assert!(!is_target_lane(&runtime_note, LanePosition(6_499_999)));
        assert!(!is_target_lane(&runtime_note, LanePosition(12_500_001)));
    }

    #[test]
    fn slide_offset_interpolates_from_two_to_one_lanes() {
        assert_eq!(
            judgement_area_offset(JudgementAreaOffsetType::Slide, LanePosition(4_000_000)),
            LanePosition(2_000_000)
        );
        assert_eq!(
            judgement_area_offset(JudgementAreaOffsetType::Slide, LanePosition(4_250_000)),
            LanePosition(1_750_000)
        );
        assert_eq!(
            judgement_area_offset(JudgementAreaOffsetType::Slide, LanePosition(5_000_000)),
            LanePosition(1_000_000)
        );
        assert_eq!(
            judgement_area_offset(JudgementAreaOffsetType::Slide, LanePosition(i32::MIN)),
            LanePosition(2_000_000)
        );
        assert_eq!(
            judgement_area_offset(JudgementAreaOffsetType::Slide, LanePosition(i32::MAX)),
            LanePosition(1_000_000)
        );
    }

    #[test]
    fn every_assist_level_has_nine_unique_area_rows() {
        let area_types = [
            JudgementAreaOffsetType::Default,
            JudgementAreaOffsetType::SlideBegin,
            JudgementAreaOffsetType::SlideEnd,
            JudgementAreaOffsetType::SlideMin,
            JudgementAreaOffsetType::SlideMax,
            JudgementAreaOffsetType::Flick,
            JudgementAreaOffsetType::Trace,
            JudgementAreaOffsetType::EasyDefault,
            JudgementAreaOffsetType::EasySlideBegin,
        ];
        let mut keys = BTreeSet::new();
        for assist in AssistLevel::ALL {
            let mut count = 0;
            for area_type in area_types {
                assert!(area_offset(assist, area_type).is_some());
                assert!(keys.insert((assist as u8, area_type as i8)));
                count += 1;
            }
            assert_eq!(count, AREA_OFFSETS_PER_LEVEL);
            assert!(area_offset(assist, JudgementAreaOffsetType::Slide).is_none());
            assert!(area_offset(assist, JudgementAreaOffsetType::EnumMax).is_none());
        }
        assert_eq!(keys.len(), AREA_OFFSETS.len());
        assert_eq!(AREA_OFFSETS.len(), 54);
    }

    #[test]
    fn area_profiles_and_slide_interpolation_cover_every_level() {
        let defaults = [
            1_000_000, 1_100_000, 1_200_000, 1_300_000, 1_400_000, 1_500_000,
        ];
        let traces = [
            2_800_000, 2_850_000, 2_900_000, 2_950_000, 3_000_000, 3_100_000,
        ];
        for ((assist, default), trace) in AssistLevel::ALL.into_iter().zip(defaults).zip(traces) {
            assert_eq!(
                area_offset(assist, JudgementAreaOffsetType::Default),
                Some(LanePosition(default))
            );
            assert_eq!(
                area_offset(assist, JudgementAreaOffsetType::Trace),
                Some(LanePosition(trace))
            );
            assert_eq!(
                judgement_area_offset_with_assist(
                    assist,
                    JudgementAreaOffsetType::Slide,
                    LanePosition(4_000_000)
                ),
                area_offset(assist, JudgementAreaOffsetType::SlideMin).unwrap()
            );
            assert_eq!(
                judgement_area_offset_with_assist(
                    assist,
                    JudgementAreaOffsetType::Slide,
                    LanePosition(5_000_000)
                ),
                area_offset(assist, JudgementAreaOffsetType::SlideMax).unwrap()
            );
        }
        assert_eq!(
            judgement_area_offset_with_assist(
                AssistLevel::Level5,
                JudgementAreaOffsetType::Slide,
                LanePosition(4_500_000)
            ),
            LanePosition(2_000_000)
        );
    }

    #[test]
    fn area_compatibility_wrappers_are_exactly_level_zero() {
        let area_types = [
            JudgementAreaOffsetType::Default,
            JudgementAreaOffsetType::Slide,
            JudgementAreaOffsetType::SlideBegin,
            JudgementAreaOffsetType::SlideEnd,
            JudgementAreaOffsetType::Flick,
            JudgementAreaOffsetType::Trace,
            JudgementAreaOffsetType::SlideMin,
            JudgementAreaOffsetType::SlideMax,
            JudgementAreaOffsetType::EasyDefault,
            JudgementAreaOffsetType::EasySlideBegin,
            JudgementAreaOffsetType::EnumMax,
        ];
        for area_type in area_types {
            for size in [
                LanePosition(i32::MIN),
                LanePosition(4_500_000),
                LanePosition(i32::MAX),
            ] {
                assert_eq!(
                    judgement_area_offset(area_type, size),
                    judgement_area_offset_with_assist(AssistLevel::Level0, area_type, size)
                );
            }
        }

        let runtime_note = note(0, 0, 8, 4);
        for lane in [
            LanePosition(6_499_999),
            LanePosition(6_500_000),
            LanePosition(12_500_000),
        ] {
            assert_eq!(
                is_target_lane(&runtime_note, lane),
                is_target_lane_with_assist(AssistLevel::Level0, &runtime_note, lane)
            );
        }
    }
}
