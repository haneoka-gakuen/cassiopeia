//! Native session ownership and allocation-stable observations. ABI1 JSON remains compatible.
use haneoka_cassiopeia_core::{
    GameplaySession, JudgementEvent, RuntimeChartV1, RuntimeInputEvent, ScoringComboPolicy,
    SessionMode, TimeMicros, WatchSessionLoad,
    automatic::AutomaticJudgementPolicy,
    runtime_boundary::{EVENT_STRIDE, SNAPSHOT_WORDS, append_event_words, write_snapshot_words},
};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

pub const ABI_VERSION: u32 = 1;
const MAX_REQUEST_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "command",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Command {
    Load {
        chart: RuntimeChartV1,
        mode: SessionMode,
        #[serde(default)]
        offset_micros: i64,
        #[serde(default)]
        automatic_policy: AutomaticJudgementPolicy,
        #[serde(default)]
        scoring_combo_policy: ScoringComboPolicy,
    },
    LoadNotes(WatchSessionLoad),
    Advance {
        time_micros: i64,
    },
    Input {
        event: RuntimeInputEvent,
    },
    Reset {
        time_micros: i64,
    },
    Finish {
        time_micros: i64,
    },
    SetMode {
        mode: SessionMode,
    },
    SetOffset {
        offset_micros: i64,
    },
    SetAutomaticPolicy {
        policy: AutomaticJudgementPolicy,
    },
    Snapshot {},
}

#[derive(Default)]
pub struct NativeHost {
    session: Option<GameplaySession>,
    notes_only: bool,
    numeric_note_ids: BTreeMap<String, u32>,
    event_words: Vec<i64>,
    snapshot_words: [i64; SNAPSHOT_WORDS],
    projection_event_words: Vec<i64>,
    projection_snapshot_words: [i64; SNAPSHOT_WORDS],
    error: Vec<u8>,
    reply: Vec<u8>,
}

impl NativeHost {
    pub fn dispatch(&mut self, request: &[u8]) -> &[u8] {
        let result = self.execute(request);
        let value = match result {
            Ok(events) => match self.refresh(&events) {
                Ok(()) => {
                    self.error.clear();
                    json!({"abiVersion": ABI_VERSION, "ok": true,
                    "snapshot": self.session.as_ref().map(GameplaySession::snapshot), "events": events})
                }
                Err(error) => {
                    self.error = error.as_bytes().to_vec();
                    json!({"abiVersion":ABI_VERSION,"ok":false,"error":error})
                }
            },
            Err(error) => {
                self.error = error.as_bytes().to_vec();
                json!({"abiVersion": ABI_VERSION, "ok": false, "error": error})
            }
        };
        self.reply.clear();
        serde_json::to_writer(&mut self.reply, &value).expect("serializable native reply");
        &self.reply
    }

    fn install(
        &mut self,
        session: GameplaySession,
        ids: BTreeMap<String, u32>,
        notes_only: bool,
    ) -> Result<Vec<JudgementEvent>, String> {
        let capacity = ids
            .len()
            .checked_mul(EVENT_STRIDE)
            .ok_or("event capacity overflowed")?;
        self.projection_event_words.clear();
        self.projection_snapshot_words.fill(0);
        self.session = Some(session);
        self.numeric_note_ids = ids;
        self.notes_only = notes_only;
        self.event_words = Vec::with_capacity(capacity);
        Ok(vec![])
    }

    fn execute(&mut self, request: &[u8]) -> Result<Vec<JudgementEvent>, String> {
        if request.len() > MAX_REQUEST_BYTES {
            return Err("request exceeds 32 MiB".into());
        }
        let command: Command = serde_json::from_slice(request).map_err(|e| e.to_string())?;
        match command {
            Command::Load {
                chart,
                mode,
                offset_micros,
                automatic_policy,
                scoring_combo_policy,
            } => {
                let ids = chart
                    .notes
                    .iter()
                    .filter(|n| n.judged)
                    .map(|n| (n.id.to_string(), n.id))
                    .collect();
                let mut session =
                    GameplaySession::from_runtime_chart(chart, mode, TimeMicros(offset_micros))
                        .map_err(|e| e.to_string())?;
                session.set_automatic_policy(automatic_policy)?;
                session
                    .set_scoring_combo_policy(scoring_combo_policy)
                    .map_err(|e| e.to_string())?;
                return self.install(session, ids, false);
            }
            Command::LoadNotes(load) => {
                let (session, ids) = load.into_session()?;
                return self.install(session, ids, true);
            }
            _ => {}
        }
        let session = self
            .session
            .as_mut()
            .ok_or("load a chart before issuing gameplay commands")?;
        let result = match command {
            Command::Advance { time_micros } => session.advance(TimeMicros(time_micros)),
            Command::Finish { time_micros } => session.finish(TimeMicros(time_micros)),
            Command::Input { event } => session
                .consume_runtime_input(&event)
                .map(|e| e.into_iter().collect()),
            Command::Reset { time_micros } => {
                session.reset(TimeMicros(time_micros)).map(|_| vec![])
            }
            Command::SetMode { mode } => {
                if self.notes_only && mode == SessionMode::Play {
                    return Err("loadNotes cannot switch to Play".into());
                }
                session.set_mode(mode).map(|_| vec![])
            }
            Command::SetOffset { offset_micros } => {
                session.set_judgement_offset(TimeMicros(offset_micros));
                Ok(vec![])
            }
            Command::SetAutomaticPolicy { policy } => {
                session.set_automatic_policy(policy)?;
                Ok(vec![])
            }
            Command::Snapshot {} => Ok(vec![]),
            Command::Load { .. } | Command::LoadNotes(_) => unreachable!(),
        };
        result.map_err(|e| e.to_string())
    }

    fn refresh(&mut self, events: &[JudgementEvent]) -> Result<(), String> {
        self.event_words.clear();
        for event in events {
            let id = *self
                .numeric_note_ids
                .get(&event.note_id)
                .ok_or("unknown event source identifier")?;
            append_event_words(&mut self.event_words, event, id)?;
        }
        if let Some(session) = &self.session {
            write_snapshot_words(
                &mut self.snapshot_words,
                session.fixed_snapshot(),
                events.len(),
            )?;
        }
        Ok(())
    }

    fn project_watch(&mut self, time: i64, observations: &BTreeMap<String, TimeMicros>) -> i32 {
        let result = (|| {
            let session = self.session.as_ref().ok_or("load a chart first")?;
            if session.mode() != SessionMode::Watch {
                return Err("visual projection requires Watch".to_owned());
            }
            let (snapshot, events) = session
                .project_watch_observed(TimeMicros(time), observations)
                .map_err(|e| e.to_string())?;
            self.projection_event_words.clear();
            for event in &events {
                let id = *self
                    .numeric_note_ids
                    .get(&event.note_id)
                    .ok_or("unknown projection source identifier")?;
                append_event_words(&mut self.projection_event_words, event, id)?;
            }
            write_snapshot_words(&mut self.projection_snapshot_words, snapshot, events.len())?;
            i32::try_from(events.len()).map_err(|_| "event count overflowed".to_owned())
        })();
        match result {
            Ok(count) => {
                self.error.clear();
                count
            }
            Err(error) => {
                self.projection_event_words.clear();
                self.projection_snapshot_words.fill(0);
                self.error = error.into_bytes();
                -1
            }
        }
    }

    fn fixed(&mut self, kind: u8, time: i64, observations: &BTreeMap<String, TimeMicros>) -> i32 {
        let result = self
            .session
            .as_mut()
            .ok_or_else(|| "load a chart first".to_owned())
            .and_then(|s| {
                match kind {
                    0 => s.advance(TimeMicros(time)),
                    1 => s.reset_observed(TimeMicros(time), observations),
                    _ => s.finish(TimeMicros(time)),
                }
                .map_err(|e| e.to_string())
            })
            .and_then(|events| {
                self.refresh(&events)?;
                i32::try_from(events.len()).map_err(|_| "event count overflowed".to_owned())
            });
        match result {
            Ok(count) => {
                self.error.clear();
                count
            }
            Err(e) => {
                self.error = e.into_bytes();
                -1
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cassiopeia_abi_version() -> u32 {
    ABI_VERSION
}
#[unsafe(no_mangle)]
pub extern "C" fn cassiopeia_fixed_abi_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn cassiopeia_create() -> *mut NativeHost {
    Box::into_raw(Box::new(NativeHost::default()))
}

/// # Safety
/// host is null or an exclusively owned live handle; destroy once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_destroy(host: *mut NativeHost) {
    if !host.is_null() {
        drop(unsafe { Box::from_raw(host) });
    }
}
/// # Safety
/// host is live and exclusive; request is readable for length bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_dispatch(
    host: *mut NativeHost,
    request: *const u8,
    length: usize,
) -> usize {
    if host.is_null() || request.is_null() || length > MAX_REQUEST_BYTES {
        return 0;
    }
    unsafe { &mut *host }
        .dispatch(unsafe { std::slice::from_raw_parts(request, length) })
        .len()
}
/// # Safety
/// host is live or null. Copy before next dispatch/destroy.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_reply(host: *const NativeHost) -> *const u8 {
    if host.is_null() {
        std::ptr::null()
    } else {
        unsafe { &*host }.reply.as_ptr()
    }
}
/// # Safety
/// host is a live exclusive handle. Returns event count or -1.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_update(host: *mut NativeHost, time_micros: i64) -> i32 {
    if host.is_null() {
        -1
    } else {
        unsafe { &mut *host }.fixed(0, time_micros, &BTreeMap::new())
    }
}
/// # Safety
/// host is a live exclusive handle. Rebuilt events are historical, not live emissions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_reset(host: *mut NativeHost, time_micros: i64) -> i32 {
    if host.is_null() {
        -1
    } else {
        unsafe { &mut *host }.fixed(1, time_micros, &BTreeMap::new())
    }
}
unsafe fn read_observations(
    ids: *const u32,
    times: *const i64,
    count: usize,
) -> Option<BTreeMap<String, TimeMicros>> {
    if (count > 0 && (ids.is_null() || times.is_null())) || count > MAX_REQUEST_BYTES / 12 {
        return None;
    }
    let mut observations = BTreeMap::new();
    if count > 0 {
        for (&id, &time) in unsafe { std::slice::from_raw_parts(ids, count) }
            .iter()
            .zip(unsafe { std::slice::from_raw_parts(times, count) })
        {
            observations.insert(id.to_string(), TimeMicros(time));
        }
    }
    Some(observations)
}

/// # Safety
/// host is live and exclusive. ids/times each contain count readable entries.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_reset_observed(
    host: *mut NativeHost,
    time_micros: i64,
    ids: *const u32,
    times: *const i64,
    count: usize,
) -> i32 {
    if host.is_null() {
        return -1;
    }
    let Some(observations) = (unsafe { read_observations(ids, times, count) }) else {
        return -1;
    };
    unsafe { &mut *host }.fixed(1, time_micros, &observations)
}
/// # Safety
/// host is live and exclusive. ids/times each contain count readable entries.
/// Projection writes only its separate buffers, leaving gameplay unchanged.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_project_watch_observed(
    host: *mut NativeHost,
    time_micros: i64,
    ids: *const u32,
    times: *const i64,
    count: usize,
) -> i32 {
    if host.is_null() {
        return -1;
    }
    let Some(observations) = (unsafe { read_observations(ids, times, count) }) else {
        return -1;
    };
    unsafe { &mut *host }.project_watch(time_micros, &observations)
}
/// # Safety
/// host is live or null. Borrow until next projection/load/destruction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_projection_snapshot_words(
    host: *const NativeHost,
) -> *const i64 {
    if host.is_null() {
        std::ptr::null()
    } else {
        unsafe { &*host }.projection_snapshot_words.as_ptr()
    }
}
/// # Safety
/// host is live or null. Borrow until next projection/load/destruction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_projection_event_words(host: *const NativeHost) -> *const i64 {
    if host.is_null() {
        std::ptr::null()
    } else {
        unsafe { &*host }.projection_event_words.as_ptr()
    }
}
/// # Safety
/// host is a live exclusive handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_finish(host: *mut NativeHost, time_micros: i64) -> i32 {
    if host.is_null() {
        -1
    } else {
        unsafe { &mut *host }.fixed(2, time_micros, &BTreeMap::new())
    }
}
/// # Safety
/// host is live or null; pointer valid until next operation/destruction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_snapshot_words(host: *const NativeHost) -> *const i64 {
    if host.is_null() {
        std::ptr::null()
    } else {
        unsafe { &*host }.snapshot_words.as_ptr()
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn cassiopeia_snapshot_word_count() -> usize {
    SNAPSHOT_WORDS
}
/// # Safety
/// host is live or null; pointer valid until next operation/destruction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_event_words(host: *const NativeHost) -> *const i64 {
    if host.is_null() {
        std::ptr::null()
    } else {
        unsafe { &*host }.event_words.as_ptr()
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn cassiopeia_event_stride() -> usize {
    EVENT_STRIDE
}
/// # Safety
/// host is live or null; pointer valid until next operation/destruction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_error(host: *const NativeHost) -> *const u8 {
    if host.is_null() {
        std::ptr::null()
    } else {
        unsafe { &*host }.error.as_ptr()
    }
}
/// # Safety
/// host is live or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_error_length(host: *const NativeHost) -> usize {
    if host.is_null() {
        0
    } else {
        unsafe { &*host }.error.len()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn call(h: &mut NativeHost, v: serde_json::Value) -> serde_json::Value {
        serde_json::from_slice(h.dispatch(v.to_string().as_bytes())).unwrap()
    }
    #[test]
    fn lifecycle_and_invalid_load_preserve_session() {
        let mut h = NativeHost::default();
        assert_eq!(call(&mut h, json!({"command":"snapshot"}))["ok"], false);
        let chart = json!({"format":"org.haneoka.cassiopeia.runtime","version":1,"laneCount":24,"notes":[],"lines":[]});
        assert_eq!(
            call(
                &mut h,
                json!({"command":"load","chart":chart,"mode":"play"})
            )["ok"],
            true
        );
        assert_eq!(
            call(&mut h, json!({"command":"advance","timeMicros":123000}))["snapshot"]["time"],
            123000
        );
        assert_eq!(
            call(&mut h, json!({"command":"load","chart":{},"mode":"play"}))["ok"],
            false
        );
        assert_eq!(
            call(&mut h, json!({"command":"snapshot"}))["snapshot"]["time"],
            123000
        );
        assert_eq!(
            call(&mut h, json!({"command":"reset","timeMicros":0}))["snapshot"]["time"],
            0
        );
        assert_eq!(
            call(&mut h, json!({"command":"snapshot","unknown":1}))["ok"],
            false
        );
    }
}
