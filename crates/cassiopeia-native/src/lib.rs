//! Synchronous native ABI. Each handle owns its session and reply buffer.
//! No globals, renderer, audio thread, network, or platform vibration APIs.
use haneoka_cassiopeia_core::{
    GameplaySession, JudgementEvent, RuntimeChartV1, RuntimeInputEvent, SessionMode, TimeMicros,
};
use serde::Deserialize;
use serde_json::json;

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
    },
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
    Snapshot {},
}

#[derive(Default)]
pub struct NativeHost {
    session: Option<GameplaySession>,
    reply: Vec<u8>,
}

impl NativeHost {
    /// Replies remain valid until the next dispatch or handle destruction.
    pub fn dispatch(&mut self, request: &[u8]) -> &[u8] {
        let result = self.execute(request);
        let value = match result {
            Ok(events) => json!({"abiVersion": ABI_VERSION, "ok": true,
                "snapshot": self.session.as_ref().map(GameplaySession::snapshot), "events": events}),
            Err(error) => json!({"abiVersion": ABI_VERSION, "ok": false, "error": error}),
        };
        self.reply.clear();
        serde_json::to_writer(&mut self.reply, &value)
            .expect("JSON reply contains only serializable values");
        &self.reply
    }

    fn execute(&mut self, request: &[u8]) -> Result<Vec<JudgementEvent>, String> {
        if request.len() > MAX_REQUEST_BYTES {
            return Err("request exceeds 32 MiB".into());
        }
        let command: Command = serde_json::from_slice(request).map_err(|e| e.to_string())?;
        if let Command::Load {
            chart,
            mode,
            offset_micros,
        } = command
        {
            // Validate fully before replacing a live session.
            let session =
                GameplaySession::from_runtime_chart(chart, mode, TimeMicros(offset_micros))
                    .map_err(|e| e.to_string())?;
            self.session = Some(session);
            return Ok(vec![]);
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
            Command::SetMode { mode } => session.set_mode(mode).map(|_| vec![]),
            Command::SetOffset { offset_micros } => {
                session.set_judgement_offset(TimeMicros(offset_micros));
                Ok(vec![])
            }
            Command::Snapshot {} => Ok(vec![]),
            Command::Load { .. } => unreachable!(),
        };
        result.map_err(|e| e.to_string())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cassiopeia_abi_version() -> u32 {
    ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn cassiopeia_create() -> *mut NativeHost {
    Box::into_raw(Box::new(NativeHost::default()))
}

/// # Safety
/// `host` must be null or a live pointer returned by create; destroy exactly once.
/// Do not concurrently call functions for the same handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_destroy(host: *mut NativeHost) {
    if !host.is_null() {
        drop(unsafe { Box::from_raw(host) });
    }
}

/// Returns reply length, or zero for an invalid pointer/oversized request.
/// # Safety
/// `host` must be live, exclusively borrowed, and `request` readable for `length` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_dispatch(
    host: *mut NativeHost,
    request: *const u8,
    length: usize,
) -> usize {
    if host.is_null() || request.is_null() || length > MAX_REQUEST_BYTES {
        return 0;
    }
    let host = unsafe { &mut *host };
    host.dispatch(unsafe { std::slice::from_raw_parts(request, length) })
        .len()
}

/// # Safety
/// `host` must be null or live. Copy the reply before the next dispatch/destroy.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cassiopeia_reply(host: *const NativeHost) -> *const u8 {
    if host.is_null() {
        return std::ptr::null();
    }
    unsafe { &*host }.reply.as_ptr()
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
