//! Compact little-endian `RuntimeChartV1` transport.
//!
//! The 24-byte header contains `CASR`, binary version, zero flags, runtime
//! version, lane count, assist level, note count, and line count. Each note is
//! a fixed 32-byte record. Each line has a 12-byte header followed by its
//! numeric `u32` note IDs. Reserved bytes must be zero.

use std::fmt;

use haneoka_cassiopeia_core::{
    AssistLevel, JudgementAreaOffsetType, LanePosition, NoteDirection, NoteJudgementType,
    NoteOperateType, RUNTIME_CHART_FORMAT, RuntimeChartV1, RuntimeLineKind, RuntimeLineV1,
    RuntimeNoteV1, TimeMicros,
};

pub const CHART_BINARY_MAGIC: [u8; 4] = *b"CASR";
pub const CHART_BINARY_VERSION: u16 = 1;
pub const CHART_HEADER_BYTES: usize = 24;
pub const CHART_NOTE_BYTES: usize = 32;
pub const CHART_LINE_HEADER_BYTES: usize = 12;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChartBinaryError {
    Truncated { offset: usize, required: usize },
    TrailingBytes(usize),
    InvalidMagic,
    UnsupportedBinaryVersion(u16),
    UnsupportedFlags(u16),
    InvalidAssistLevel(u8),
    InvalidOperateType(i16),
    InvalidJudgementType(i16),
    InvalidAreaOffsetType(i8),
    InvalidDirection(i8),
    InvalidNoteFlags(u16),
    InvalidLineKind(u8),
    InvalidLinePadding,
    LengthOverflow,
}

impl fmt::Display for ChartBinaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { offset, required } => write!(
                formatter,
                "runtime chart is truncated at byte {offset}; {required} bytes are required"
            ),
            Self::TrailingBytes(count) => {
                write!(formatter, "runtime chart has {count} trailing bytes")
            }
            Self::InvalidMagic => formatter.write_str("invalid runtime chart binary magic"),
            Self::UnsupportedBinaryVersion(version) => {
                write!(
                    formatter,
                    "unsupported runtime chart binary version: {version}"
                )
            }
            Self::UnsupportedFlags(flags) => {
                write!(
                    formatter,
                    "unsupported runtime chart binary flags: {flags:#06x}"
                )
            }
            Self::InvalidAssistLevel(level) => write!(formatter, "invalid assist level: {level}"),
            Self::InvalidOperateType(value) => {
                write!(formatter, "invalid note operation value: {value}")
            }
            Self::InvalidJudgementType(value) => {
                write!(formatter, "invalid note judgement value: {value}")
            }
            Self::InvalidAreaOffsetType(value) => {
                write!(formatter, "invalid judgement area value: {value}")
            }
            Self::InvalidDirection(value) => {
                write!(formatter, "invalid note direction value: {value}")
            }
            Self::InvalidNoteFlags(flags) => {
                write!(formatter, "invalid runtime note flags: {flags:#06x}")
            }
            Self::InvalidLineKind(value) => {
                write!(formatter, "invalid runtime line kind: {value}")
            }
            Self::InvalidLinePadding => formatter.write_str("runtime line padding must be zero"),
            Self::LengthOverflow => formatter.write_str("runtime chart length overflowed"),
        }
    }
}

impl std::error::Error for ChartBinaryError {}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], ChartBinaryError> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(ChartBinaryError::LengthOverflow)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(ChartBinaryError::Truncated {
                offset: self.offset,
                required: N,
            })?;
        self.offset = end;
        Ok(slice.try_into().expect("the slice length is fixed"))
    }

    fn u8(&mut self) -> Result<u8, ChartBinaryError> {
        Ok(self.take::<1>()?[0])
    }

    fn i8(&mut self) -> Result<i8, ChartBinaryError> {
        Ok(i8::from_le_bytes(self.take()?))
    }

    fn u16(&mut self) -> Result<u16, ChartBinaryError> {
        Ok(u16::from_le_bytes(self.take()?))
    }

    fn i16(&mut self) -> Result<i16, ChartBinaryError> {
        Ok(i16::from_le_bytes(self.take()?))
    }

    fn u32(&mut self) -> Result<u32, ChartBinaryError> {
        Ok(u32::from_le_bytes(self.take()?))
    }

    fn i32(&mut self) -> Result<i32, ChartBinaryError> {
        Ok(i32::from_le_bytes(self.take()?))
    }

    fn i64(&mut self) -> Result<i64, ChartBinaryError> {
        Ok(i64::from_le_bytes(self.take()?))
    }
}

pub fn decode_runtime_chart_v1(bytes: &[u8]) -> Result<RuntimeChartV1, ChartBinaryError> {
    let mut reader = Reader::new(bytes);
    if reader.take::<4>()? != CHART_BINARY_MAGIC {
        return Err(ChartBinaryError::InvalidMagic);
    }
    let binary_version = reader.u16()?;
    if binary_version != CHART_BINARY_VERSION {
        return Err(ChartBinaryError::UnsupportedBinaryVersion(binary_version));
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(ChartBinaryError::UnsupportedFlags(flags));
    }
    let version = reader.u32()?;
    let lane_count = reader.u16()?;
    let assist_value = reader.u8()?;
    let assist_level = AssistLevel::try_from(assist_value)
        .map_err(|_| ChartBinaryError::InvalidAssistLevel(assist_value))?;
    if reader.u8()? != 0 {
        return Err(ChartBinaryError::UnsupportedFlags(1));
    }
    let note_count =
        usize::try_from(reader.u32()?).map_err(|_| ChartBinaryError::LengthOverflow)?;
    let line_count =
        usize::try_from(reader.u32()?).map_err(|_| ChartBinaryError::LengthOverflow)?;

    let minimum_note_bytes = note_count
        .checked_mul(CHART_NOTE_BYTES)
        .and_then(|value| value.checked_add(CHART_HEADER_BYTES))
        .ok_or(ChartBinaryError::LengthOverflow)?;
    if bytes.len() < minimum_note_bytes {
        return Err(ChartBinaryError::Truncated {
            offset: bytes.len(),
            required: minimum_note_bytes - bytes.len(),
        });
    }

    let mut notes = Vec::with_capacity(note_count);
    for _ in 0..note_count {
        let id = reader.u32()?;
        let time = TimeMicros(reader.i64()?);
        let position = LanePosition(reader.i32()?);
        let size = LanePosition(reader.i32()?);
        let operate_type = operate_type(reader.i16()?)?;
        let judgement_type = judgement_type(reader.i16()?)?;
        let judgement_area_offset_type = area_offset_type(reader.i8()?)?;
        let direction = direction(reader.i8()?)?;
        let note_flags = reader.u16()?;
        if note_flags & !1 != 0 {
            return Err(ChartBinaryError::InvalidNoteFlags(note_flags));
        }
        if reader.u32()? != 0 {
            return Err(ChartBinaryError::InvalidNoteFlags(note_flags | 2));
        }
        notes.push(RuntimeNoteV1 {
            id,
            time,
            position,
            size,
            operate_type,
            judgement_type,
            judgement_area_offset_type,
            direction,
            judged: note_flags & 1 != 0,
        });
    }

    let mut lines = Vec::with_capacity(line_count);
    for _ in 0..line_count {
        let id = reader.u32()?;
        let kind = line_kind(reader.u8()?)?;
        if reader.take::<3>()? != [0; 3] {
            return Err(ChartBinaryError::InvalidLinePadding);
        }
        let member_count =
            usize::try_from(reader.u32()?).map_err(|_| ChartBinaryError::LengthOverflow)?;
        let mut note_ids = Vec::with_capacity(member_count);
        for _ in 0..member_count {
            note_ids.push(reader.u32()?);
        }
        lines.push(RuntimeLineV1 { id, kind, note_ids });
    }

    if reader.offset != bytes.len() {
        return Err(ChartBinaryError::TrailingBytes(bytes.len() - reader.offset));
    }
    Ok(RuntimeChartV1 {
        format: RUNTIME_CHART_FORMAT.into(),
        version,
        lane_count,
        assist_level,
        notes,
        lines,
    })
}

pub fn encode_runtime_chart_v1(chart: &RuntimeChartV1) -> Result<Vec<u8>, ChartBinaryError> {
    let note_bytes = chart
        .notes
        .len()
        .checked_mul(CHART_NOTE_BYTES)
        .ok_or(ChartBinaryError::LengthOverflow)?;
    let line_bytes = chart.lines.iter().try_fold(0usize, |sum, line| {
        line.note_ids
            .len()
            .checked_mul(4)
            .and_then(|members| members.checked_add(CHART_LINE_HEADER_BYTES))
            .and_then(|line_size| sum.checked_add(line_size))
            .ok_or(ChartBinaryError::LengthOverflow)
    })?;
    let capacity = CHART_HEADER_BYTES
        .checked_add(note_bytes)
        .and_then(|value| value.checked_add(line_bytes))
        .ok_or(ChartBinaryError::LengthOverflow)?;
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(&CHART_BINARY_MAGIC);
    bytes.extend_from_slice(&CHART_BINARY_VERSION.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&chart.version.to_le_bytes());
    bytes.extend_from_slice(&chart.lane_count.to_le_bytes());
    bytes.push(u8::from(chart.assist_level));
    bytes.push(0);
    bytes.extend_from_slice(
        &u32::try_from(chart.notes.len())
            .map_err(|_| ChartBinaryError::LengthOverflow)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u32::try_from(chart.lines.len())
            .map_err(|_| ChartBinaryError::LengthOverflow)?
            .to_le_bytes(),
    );
    for note in &chart.notes {
        bytes.extend_from_slice(&note.id.to_le_bytes());
        bytes.extend_from_slice(&note.time.0.to_le_bytes());
        bytes.extend_from_slice(&note.position.0.to_le_bytes());
        bytes.extend_from_slice(&note.size.0.to_le_bytes());
        bytes.extend_from_slice(&(note.operate_type as i16).to_le_bytes());
        bytes.extend_from_slice(&(note.judgement_type as i16).to_le_bytes());
        bytes.push(note.judgement_area_offset_type as i8 as u8);
        bytes.push(note.direction as i8 as u8);
        bytes.extend_from_slice(&u16::from(note.judged).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
    }
    for line in &chart.lines {
        bytes.extend_from_slice(&line.id.to_le_bytes());
        bytes.push(match line.kind {
            RuntimeLineKind::Long => 0,
            RuntimeLineKind::Guide => 1,
        });
        bytes.extend_from_slice(&[0; 3]);
        bytes.extend_from_slice(
            &u32::try_from(line.note_ids.len())
                .map_err(|_| ChartBinaryError::LengthOverflow)?
                .to_le_bytes(),
        );
        for note_id in &line.note_ids {
            bytes.extend_from_slice(&note_id.to_le_bytes());
        }
    }
    Ok(bytes)
}

fn operate_type(value: i16) -> Result<NoteOperateType, ChartBinaryError> {
    let result = match value {
        0 => NoteOperateType::None,
        1 => NoteOperateType::Normal,
        20 => NoteOperateType::SlideBegin,
        21 => NoteOperateType::SlideConnection,
        22 => NoteOperateType::SlideEnd,
        40 => NoteOperateType::Flick,
        41 => NoteOperateType::SlideBeginFlick,
        42 => NoteOperateType::SlideEndFlick,
        60 => NoteOperateType::Trace,
        61 => NoteOperateType::SlideBeginTrace,
        62 => NoteOperateType::SlideEndTrace,
        63 => NoteOperateType::SlideConnectionTrace,
        80 => NoteOperateType::HiddenSlideBegin,
        82 => NoteOperateType::HiddenSlideEnd,
        100 => NoteOperateType::GuideBegin,
        101 => NoteOperateType::GuideBeginNormal,
        102 => NoteOperateType::GuideBeginFlick,
        103 => NoteOperateType::GuideEnd,
        104 => NoteOperateType::GuideBeginTrace,
        105 => NoteOperateType::GuideEndTrace,
        120 => NoteOperateType::Combo,
        121 => NoteOperateType::ComboSkip,
        122 => NoteOperateType::Hidden,
        123 => NoteOperateType::InvalidHidden,
        _ => return Err(ChartBinaryError::InvalidOperateType(value)),
    };
    Ok(result)
}

fn judgement_type(value: i16) -> Result<NoteJudgementType, ChartBinaryError> {
    let result = match value {
        0 => NoteJudgementType::None,
        1 => NoteJudgementType::Normal,
        2 => NoteJudgementType::EasyNormal,
        5 => NoteJudgementType::Flick,
        10 => NoteJudgementType::SlideBegin,
        11 => NoteJudgementType::SlideEnd,
        12 => NoteJudgementType::SlideEndFlick,
        15 => NoteJudgementType::SlideBeginEasy,
        21 => NoteJudgementType::Trace,
        22 => NoteJudgementType::SlideEndTrace,
        _ => return Err(ChartBinaryError::InvalidJudgementType(value)),
    };
    Ok(result)
}

fn area_offset_type(value: i8) -> Result<JudgementAreaOffsetType, ChartBinaryError> {
    let result = match value {
        0 => JudgementAreaOffsetType::Default,
        1 => JudgementAreaOffsetType::Slide,
        2 => JudgementAreaOffsetType::SlideBegin,
        3 => JudgementAreaOffsetType::SlideEnd,
        4 => JudgementAreaOffsetType::Flick,
        5 => JudgementAreaOffsetType::Trace,
        6 => JudgementAreaOffsetType::SlideMin,
        7 => JudgementAreaOffsetType::SlideMax,
        8 => JudgementAreaOffsetType::EasyDefault,
        9 => JudgementAreaOffsetType::EasySlideBegin,
        10 => JudgementAreaOffsetType::EnumMax,
        _ => return Err(ChartBinaryError::InvalidAreaOffsetType(value)),
    };
    Ok(result)
}

fn direction(value: i8) -> Result<NoteDirection, ChartBinaryError> {
    let result = match value {
        0 => NoteDirection::Normal,
        1 => NoteDirection::Left,
        2 => NoteDirection::Right,
        _ => return Err(ChartBinaryError::InvalidDirection(value)),
    };
    Ok(result)
}

fn line_kind(value: u8) -> Result<RuntimeLineKind, ChartBinaryError> {
    match value {
        0 => Ok(RuntimeLineKind::Long),
        1 => Ok(RuntimeLineKind::Guide),
        _ => Err(ChartBinaryError::InvalidLineKind(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use haneoka_cassiopeia_core::{RUNTIME_CHART_VERSION, RUNTIME_LANE_COUNT};

    fn chart() -> RuntimeChartV1 {
        RuntimeChartV1 {
            format: RUNTIME_CHART_FORMAT.into(),
            version: RUNTIME_CHART_VERSION,
            lane_count: RUNTIME_LANE_COUNT,
            assist_level: AssistLevel::Level2,
            notes: vec![RuntimeNoteV1 {
                id: 42,
                time: TimeMicros(1_234_567),
                position: LanePosition(2_000_000),
                size: LanePosition(3_000_000),
                operate_type: NoteOperateType::Normal,
                judgement_type: NoteJudgementType::Normal,
                judgement_area_offset_type: JudgementAreaOffsetType::Default,
                direction: NoteDirection::Normal,
                judged: true,
            }],
            lines: vec![RuntimeLineV1 {
                id: 7,
                kind: RuntimeLineKind::Long,
                note_ids: vec![42],
            }],
        }
    }

    #[test]
    fn binary_round_trip_preserves_fixed_width_runtime_values() {
        let source = chart();
        let bytes = encode_runtime_chart_v1(&source).unwrap();
        assert_eq!(bytes.len(), CHART_HEADER_BYTES + CHART_NOTE_BYTES + 16);
        assert_eq!(decode_runtime_chart_v1(&bytes).unwrap(), source);
    }

    #[test]
    fn decoder_rejects_unknown_values_and_trailing_data() {
        let bytes = encode_runtime_chart_v1(&chart()).unwrap();
        let mut invalid = bytes.clone();
        invalid[CHART_HEADER_BYTES + 24] = 99;
        assert_eq!(
            decode_runtime_chart_v1(&invalid),
            Err(ChartBinaryError::InvalidAreaOffsetType(99))
        );

        let mut trailing = bytes;
        trailing.push(0);
        assert_eq!(
            decode_runtime_chart_v1(&trailing),
            Err(ChartBinaryError::TrailingBytes(1))
        );
    }
}
