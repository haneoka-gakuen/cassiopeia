//! Fixed-width gameplay observations shared by C and WASM hosts.
use crate::{FixedSessionSnapshot, JudgementEvent, SessionMode};

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

pub fn append_event_words(
    words: &mut Vec<i64>,
    event: &JudgementEvent,
    note_id: u32,
) -> Result<(), String> {
    words.extend_from_slice(&[
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

pub fn write_snapshot_words(
    target: &mut [i64; SNAPSHOT_WORDS],
    s: FixedSessionSnapshot,
    event_count: usize,
) -> Result<(), String> {
    *target = [
        HOST_ABI_VERSION,
        match s.mode {
            SessionMode::Chart => 0,
            SessionMode::Watch => 1,
            SessionMode::Play => 2,
        },
        i64::from(u8::from(s.assist_level)),
        s.time.0,
        s.judgement_offset.0,
        i64::from(s.combo),
        i64::from(s.perfect_combo),
        i64::from(s.all_perfect),
        i64::from(s.full_combo),
        i64::from(s.max_combo),
        i64::from(s.score),
        i64::from(s.life),
        i64::from(s.processed),
        i64::from(s.total),
        optional_sequence_word(s.last_input_sequence)?,
        i64::try_from(event_count).map_err(|_| "event count overflowed".to_owned())?,
    ];
    Ok(())
}

fn optional_sequence_word(sequence: Option<u64>) -> Result<i64, String> {
    sequence
        .map(|n| {
            i64::try_from(n).map_err(|_| "input sequence exceeds the signed host ABI".to_owned())
        })
        .transpose()
        .map(|n| n.unwrap_or(NO_VALUE))
}
