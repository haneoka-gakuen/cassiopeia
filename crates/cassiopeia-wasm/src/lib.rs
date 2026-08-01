#![forbid(unsafe_code)]

//! Allocation-stable synchronous boundary for browser gameplay hosts.
//!
//! The snapshot region is 16 signed 64-bit words in this order: ABI version,
//! mode, assist level, time, judgement offset, combo, Perfect-Combo flag,
//! All-Perfect flag, Full-Combo flag, max combo, score, life, processed count,
//! total count, last input sequence, and event count.
//!
//! Each event uses 12 signed 64-bit words: numeric note ID, judgement, timing,
//! difference, judged-at time, input sequence, combo, max combo, score, score
//! delta, life, and reserved zero. `-1` represents an absent sequence. Hosts
//! should recreate typed-array views when `WebAssembly.Memory.buffer` changes.

mod chart_binary;

use std::collections::BTreeMap;

pub use chart_binary::{
    CHART_BINARY_MAGIC, CHART_BINARY_VERSION, CHART_HEADER_BYTES, CHART_LINE_HEADER_BYTES,
    CHART_NOTE_BYTES, ChartBinaryError, decode_runtime_chart_v1, encode_runtime_chart_v1,
};
use haneoka_cassiopeia_core::{
    GameplaySession, InputAction, InputVector, JudgementEvent, LanePosition, RuntimeInputEvent,
    SessionMode, TimeMicros,
};
use wasm_bindgen::prelude::*;

pub const HOST_ABI_VERSION: i64 = 1;
pub const EVENT_STRIDE: usize = 12;
pub const SNAPSHOT_WORDS: usize = 16;
const NO_VALUE: i64 = -1;

pub const EVENT_NOTE_ID: usize = 0;
pub const EVENT_JUDGEMENT: usize = 1;
pub const EVENT_TIMING: usize = 2;
pub const EVENT_DIFFERENCE: usize = 3;
pub const EVENT_JUDGED_AT: usize = 4;
pub const EVENT_INPUT_SEQUENCE: usize = 5;
pub const EVENT_COMBO: usize = 6;
pub const EVENT_MAX_COMBO: usize = 7;
pub const EVENT_SCORE: usize = 8;
pub const EVENT_SCORE_DELTA: usize = 9;
pub const EVENT_LIFE: usize = 10;
pub const EVENT_RESERVED: usize = 11;

pub const SNAPSHOT_ABI_VERSION: usize = 0;
pub const SNAPSHOT_MODE: usize = 1;
pub const SNAPSHOT_ASSIST_LEVEL: usize = 2;
pub const SNAPSHOT_TIME: usize = 3;
pub const SNAPSHOT_JUDGEMENT_OFFSET: usize = 4;
pub const SNAPSHOT_COMBO: usize = 5;
pub const SNAPSHOT_PERFECT_COMBO: usize = 6;
pub const SNAPSHOT_ALL_PERFECT: usize = 7;
pub const SNAPSHOT_FULL_COMBO: usize = 8;
pub const SNAPSHOT_MAX_COMBO: usize = 9;
pub const SNAPSHOT_SCORE: usize = 10;
pub const SNAPSHOT_LIFE: usize = 11;
pub const SNAPSHOT_PROCESSED: usize = 12;
pub const SNAPSHOT_TOTAL: usize = 13;
pub const SNAPSHOT_LAST_INPUT_SEQUENCE: usize = 14;
pub const SNAPSHOT_EVENT_COUNT: usize = 15;

/// Owns one validated chart and exposes only synchronous, fixed-width calls.
#[derive(Debug)]
#[wasm_bindgen(js_name = CassiopeiaRuntime)]
pub struct WasmRuntime {
    session: GameplaySession,
    numeric_note_ids: BTreeMap<String, u32>,
    event_words: Vec<i64>,
    snapshot_words: [i64; SNAPSHOT_WORDS],
}

#[wasm_bindgen(js_class = CassiopeiaRuntime)]
impl WasmRuntime {
    #[wasm_bindgen(constructor)]
    pub fn new(
        chart_bytes: &[u8],
        mode: u8,
        judgement_offset_micros: i64,
    ) -> Result<WasmRuntime, JsError> {
        Self::from_binary(chart_bytes, mode, judgement_offset_micros).map_err(js_error)
    }

    /// Advances automatic judgements and writes all resulting events contiguously.
    pub fn update(&mut self, time_micros: i64) -> Result<u32, JsError> {
        self.event_words.clear();
        let events = self
            .session
            .advance(TimeMicros(time_micros))
            .map_err(js_error)?;
        self.write_events(events.iter()).map_err(js_error)?;
        self.refresh_snapshot().map_err(js_error)?;
        self.event_count()
    }

    /// Settles exact-tail and post-music note states after media playback ends.
    pub fn finish(&mut self, time_micros: i64) -> Result<u32, JsError> {
        self.event_words.clear();
        let events = self
            .session
            .finish(TimeMicros(time_micros))
            .map_err(js_error)?;
        self.write_events(events.iter()).map_err(js_error)?;
        self.refresh_snapshot().map_err(js_error)?;
        self.event_count()
    }

    pub fn reset(&mut self, time_micros: i64) -> Result<(), JsError> {
        self.event_words.clear();
        self.session
            .reset(TimeMicros(time_micros))
            .map_err(js_error)?;
        self.refresh_snapshot().map_err(js_error)
    }

    pub fn set_mode(&mut self, mode: u8) -> Result<(), JsError> {
        self.event_words.clear();
        self.session
            .set_mode(session_mode(mode).map_err(js_error)?)
            .map_err(js_error)?;
        self.refresh_snapshot().map_err(js_error)
    }

    pub fn set_judgement_offset(&mut self, offset_micros: i64) -> Result<(), JsError> {
        self.event_words.clear();
        self.session.set_judgement_offset(TimeMicros(offset_micros));
        self.refresh_snapshot().map_err(js_error)
    }

    pub fn has_candidate(
        &self,
        lane_millionths: i32,
        time_micros: i64,
        pointer_id: i64,
    ) -> Result<bool, JsError> {
        self.session
            .has_runtime_input_candidate(
                LanePosition(lane_millionths),
                TimeMicros(time_micros),
                optional_pointer_id(pointer_id).map_err(js_error)?,
            )
            .map_err(js_error)
    }

    pub fn tap(
        &mut self,
        sequence: i64,
        time_micros: i64,
        lane_millionths: i32,
        pointer_id: i64,
    ) -> Result<u32, JsError> {
        self.input(
            sequence,
            time_micros,
            lane_millionths,
            pointer_id,
            InputAction::Tap,
        )
        .map_err(js_error)
    }

    pub fn release(
        &mut self,
        sequence: i64,
        time_micros: i64,
        lane_millionths: i32,
        pointer_id: i64,
    ) -> Result<u32, JsError> {
        self.input(
            sequence,
            time_micros,
            lane_millionths,
            pointer_id,
            InputAction::Release,
        )
        .map_err(js_error)
    }

    pub fn flick(
        &mut self,
        sequence: i64,
        time_micros: i64,
        lane_millionths: i32,
        pointer_id: i64,
        delta_x: i32,
        delta_y: i32,
    ) -> Result<u32, JsError> {
        self.input(
            sequence,
            time_micros,
            lane_millionths,
            pointer_id,
            InputAction::Flick {
                movement: InputVector { delta_x, delta_y },
            },
        )
        .map_err(js_error)
    }

    pub fn trace(
        &mut self,
        sequence: i64,
        time_micros: i64,
        lane_millionths: i32,
        pointer_id: i64,
    ) -> Result<u32, JsError> {
        self.input(
            sequence,
            time_micros,
            lane_millionths,
            pointer_id,
            InputAction::Trace,
        )
        .map_err(js_error)
    }

    pub fn cancel(
        &mut self,
        sequence: i64,
        time_micros: i64,
        lane_millionths: i32,
        pointer_id: i64,
    ) -> Result<u32, JsError> {
        self.input(
            sequence,
            time_micros,
            lane_millionths,
            pointer_id,
            InputAction::Cancel,
        )
        .map_err(js_error)
    }

    /// Pointer to a reusable `BigInt64Array`-compatible event region.
    pub fn event_buffer_ptr(&self) -> usize {
        self.event_words.as_ptr() as usize
    }

    pub fn event_buffer_len(&self) -> usize {
        self.event_words.len()
    }

    pub fn event_stride(&self) -> usize {
        EVENT_STRIDE
    }

    pub fn event_count(&self) -> Result<u32, JsError> {
        u32::try_from(self.event_words.len() / EVENT_STRIDE).map_err(js_error)
    }

    /// Pointer to the stable 16-word `BigInt64Array` snapshot region.
    pub fn snapshot_buffer_ptr(&self) -> usize {
        self.snapshot_words.as_ptr() as usize
    }

    pub fn snapshot_buffer_len(&self) -> usize {
        SNAPSHOT_WORDS
    }

    pub fn mode(&self) -> u8 {
        self.snapshot_words[SNAPSHOT_MODE] as u8
    }

    pub fn assist_level(&self) -> u8 {
        self.snapshot_words[SNAPSHOT_ASSIST_LEVEL] as u8
    }

    pub fn time_micros(&self) -> i64 {
        self.snapshot_words[SNAPSHOT_TIME]
    }

    pub fn judgement_offset_micros(&self) -> i64 {
        self.snapshot_words[SNAPSHOT_JUDGEMENT_OFFSET]
    }

    pub fn combo(&self) -> u32 {
        self.snapshot_words[SNAPSHOT_COMBO] as u32
    }

    pub fn perfect_combo(&self) -> bool {
        self.snapshot_words[SNAPSHOT_PERFECT_COMBO] != 0
    }

    pub fn all_perfect(&self) -> bool {
        self.snapshot_words[SNAPSHOT_ALL_PERFECT] != 0
    }

    pub fn full_combo(&self) -> bool {
        self.snapshot_words[SNAPSHOT_FULL_COMBO] != 0
    }

    pub fn max_combo(&self) -> u32 {
        self.snapshot_words[SNAPSHOT_MAX_COMBO] as u32
    }

    pub fn score(&self) -> u32 {
        self.snapshot_words[SNAPSHOT_SCORE] as u32
    }

    pub fn life(&self) -> u32 {
        self.snapshot_words[SNAPSHOT_LIFE] as u32
    }

    pub fn processed(&self) -> u32 {
        self.snapshot_words[SNAPSHOT_PROCESSED] as u32
    }

    pub fn total(&self) -> u32 {
        self.snapshot_words[SNAPSHOT_TOTAL] as u32
    }
}

impl WasmRuntime {
    pub fn from_binary(
        chart_bytes: &[u8],
        mode: u8,
        judgement_offset_micros: i64,
    ) -> Result<Self, String> {
        let chart = decode_runtime_chart_v1(chart_bytes).map_err(|error| error.to_string())?;
        chart.validate().map_err(|error| error.to_string())?;
        let numeric_note_ids: BTreeMap<_, _> = chart
            .notes
            .iter()
            .filter(|note| note.judged)
            .map(|note| (note.id.to_string(), note.id))
            .collect();
        let playable_count = numeric_note_ids.len();
        let session = GameplaySession::from_runtime_chart(
            chart,
            session_mode(mode)?,
            TimeMicros(judgement_offset_micros),
        )
        .map_err(|error| error.to_string())?;
        let event_capacity = playable_count
            .checked_mul(EVENT_STRIDE)
            .ok_or_else(|| "event buffer capacity overflowed".to_owned())?;
        let mut result = Self {
            session,
            numeric_note_ids,
            event_words: Vec::with_capacity(event_capacity),
            snapshot_words: [0; SNAPSHOT_WORDS],
        };
        result.refresh_snapshot()?;
        Ok(result)
    }

    fn input(
        &mut self,
        sequence: i64,
        time_micros: i64,
        lane_millionths: i32,
        pointer_id: i64,
        action: InputAction,
    ) -> Result<u32, String> {
        self.event_words.clear();
        let input = RuntimeInputEvent {
            sequence: non_negative_sequence(sequence)?,
            time: TimeMicros(time_micros),
            pointer_id: optional_pointer_id(pointer_id)?,
            lane: LanePosition(lane_millionths),
            action,
        };
        if let Some(event) = self
            .session
            .consume_runtime_input(&input)
            .map_err(|error| error.to_string())?
        {
            self.write_event(&event)?;
        }
        self.refresh_snapshot()?;
        u32::try_from(self.event_words.len() / EVENT_STRIDE)
            .map_err(|_| "event count overflowed".to_owned())
    }

    fn write_events<'a>(
        &mut self,
        events: impl IntoIterator<Item = &'a JudgementEvent>,
    ) -> Result<(), String> {
        for event in events {
            self.write_event(event)?;
        }
        Ok(())
    }

    fn write_event(&mut self, event: &JudgementEvent) -> Result<(), String> {
        let note_id = *self
            .numeric_note_ids
            .get(&event.note_id)
            .ok_or_else(|| format!("runtime event has unknown note ID: {}", event.note_id))?;
        self.event_words.extend_from_slice(&[
            i64::from(note_id),
            event.judgement as i8 as i64,
            event.timing as i8 as i64,
            event.difference.0,
            event.judged_at.0,
            optional_sequence_word(event.input_sequence)?,
            i64::from(event.combo),
            i64::from(event.max_combo),
            i64::from(event.score),
            i64::from(event.score_delta),
            i64::from(event.life),
            0,
        ]);
        Ok(())
    }

    fn refresh_snapshot(&mut self) -> Result<(), String> {
        let snapshot = self.session.fixed_snapshot();
        self.snapshot_words[SNAPSHOT_ABI_VERSION] = HOST_ABI_VERSION;
        self.snapshot_words[SNAPSHOT_MODE] = i64::from(mode_code(snapshot.mode));
        self.snapshot_words[SNAPSHOT_ASSIST_LEVEL] = i64::from(u8::from(snapshot.assist_level));
        self.snapshot_words[SNAPSHOT_TIME] = snapshot.time.0;
        self.snapshot_words[SNAPSHOT_JUDGEMENT_OFFSET] = snapshot.judgement_offset.0;
        self.snapshot_words[SNAPSHOT_COMBO] = i64::from(snapshot.combo);
        self.snapshot_words[SNAPSHOT_PERFECT_COMBO] = i64::from(snapshot.perfect_combo);
        self.snapshot_words[SNAPSHOT_ALL_PERFECT] = i64::from(snapshot.all_perfect);
        self.snapshot_words[SNAPSHOT_FULL_COMBO] = i64::from(snapshot.full_combo);
        self.snapshot_words[SNAPSHOT_MAX_COMBO] = i64::from(snapshot.max_combo);
        self.snapshot_words[SNAPSHOT_SCORE] = i64::from(snapshot.score);
        self.snapshot_words[SNAPSHOT_LIFE] = i64::from(snapshot.life);
        self.snapshot_words[SNAPSHOT_PROCESSED] = i64::from(snapshot.processed);
        self.snapshot_words[SNAPSHOT_TOTAL] = i64::from(snapshot.total);
        self.snapshot_words[SNAPSHOT_LAST_INPUT_SEQUENCE] =
            optional_sequence_word(snapshot.last_input_sequence)?;
        self.snapshot_words[SNAPSHOT_EVENT_COUNT] =
            i64::try_from(self.event_words.len() / EVENT_STRIDE)
                .map_err(|_| "event count overflowed".to_owned())?;
        Ok(())
    }
}

fn session_mode(value: u8) -> Result<SessionMode, String> {
    match value {
        0 => Ok(SessionMode::Chart),
        1 => Ok(SessionMode::Watch),
        2 => Ok(SessionMode::Play),
        _ => Err(format!("invalid session mode: {value}")),
    }
}

const fn mode_code(mode: SessionMode) -> u8 {
    match mode {
        SessionMode::Chart => 0,
        SessionMode::Watch => 1,
        SessionMode::Play => 2,
    }
}

fn non_negative_sequence(sequence: i64) -> Result<u64, String> {
    u64::try_from(sequence).map_err(|_| "input sequence must be non-negative".to_owned())
}

fn optional_pointer_id(pointer_id: i64) -> Result<Option<u32>, String> {
    if pointer_id == NO_VALUE {
        return Ok(None);
    }
    u32::try_from(pointer_id)
        .map(Some)
        .map_err(|_| "pointer ID must be -1 or an unsigned 32-bit integer".to_owned())
}

fn optional_sequence_word(sequence: Option<u64>) -> Result<i64, String> {
    sequence
        .map(|value| {
            i64::try_from(value)
                .map_err(|_| "input sequence exceeds the signed host ABI".to_owned())
        })
        .transpose()
        .map(|value| value.unwrap_or(NO_VALUE))
}

fn js_error(error: impl ToString) -> JsError {
    JsError::new(&error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use haneoka_cassiopeia_core::{
        AssistLevel, Judgement, JudgementAreaOffsetType, NoteDirection, NoteJudgementType,
        NoteOperateType, RUNTIME_CHART_FORMAT, RUNTIME_CHART_VERSION, RUNTIME_LANE_COUNT,
        RuntimeChartV1, RuntimeNoteV1,
    };

    fn chart(notes: &[(u32, i64, NoteOperateType, NoteJudgementType)]) -> RuntimeChartV1 {
        RuntimeChartV1 {
            format: RUNTIME_CHART_FORMAT.into(),
            version: RUNTIME_CHART_VERSION,
            lane_count: RUNTIME_LANE_COUNT,
            assist_level: AssistLevel::Level0,
            notes: notes
                .iter()
                .map(|(id, time, operate_type, judgement_type)| RuntimeNoteV1 {
                    id: *id,
                    time: TimeMicros(*time),
                    position: LanePosition(10_000_000),
                    size: LanePosition(2_000_000),
                    operate_type: *operate_type,
                    judgement_type: *judgement_type,
                    judgement_area_offset_type: JudgementAreaOffsetType::Default,
                    direction: NoteDirection::Normal,
                    judged: true,
                })
                .collect(),
            lines: Vec::new(),
        }
    }

    fn runtime(source: RuntimeChartV1, mode: u8) -> WasmRuntime {
        WasmRuntime::from_binary(&encode_runtime_chart_v1(&source).unwrap(), mode, 0).unwrap()
    }

    #[test]
    fn host_contract_uses_fixed_snapshot_and_event_words() {
        let mut runtime = runtime(
            chart(&[(
                4_000_000_001,
                1_000_000,
                NoteOperateType::Normal,
                NoteJudgementType::Normal,
            )]),
            2,
        );
        let event_capacity = runtime.event_words.capacity();
        let event_pointer = runtime.event_buffer_ptr();
        let snapshot_pointer = runtime.snapshot_buffer_ptr();

        assert!(runtime.has_candidate(10_000_000, 1_000_000, 7).unwrap());
        assert_eq!(runtime.tap(1, 1_000_000, 10_000_000, 7).unwrap(), 1);
        assert_eq!(runtime.event_words.len(), EVENT_STRIDE);
        assert_eq!(runtime.event_words[0], 4_000_000_001);
        assert_eq!(runtime.event_words[1], Judgement::Just as i8 as i64);
        assert_eq!(runtime.event_words[3], 0);
        assert_eq!(runtime.event_words[5], 1);
        assert_eq!(runtime.event_words[6], 1);
        assert_eq!(runtime.event_words[10], 1_000);
        assert_eq!(runtime.event_words[11], 0);
        assert_eq!(
            runtime.snapshot_words[SNAPSHOT_ABI_VERSION],
            HOST_ABI_VERSION
        );
        assert_eq!(runtime.snapshot_words[SNAPSHOT_EVENT_COUNT], 1);
        assert_eq!(runtime.combo(), 1);
        assert!(runtime.perfect_combo());
        assert_eq!(runtime.processed(), 1);
        assert_eq!(runtime.event_words.capacity(), event_capacity);
        assert_eq!(runtime.event_buffer_ptr(), event_pointer);
        assert_eq!(runtime.snapshot_buffer_ptr(), snapshot_pointer);
    }

    #[test]
    fn update_batches_many_events_without_per_note_boundary_calls() {
        let mut runtime = runtime(
            chart(&[
                (
                    10,
                    100_000,
                    NoteOperateType::Normal,
                    NoteJudgementType::Normal,
                ),
                (
                    11,
                    200_000,
                    NoteOperateType::Normal,
                    NoteJudgementType::Normal,
                ),
                (
                    12,
                    300_000,
                    NoteOperateType::Normal,
                    NoteJudgementType::Normal,
                ),
            ]),
            1,
        );
        let pointer = runtime.event_buffer_ptr();
        assert_eq!(runtime.update(300_001).unwrap(), 3);
        assert_eq!(runtime.event_buffer_len(), 3 * EVENT_STRIDE);
        assert_eq!(runtime.event_words[0], 10);
        assert_eq!(runtime.event_words[EVENT_STRIDE], 11);
        assert_eq!(runtime.event_words[2 * EVENT_STRIDE], 12);
        assert_eq!(runtime.event_buffer_ptr(), pointer);
        assert_eq!(runtime.processed(), 3);
    }

    #[test]
    fn reset_mode_offset_and_all_input_kinds_share_one_snapshot() {
        let mut runtime = runtime(
            chart(&[
                (
                    1,
                    1_000_000,
                    NoteOperateType::Normal,
                    NoteJudgementType::Normal,
                ),
                (
                    2,
                    2_000_000,
                    NoteOperateType::Flick,
                    NoteJudgementType::Flick,
                ),
                (
                    3,
                    3_000_000,
                    NoteOperateType::SlideEnd,
                    NoteJudgementType::SlideEnd,
                ),
                (
                    4,
                    4_000_000,
                    NoteOperateType::Trace,
                    NoteJudgementType::Trace,
                ),
            ]),
            2,
        );
        runtime.set_judgement_offset(1_000).unwrap();
        assert_eq!(runtime.judgement_offset_micros(), 1_000);
        assert_eq!(runtime.tap(1, 999_000, 10_000_000, 5).unwrap(), 1);
        assert_eq!(
            runtime.flick(2, 1_999_000, 10_000_000, 5, 10, 0).unwrap(),
            1
        );
        assert_eq!(runtime.release(3, 2_999_000, 10_000_000, 5).unwrap(), 1);
        assert_eq!(runtime.trace(4, 3_999_000, 10_000_000, 5).unwrap(), 1);
        assert_eq!(runtime.cancel(5, 4_000_000, 10_000_000, 5).unwrap(), 0);
        assert_eq!(runtime.snapshot_words[SNAPSHOT_LAST_INPUT_SEQUENCE], 5);

        runtime.reset(0).unwrap();
        assert_eq!(runtime.processed(), 0);
        runtime.set_mode(1).unwrap();
        assert_eq!(runtime.mode(), 1);
        assert_eq!(runtime.update(4_000_001).unwrap(), 4);
    }

    #[test]
    fn constructor_validates_the_chart_once() {
        let source = chart(&[(
            1,
            1_000_000,
            NoteOperateType::Normal,
            NoteJudgementType::Normal,
        )]);
        let mut bytes = encode_runtime_chart_v1(&source).unwrap();
        bytes[8..12].copy_from_slice(&99u32.to_le_bytes());
        let error = WasmRuntime::from_binary(&bytes, 2, 0).unwrap_err();
        assert_eq!(error, "unsupported runtime version: 99");
    }
}
