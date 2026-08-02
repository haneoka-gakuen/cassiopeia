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
mod combo_cutin;

use std::collections::BTreeMap;

pub use chart_binary::{
    CHART_BINARY_MAGIC, CHART_BINARY_VERSION, CHART_HEADER_BYTES, CHART_LINE_HEADER_BYTES,
    CHART_NOTE_BYTES, ChartBinaryError, decode_runtime_chart_v1, encode_runtime_chart_v1,
};
pub use combo_cutin::*;
use haneoka_cassiopeia_core::{
    CameraChorusResource, CameraEffect, CameraEffectsResource, CameraEvaluation,
    CameraFinishEvaluation, CameraFinishSequence, CameraIntroductionEvaluation,
    CameraIntroductionSequence, CameraProfile, CameraTimelineResource, GameplaySession,
    InputAction, InputVector, JudgementEvent, LanePosition, PerformanceClock, PlaybackRate,
    PlaybackState, RuntimeInputEvent, ScreenMode, ScreenResources, SessionMode, TimeMicros,
    resolve_screen_mode,
};
use wasm_bindgen::{JsCast, prelude::*};

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

pub const CAMERA_FRAME_ABI_VERSION: u32 = 1;
pub const CAMERA_FRAME_WORDS: usize = 13;
pub const CAMERA_FRAME_ABI: usize = 0;
pub const CAMERA_FRAME_AVAILABLE: usize = 1;
pub const CAMERA_FRAME_ACTIVE_CLIPS: usize = 2;
pub const CAMERA_FRAME_INCOMING_WEIGHT: usize = 3;
pub const CAMERA_FRAME_INCOMING_INDEX: usize = 4;
pub const CAMERA_FRAME_POSITION_X: usize = 5;
pub const CAMERA_FRAME_POSITION_Y: usize = 6;
pub const CAMERA_FRAME_POSITION_Z: usize = 7;
pub const CAMERA_FRAME_ROTATION_X: usize = 8;
pub const CAMERA_FRAME_ROTATION_Y: usize = 9;
pub const CAMERA_FRAME_ROTATION_Z: usize = 10;
pub const CAMERA_FRAME_ROTATION_W: usize = 11;
pub const CAMERA_FRAME_VERTICAL_FOV: usize = 12;

pub const CAMERA_INTRODUCTION_FRAME_ABI_VERSION: u32 = 1;
pub const CAMERA_INTRODUCTION_FRAME_WORDS: usize = 5;
pub const CAMERA_INTRODUCTION_FRAME_ABI: usize = 0;
pub const CAMERA_INTRODUCTION_FRAME_AVAILABLE: usize = 1;
pub const CAMERA_INTRODUCTION_FRAME_POSITION_X: usize = 2;
pub const CAMERA_INTRODUCTION_FRAME_POSITION_Y: usize = 3;
pub const CAMERA_INTRODUCTION_FRAME_POSITION_Z: usize = 4;

pub const CAMERA_FINISH_FRAME_ABI_VERSION: u32 = 1;
pub const CAMERA_FINISH_FRAME_WORDS: usize = 6;
pub const CAMERA_FINISH_FRAME_ABI: usize = 0;
pub const CAMERA_FINISH_FRAME_AVAILABLE: usize = 1;
pub const CAMERA_FINISH_FRAME_ACTIVE_CLIPS: usize = 2;
pub const CAMERA_FINISH_FRAME_INCOMING_WEIGHT: usize = 3;
pub const CAMERA_FINISH_FRAME_INCOMING_INDEX: usize = 4;
pub const CAMERA_FINISH_FRAME_VERTICAL_FOV: usize = 5;

#[wasm_bindgen]
#[repr(u8)]
pub enum CameraFrameWord {
    AbiVersion = 0,
    Available = 1,
    ActiveClips = 2,
    IncomingWeight = 3,
    IncomingSerializedIndex = 4,
    PositionX = 5,
    PositionY = 6,
    PositionZ = 7,
    RotationX = 8,
    RotationY = 9,
    RotationZ = 10,
    RotationW = 11,
    VerticalFovDegrees = 12,
}

/// Position-only stage-camera introduction ABI. The host preserves orientation and lens.
#[wasm_bindgen]
#[repr(u8)]
pub enum CameraIntroductionFrameWord {
    AbiVersion = 0,
    Available = 1,
    PositionX = 2,
    PositionY = 3,
    PositionZ = 4,
}

/// Lens-only finish ABI. The host preserves the current camera pose.
#[wasm_bindgen]
#[repr(u8)]
pub enum CameraFinishFrameWord {
    AbiVersion = 0,
    Available = 1,
    ActiveClips = 2,
    IncomingWeight = 3,
    IncomingSerializedIndex = 4,
    VerticalFovDegrees = 5,
}

/// Owns one parsed camera resource and a reusable 13-word frame region.
#[derive(Debug)]
#[wasm_bindgen(js_name = CassiopeiaCameraTimeline)]
pub struct WasmCameraTimeline {
    resource: CameraTimelineResource,
    effects: Option<CameraEffectsResource>,
    chorus: Option<CameraChorusResource>,
    frame_words: [f64; CAMERA_FRAME_WORDS],
    chorus_frame_words: [f64; CAMERA_FRAME_WORDS],
    introduction_frame_words: [f64; CAMERA_INTRODUCTION_FRAME_WORDS],
    finish_frame_words: [f64; CAMERA_FINISH_FRAME_WORDS],
}

#[wasm_bindgen(js_class = CassiopeiaCameraTimeline)]
impl WasmCameraTimeline {
    #[wasm_bindgen(constructor)]
    pub fn new(resource_json: &[u8]) -> Result<WasmCameraTimeline, JsError> {
        Self::from_json(resource_json).map_err(js_error)
    }

    /// Parses and validates a separate optional camera-effects contract.
    #[wasm_bindgen(js_name = withEffects)]
    pub fn with_effects(
        resource_json: &[u8],
        effects_json: &[u8],
    ) -> Result<WasmCameraTimeline, JsError> {
        Self::from_json_with_effects(resource_json, effects_json).map_err(js_error)
    }

    /// Parses the score Timeline, effects contract, and independent chorus camera.
    #[wasm_bindgen(js_name = withEffectsAndChorus)]
    pub fn with_effects_and_chorus(
        resource_json: &[u8],
        effects_json: &[u8],
        chorus_json: &[u8],
    ) -> Result<WasmCameraTimeline, JsError> {
        Self::from_json_with_effects_and_chorus(resource_json, effects_json, chorus_json)
            .map_err(js_error)
    }

    /// Writes ABI, availability, blend, transform, and FOV into one stable region.
    pub fn evaluate(&mut self, profile: u8, time_seconds: f64) -> Result<bool, JsError> {
        self.evaluate_frame(profile, time_seconds).map_err(js_error)
    }

    /// Samples Unity-native Perlin with a pauseable scene uptime that is
    /// independent of score time. Calling this after a seek must retain the
    /// existing scene uptime instead of deriving it from the new score time.
    #[wasm_bindgen(js_name = evaluateWithEffects)]
    pub fn evaluate_with_effects(
        &mut self,
        profile: u8,
        score_time_seconds: f64,
        scene_uptime_seconds: f64,
    ) -> Result<bool, JsError> {
        self.evaluate_effects_frame(profile, score_time_seconds, scene_uptime_seconds)
            .map_err(js_error)
    }

    /// Samples the position-only introduction track into its stable frame region.
    #[wasm_bindgen(js_name = evaluateIntroduction)]
    pub fn evaluate_introduction(&mut self, time_seconds: f64) -> Result<bool, JsError> {
        self.evaluate_introduction_frame(time_seconds)
            .map_err(js_error)
    }

    /// Samples the lens-only finish track into its stable frame region.
    #[wasm_bindgen(js_name = evaluateFinish)]
    pub fn evaluate_finish(&mut self, time_seconds: f64) -> Result<bool, JsError> {
        self.evaluate_finish_frame(time_seconds).map_err(js_error)
    }

    /// Samples the full one-shot chorus camera into its independent frame region.
    #[wasm_bindgen(js_name = evaluateChorus)]
    pub fn evaluate_chorus(&mut self, time_seconds: f64) -> Result<bool, JsError> {
        self.evaluate_chorus_frame(time_seconds).map_err(js_error)
    }

    #[wasm_bindgen(js_name = durationSeconds)]
    pub fn duration_seconds(&self, profile: u8) -> Result<f64, JsError> {
        self.profile(profile)
            .map(|timeline| timeline.duration_seconds)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = frameRate)]
    pub fn frame_rate(&self, profile: u8) -> Result<f64, JsError> {
        self.profile(profile)
            .map(|timeline| timeline.frame_rate)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = hasIntroduction)]
    pub fn has_introduction(&self) -> bool {
        self.resource.introduction().is_some()
    }

    #[wasm_bindgen(js_name = introductionDurationSeconds)]
    pub fn introduction_duration_seconds(&self) -> Result<f64, JsError> {
        self.introduction()
            .map(|introduction| introduction.duration_seconds)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = introductionFrameRate)]
    pub fn introduction_frame_rate(&self) -> Result<f64, JsError> {
        self.introduction()
            .map(|introduction| introduction.frame_rate)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = introductionInheritsOrientationAndLens)]
    pub fn introduction_inherits_orientation_and_lens(&self) -> Result<bool, JsError> {
        self.introduction()
            .map(|introduction| introduction.inherits_orientation_and_lens)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = hasFinish)]
    pub fn has_finish(&self) -> bool {
        self.resource.finish().is_some()
    }

    #[wasm_bindgen(js_name = finishDurationSeconds)]
    pub fn finish_duration_seconds(&self) -> Result<f64, JsError> {
        self.finish()
            .map(|finish| finish.duration_seconds)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = finishFrameRate)]
    pub fn finish_frame_rate(&self) -> Result<f64, JsError> {
        self.finish()
            .map(|finish| finish.frame_rate)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = hasChorus)]
    pub fn has_chorus(&self) -> bool {
        self.chorus.is_some()
    }

    #[wasm_bindgen(js_name = chorusDurationSeconds)]
    pub fn chorus_duration_seconds(&self) -> Result<f64, JsError> {
        self.chorus()
            .map(|chorus| chorus.duration_seconds)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = chorusFrameRate)]
    pub fn chorus_frame_rate(&self) -> Result<f64, JsError> {
        self.chorus()
            .map(|chorus| chorus.frame_rate)
            .map_err(js_error)
    }

    /// Pointer to a reusable `Float64Array`-compatible camera frame region.
    #[wasm_bindgen(js_name = frameBufferPtr)]
    pub fn frame_buffer_ptr(&self) -> usize {
        self.frame_words.as_ptr() as usize
    }

    #[wasm_bindgen(js_name = frameBufferLen)]
    pub fn frame_buffer_len(&self) -> usize {
        CAMERA_FRAME_WORDS
    }

    /// Pointer to a second full-camera frame so score and chorus samples can coexist.
    #[wasm_bindgen(js_name = chorusFrameBufferPtr)]
    pub fn chorus_frame_buffer_ptr(&self) -> usize {
        self.chorus_frame_words.as_ptr() as usize
    }

    #[wasm_bindgen(js_name = chorusFrameBufferLen)]
    pub fn chorus_frame_buffer_len(&self) -> usize {
        CAMERA_FRAME_WORDS
    }

    /// Pointer to a reusable `Float64Array`-compatible introduction position region.
    #[wasm_bindgen(js_name = introductionFrameBufferPtr)]
    pub fn introduction_frame_buffer_ptr(&self) -> usize {
        self.introduction_frame_words.as_ptr() as usize
    }

    #[wasm_bindgen(js_name = introductionFrameBufferLen)]
    pub fn introduction_frame_buffer_len(&self) -> usize {
        CAMERA_INTRODUCTION_FRAME_WORDS
    }

    /// Pointer to a reusable `Float64Array`-compatible finish lens region.
    #[wasm_bindgen(js_name = finishFrameBufferPtr)]
    pub fn finish_frame_buffer_ptr(&self) -> usize {
        self.finish_frame_words.as_ptr() as usize
    }

    #[wasm_bindgen(js_name = finishFrameBufferLen)]
    pub fn finish_frame_buffer_len(&self) -> usize {
        CAMERA_FINISH_FRAME_WORDS
    }

    #[wasm_bindgen(js_name = hasEffectsContract)]
    pub fn has_effects_contract(&self) -> bool {
        self.effects.is_some()
    }

    /// Reports exact pure sampling and active Timeline blend support for this contract.
    #[wasm_bindgen(js_name = effectsSamplingSupported)]
    pub fn effects_sampling_supported(&self) -> bool {
        self.effects
            .as_ref()
            .is_some_and(CameraEffectsResource::supports_exact_sampling)
    }

    /// Inactive virtual-camera StandbyUpdate/RoundRobin phase is not modeled by
    /// this stateless boundary. Hosts currently provide continuously advancing
    /// scene uptime for every selected input.
    #[wasm_bindgen(js_name = effectsStandbyRoundRobinSupported)]
    pub fn effects_standby_round_robin_supported(&self) -> bool {
        false
    }

    #[wasm_bindgen(js_name = cameraHasPerlin)]
    pub fn camera_has_perlin(&self, profile: u8, camera_name: &str) -> Result<bool, JsError> {
        self.effect(profile, camera_name)
            .map(|effect| effect.perlin.is_some())
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = cameraUsesCustomLookAt)]
    pub fn camera_uses_custom_look_at(
        &self,
        profile: u8,
        camera_name: &str,
    ) -> Result<bool, JsError> {
        self.effect(profile, camera_name)
            .map(|effect| effect.custom_look_at_target)
            .map_err(js_error)
    }
}

impl WasmCameraTimeline {
    fn from_json(resource_json: &[u8]) -> Result<Self, String> {
        let resource: CameraTimelineResource =
            serde_json::from_slice(resource_json).map_err(|error| error.to_string())?;
        Ok(Self {
            resource,
            effects: None,
            chorus: None,
            frame_words: empty_camera_frame(),
            chorus_frame_words: empty_camera_frame(),
            introduction_frame_words: empty_camera_introduction_frame(),
            finish_frame_words: empty_camera_finish_frame(),
        })
    }

    fn from_json_with_effects(resource_json: &[u8], effects_json: &[u8]) -> Result<Self, String> {
        let resource: CameraTimelineResource =
            serde_json::from_slice(resource_json).map_err(|error| error.to_string())?;
        let effects: CameraEffectsResource =
            serde_json::from_slice(effects_json).map_err(|error| error.to_string())?;
        effects
            .validate_timeline_coverage(&resource)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            resource,
            effects: Some(effects),
            chorus: None,
            frame_words: empty_camera_frame(),
            chorus_frame_words: empty_camera_frame(),
            introduction_frame_words: empty_camera_introduction_frame(),
            finish_frame_words: empty_camera_finish_frame(),
        })
    }

    fn from_json_with_effects_and_chorus(
        resource_json: &[u8],
        effects_json: &[u8],
        chorus_json: &[u8],
    ) -> Result<Self, String> {
        let mut timeline = Self::from_json_with_effects(resource_json, effects_json)?;
        let chorus: CameraChorusResource =
            serde_json::from_slice(chorus_json).map_err(|error| error.to_string())?;
        chorus.validate().map_err(|error| error.to_string())?;
        timeline.chorus = Some(chorus);
        Ok(timeline)
    }

    fn profile(
        &self,
        profile: u8,
    ) -> Result<&haneoka_cassiopeia_core::CameraTimelineProfile, String> {
        let profile = CameraProfile::try_from(profile).map_err(|error| error.to_string())?;
        self.resource
            .profile(profile)
            .ok_or_else(|| format!("camera profile is unavailable: {profile:?}"))
    }

    fn introduction(&self) -> Result<&CameraIntroductionSequence, String> {
        self.resource
            .introduction()
            .ok_or_else(|| "camera introduction is unavailable".to_owned())
    }

    fn finish(&self) -> Result<&CameraFinishSequence, String> {
        self.resource
            .finish()
            .ok_or_else(|| "camera finish is unavailable".to_owned())
    }

    fn chorus(&self) -> Result<&CameraChorusResource, String> {
        self.chorus
            .as_ref()
            .ok_or_else(|| "chorus camera is unavailable".to_owned())
    }

    fn evaluate_frame(&mut self, profile: u8, time_seconds: f64) -> Result<bool, String> {
        self.frame_words = empty_camera_frame();
        let evaluation = self
            .profile(profile)?
            .evaluate(&self.resource.curves, time_seconds)
            .map_err(|error| error.to_string())?;
        let Some(evaluation) = evaluation else {
            return Ok(false);
        };
        write_camera_frame(&mut self.frame_words, evaluation);
        Ok(true)
    }

    fn evaluate_effects_frame(
        &mut self,
        profile: u8,
        score_time_seconds: f64,
        scene_uptime_seconds: f64,
    ) -> Result<bool, String> {
        self.frame_words = empty_camera_frame();
        let profile = CameraProfile::try_from(profile).map_err(|error| error.to_string())?;
        let evaluation = self
            .effects
            .as_ref()
            .ok_or_else(|| "camera-effects contract is unavailable".to_owned())?
            .evaluate_timeline(
                &self.resource,
                profile,
                score_time_seconds,
                scene_uptime_seconds,
            )
            .map_err(|error| error.to_string())?;
        let Some(evaluation) = evaluation else {
            return Ok(false);
        };
        write_camera_frame(&mut self.frame_words, evaluation);
        Ok(true)
    }

    fn evaluate_introduction_frame(&mut self, time_seconds: f64) -> Result<bool, String> {
        self.introduction_frame_words = empty_camera_introduction_frame();
        let evaluation = self
            .resource
            .introduction()
            .map(|introduction| introduction.evaluate(time_seconds))
            .transpose()
            .map_err(|error| error.to_string())?
            .flatten();
        let Some(evaluation) = evaluation else {
            return Ok(false);
        };
        write_camera_introduction_frame(&mut self.introduction_frame_words, evaluation);
        Ok(true)
    }

    fn evaluate_finish_frame(&mut self, time_seconds: f64) -> Result<bool, String> {
        self.finish_frame_words = empty_camera_finish_frame();
        let evaluation = self
            .resource
            .finish()
            .map(|finish| finish.evaluate(&self.resource.curves, time_seconds))
            .transpose()
            .map_err(|error| error.to_string())?
            .flatten();
        let Some(evaluation) = evaluation else {
            return Ok(false);
        };
        write_camera_finish_frame(&mut self.finish_frame_words, evaluation);
        Ok(true)
    }

    fn evaluate_chorus_frame(&mut self, time_seconds: f64) -> Result<bool, String> {
        self.chorus_frame_words = empty_camera_frame();
        let evaluation = self
            .chorus()?
            .evaluate(time_seconds)
            .map_err(|error| error.to_string())?;
        let Some(state) = evaluation else {
            return Ok(false);
        };
        write_camera_frame(
            &mut self.chorus_frame_words,
            CameraEvaluation {
                state,
                active_clips: 1,
                incoming_weight: 1.0,
                incoming_serialized_index: None,
            },
        );
        Ok(true)
    }

    fn effect(&self, profile: u8, camera_name: &str) -> Result<CameraEffect<'_>, String> {
        let profile = CameraProfile::try_from(profile).map_err(|error| error.to_string())?;
        self.effects
            .as_ref()
            .ok_or_else(|| "camera-effects contract is unavailable".to_owned())?
            .camera_effect(profile, camera_name)
            .ok_or_else(|| format!("camera effect is unavailable: {camera_name}"))
    }
}

const fn empty_camera_frame() -> [f64; CAMERA_FRAME_WORDS] {
    let mut result = [0.0; CAMERA_FRAME_WORDS];
    result[CAMERA_FRAME_ABI] = CAMERA_FRAME_ABI_VERSION as f64;
    result[CAMERA_FRAME_INCOMING_INDEX] = -1.0;
    result
}

fn write_camera_frame(words: &mut [f64; CAMERA_FRAME_WORDS], evaluation: CameraEvaluation) {
    words[CAMERA_FRAME_AVAILABLE] = 1.0;
    words[CAMERA_FRAME_ACTIVE_CLIPS] = f64::from(evaluation.active_clips);
    words[CAMERA_FRAME_INCOMING_WEIGHT] = evaluation.incoming_weight;
    words[CAMERA_FRAME_INCOMING_INDEX] = evaluation
        .incoming_serialized_index
        .map(f64::from)
        .unwrap_or(-1.0);
    words[CAMERA_FRAME_POSITION_X..=CAMERA_FRAME_POSITION_Z]
        .copy_from_slice(&evaluation.state.position);
    words[CAMERA_FRAME_ROTATION_X..=CAMERA_FRAME_ROTATION_W]
        .copy_from_slice(&evaluation.state.rotation);
    words[CAMERA_FRAME_VERTICAL_FOV] = evaluation.state.vertical_fov_degrees;
}

const fn empty_camera_introduction_frame() -> [f64; CAMERA_INTRODUCTION_FRAME_WORDS] {
    let mut result = [0.0; CAMERA_INTRODUCTION_FRAME_WORDS];
    result[CAMERA_INTRODUCTION_FRAME_ABI] = CAMERA_INTRODUCTION_FRAME_ABI_VERSION as f64;
    result
}

fn write_camera_introduction_frame(
    words: &mut [f64; CAMERA_INTRODUCTION_FRAME_WORDS],
    evaluation: CameraIntroductionEvaluation,
) {
    words[CAMERA_INTRODUCTION_FRAME_AVAILABLE] = 1.0;
    words[CAMERA_INTRODUCTION_FRAME_POSITION_X..=CAMERA_INTRODUCTION_FRAME_POSITION_Z]
        .copy_from_slice(&evaluation.position);
}

const fn empty_camera_finish_frame() -> [f64; CAMERA_FINISH_FRAME_WORDS] {
    let mut result = [0.0; CAMERA_FINISH_FRAME_WORDS];
    result[CAMERA_FINISH_FRAME_ABI] = CAMERA_FINISH_FRAME_ABI_VERSION as f64;
    result[CAMERA_FINISH_FRAME_INCOMING_INDEX] = -1.0;
    result
}

fn write_camera_finish_frame(
    words: &mut [f64; CAMERA_FINISH_FRAME_WORDS],
    evaluation: CameraFinishEvaluation,
) {
    words[CAMERA_FINISH_FRAME_AVAILABLE] = 1.0;
    words[CAMERA_FINISH_FRAME_ACTIVE_CLIPS] = f64::from(evaluation.active_clips);
    words[CAMERA_FINISH_FRAME_INCOMING_WEIGHT] = evaluation.incoming_weight;
    words[CAMERA_FINISH_FRAME_INCOMING_INDEX] = evaluation
        .incoming_serialized_index
        .map(f64::from)
        .unwrap_or(-1.0);
    words[CAMERA_FINISH_FRAME_VERTICAL_FOV] = evaluation.vertical_fov_degrees;
}

/// Returns the current module memory for zero-copy typed-array views.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "WebAssembly.Memory")]
    pub type CassiopeiaWasmMemory;
}

#[wasm_bindgen(js_name = cassiopeiaMemory)]
pub fn cassiopeia_memory() -> CassiopeiaWasmMemory {
    wasm_bindgen::memory().unchecked_into()
}

/// Deterministic transport for camera, motion, light, and penlight schedulers.
#[derive(Debug)]
#[wasm_bindgen(js_name = CassiopeiaPerformanceClock)]
pub struct WasmPerformanceClock {
    clock: PerformanceClock,
}

#[wasm_bindgen(js_class = CassiopeiaPerformanceClock)]
impl WasmPerformanceClock {
    #[wasm_bindgen(constructor)]
    pub fn new(host_time_micros: i64, performance_time_micros: i64) -> Self {
        Self {
            clock: PerformanceClock::new(
                TimeMicros(host_time_micros),
                TimeMicros(performance_time_micros),
            ),
        }
    }

    pub fn pause(&mut self, host_time_micros: i64) -> Result<(), JsError> {
        self.clock
            .pause(TimeMicros(host_time_micros))
            .map_err(js_error)
    }

    pub fn resume(&mut self, host_time_micros: i64) -> Result<(), JsError> {
        self.clock
            .resume(TimeMicros(host_time_micros))
            .map_err(js_error)
    }

    pub fn seek(
        &mut self,
        host_time_micros: i64,
        performance_time_micros: i64,
    ) -> Result<(), JsError> {
        self.clock
            .seek(
                TimeMicros(host_time_micros),
                TimeMicros(performance_time_micros),
            )
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = setRate)]
    pub fn set_rate(&mut self, host_time_micros: i64, rate_millionths: u32) -> Result<(), JsError> {
        let rate = PlaybackRate::from_millionths(rate_millionths).map_err(js_error)?;
        self.clock
            .set_rate(TimeMicros(host_time_micros), rate)
            .map_err(js_error)
    }

    pub fn rebuild(
        &mut self,
        host_time_micros: i64,
        performance_time_micros: i64,
        rate_millionths: u32,
        playing: bool,
    ) -> Result<(), JsError> {
        let rate = PlaybackRate::from_millionths(rate_millionths).map_err(js_error)?;
        self.clock.rebuild(
            TimeMicros(host_time_micros),
            TimeMicros(performance_time_micros),
            rate,
            if playing {
                PlaybackState::Playing
            } else {
                PlaybackState::Paused
            },
        );
        Ok(())
    }

    #[wasm_bindgen(js_name = timeMicros)]
    pub fn time_micros(&self, host_time_micros: i64) -> Result<i64, JsError> {
        self.clock
            .snapshot(TimeMicros(host_time_micros))
            .map(|snapshot| snapshot.time.0)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = rateMillionths)]
    pub fn rate_millionths(&self, host_time_micros: i64) -> Result<u32, JsError> {
        self.clock
            .snapshot(TimeMicros(host_time_micros))
            .map(|snapshot| snapshot.rate.millionths())
            .map_err(js_error)
    }

    pub fn playing(&self, host_time_micros: i64) -> Result<bool, JsError> {
        self.clock
            .snapshot(TimeMicros(host_time_micros))
            .map(|snapshot| snapshot.state == PlaybackState::Playing)
            .map_err(js_error)
    }

    pub fn revision(&self, host_time_micros: i64) -> Result<u64, JsError> {
        self.clock
            .snapshot(TimeMicros(host_time_micros))
            .map(|snapshot| snapshot.revision)
            .map_err(js_error)
    }
}

#[wasm_bindgen(js_name = resolvePerformanceScreenMode)]
pub fn resolve_performance_screen_mode(
    requested: u8,
    live2d_stage: bool,
    main_music_video: bool,
    band_video_jockey: bool,
) -> Result<u8, JsError> {
    let requested = ScreenMode::try_from(requested).map_err(js_error)?;
    Ok(resolve_screen_mode(
        requested,
        ScreenResources {
            live2d_stage,
            main_music_video,
            band_video_jockey,
        },
    )
    .effective
    .into())
}

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

    const CAMERA_RESOURCE: &str = r#"{
      "schema":"caph-live-camera-timelines-v1",
      "curves":{"linear":[[0,0,1,1,0,0,0],[1,1,1,1,0,0,0]]},
      "profiles":{"low":{
        "durationSeconds":2,"frameRate":60,"wrapMode":2,
        "baseIntroCameraName":null,"baseIntroCamera":null,
        "cameras":{
          "a":{"position":[0,0,0],"rotation":[0,0,0,1],"verticalFovDegrees":40},
          "b":{"position":[10,20,30],"rotation":[0,0,1,0],"verticalFovDegrees":60}
        },
        "clips":[
          ["a",0,2,-1,-1,"linear","linear",0,0,0,1,7],
          ["b",1,1,1,-1,"linear","linear",0,0,0,1,8]
        ]
      }}
    }"#;

    const CAMERA_INTRODUCTION_RESOURCE: &str = r#"{
      "schema":"org.haneoka.caph.live-camera-timelines",
      "curves":{"empty":[]},
      "profiles":{},
      "sequences":{"introduction":{
        "type":"position-animation",
        "durationSeconds":4,
        "frameRate":60,
        "wrapMode":2,
        "inheritsOrientationAndLens":true,
        "animation":{
          "startSeconds":1,
          "durationSeconds":3,
          "sourceDurationSeconds":1,
          "preExtrapolation":"hold",
          "postExtrapolation":"hold",
          "positionCurves":{
            "x":[[0,0,0,2,10],[1,0,0,0,12]],
            "y":[[0,1,0,0,20],[1,0,0,0,21]],
            "z":[[0,0,3,0,30],[1,0,0,0,33]]
          }
        }
      },"finish":{
        "type":"camera-blend",
        "durationSeconds":3,
        "frameRate":60,
        "wrapMode":2,
        "cameras":{
          "from":{"position":[0,0,-10],"rotation":[0,0,0,1],"verticalFovDegrees":40},
          "to":{"position":[0,0,-10],"rotation":[0,0,0,1],"verticalFovDegrees":70}
        },
        "clips":[
          ["from",0,3,-1,3,"empty","empty",0,0,0,1,0],
          ["to",0,3,3,-1,"empty","empty",0,0,0,1,1]
        ]
      }}
    }"#;

    const CAMERA_EFFECTS: &str = r#"{
      "schema":"org.haneoka.caph.live-camera-effects",
      "schemaVersion":1,
      "channelTuple":["amplitude","frequency","constant"],
      "octaveTuple":["x","y","z"],
      "sampling":{
        "cinemachine":"3.1",
        "timeBase":"absolute-seconds-times-frequency-gain",
        "noiseFunction":"unity-mathf-perlin-noise-2d",
        "implementationStatus":"parameters-only"
      },
      "noiseProfiles":{"handheld":{"positionOctaves":[],"orientationOctaves":[
        [[4,0.2,false],[2,0.15,false],[0,0,false]]
      ]}},
      "profiles":{"low":{
        "cameraNames":["a","b"],
        "customLookAtTarget":["b"],
        "perlin":{
          "enabled":true,
          "noiseProfile":"handheld",
          "pivotOffset":[0,0,0],
          "noiseOffsets":[347.368896484375,731.6524658203125,-17.897705078125],
          "defaultGain":[0.1,1],
          "gainOverrides":{"b":[0.2,2]}
        }
      }}
    }"#;

    const CAMERA_CHORUS: &str = r#"{
      "schema":"org.haneoka.caph.live-chorus-camera",
      "schemaVersion":1,
      "durationSeconds":10,
      "frameRate":60,
      "playback":"once",
      "completion":"restore-score-timeline",
      "coefficientTuple":["timeSeconds","a","b","c","d"],
      "channels":{
        "position":{
          "x":[[0,0,0,0,0]],
          "y":[[0,0,0,0,1]],
          "z":[[0,0,0,0,-15]]
        },
        "eulerDegrees":{
          "x":[[0,0,0,0,-4]],
          "y":[[0,0,0,0,0]],
          "z":[[0,0,0,0,0]]
        },
        "verticalFovDegrees":[[0,0,0,0,20]]
      }
    }"#;

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
    fn camera_boundary_parses_once_and_reuses_one_fixed_frame_region() {
        let mut timeline = WasmCameraTimeline::from_json(CAMERA_RESOURCE.as_bytes()).unwrap();
        assert!(!timeline.has_effects_contract());
        assert!(!timeline.effects_sampling_supported());
        let pointer = timeline.frame_buffer_ptr();
        assert_eq!(timeline.frame_buffer_len(), CAMERA_FRAME_WORDS);
        assert_eq!(timeline.duration_seconds(0).unwrap(), 2.0);
        assert_eq!(timeline.frame_rate(0).unwrap(), 60.0);

        assert!(timeline.evaluate_frame(0, 1.5).unwrap());
        assert_eq!(timeline.frame_buffer_ptr(), pointer);
        assert_eq!(timeline.frame_words[CAMERA_FRAME_ABI], 1.0);
        assert_eq!(timeline.frame_words[CAMERA_FRAME_AVAILABLE], 1.0);
        assert_eq!(timeline.frame_words[CAMERA_FRAME_ACTIVE_CLIPS], 2.0);
        assert!((timeline.frame_words[CAMERA_FRAME_INCOMING_WEIGHT] - 0.5).abs() < 1e-10);
        assert!((timeline.frame_words[CAMERA_FRAME_POSITION_X] - 5.0).abs() < 1e-10);
        assert!((timeline.frame_words[CAMERA_FRAME_VERTICAL_FOV] - 50.0).abs() < 1e-10);

        assert!(timeline.evaluate_frame(0, f64::NAN).is_err());
        assert_eq!(timeline.frame_words, empty_camera_frame());
        assert!(timeline.evaluate_frame(0, 1.5).unwrap());
        assert!(!timeline.evaluate_frame(0, 2.0).unwrap());
        assert_eq!(timeline.frame_buffer_ptr(), pointer);
        assert_eq!(timeline.frame_words, empty_camera_frame());
    }

    #[test]
    fn camera_boundary_accepts_a_validated_optional_effects_contract() {
        let mut timeline = WasmCameraTimeline::from_json_with_effects(
            CAMERA_RESOURCE.as_bytes(),
            CAMERA_EFFECTS.as_bytes(),
        )
        .unwrap();
        assert!(timeline.has_effects_contract());
        assert!(timeline.effects_sampling_supported());
        assert!(!timeline.effects_standby_round_robin_supported());
        assert!(!timeline.camera_uses_custom_look_at(0, "a").unwrap());
        assert!(timeline.camera_uses_custom_look_at(0, "b").unwrap());
        assert!(timeline.camera_has_perlin(0, "b").unwrap());
        assert!(timeline.effect(0, "missing").is_err());

        assert!(timeline.evaluate_effects_frame(0, 1.5, 1.0).unwrap());
        let first = timeline.frame_words;
        assert_eq!(first[CAMERA_FRAME_POSITION_X], 5.0);
        assert_ne!(first[CAMERA_FRAME_ROTATION_X], 0.0);
        assert!(timeline.evaluate_effects_frame(0, 0.5, 1.0).unwrap());
        let after_seek = timeline.frame_words;
        assert!(timeline.evaluate_effects_frame(0, 0.5, 1.0).unwrap());
        assert_eq!(timeline.frame_words, after_seek);
        assert!(timeline.evaluate_effects_frame(0, 0.5, 0.0).unwrap());
        assert_ne!(timeline.frame_words, after_seek);
        assert!(timeline.evaluate_effects_frame(0, 0.5, f64::NAN).is_err());
        assert_eq!(timeline.frame_words, empty_camera_frame());
    }

    #[test]
    fn chorus_camera_uses_an_independent_reused_full_frame_region() {
        let mut timeline = WasmCameraTimeline::from_json_with_effects_and_chorus(
            CAMERA_RESOURCE.as_bytes(),
            CAMERA_EFFECTS.as_bytes(),
            CAMERA_CHORUS.as_bytes(),
        )
        .unwrap();
        assert!(timeline.has_chorus());
        assert_eq!(timeline.chorus_duration_seconds().unwrap(), 10.0);
        assert_eq!(timeline.chorus_frame_rate().unwrap(), 60.0);
        assert_eq!(timeline.chorus_frame_buffer_len(), CAMERA_FRAME_WORDS);

        let score_pointer = timeline.frame_buffer_ptr();
        let chorus_pointer = timeline.chorus_frame_buffer_ptr();
        assert_ne!(score_pointer, chorus_pointer);
        assert!(timeline.evaluate_frame(0, 1.5).unwrap());
        let score_frame = timeline.frame_words;

        assert!(timeline.evaluate_chorus_frame(0.0).unwrap());
        assert_eq!(timeline.frame_words, score_frame);
        assert_eq!(timeline.chorus_frame_buffer_ptr(), chorus_pointer);
        assert_eq!(timeline.chorus_frame_words[CAMERA_FRAME_ABI], 1.0);
        assert_eq!(timeline.chorus_frame_words[CAMERA_FRAME_AVAILABLE], 1.0);
        assert_eq!(timeline.chorus_frame_words[CAMERA_FRAME_ACTIVE_CLIPS], 1.0);
        assert_eq!(
            timeline.chorus_frame_words[CAMERA_FRAME_INCOMING_WEIGHT],
            1.0
        );
        assert_eq!(
            timeline.chorus_frame_words[CAMERA_FRAME_INCOMING_INDEX],
            -1.0
        );
        assert_eq!(timeline.chorus_frame_words[CAMERA_FRAME_POSITION_X], 0.0);
        assert_eq!(timeline.chorus_frame_words[CAMERA_FRAME_POSITION_Y], 1.0);
        assert_eq!(timeline.chorus_frame_words[CAMERA_FRAME_POSITION_Z], -15.0);
        assert!(
            (timeline.chorus_frame_words[CAMERA_FRAME_ROTATION_X] - -0.03489949554204941).abs()
                < 1e-7
        );
        assert_eq!(timeline.chorus_frame_words[CAMERA_FRAME_VERTICAL_FOV], 20.0);

        assert!(timeline.evaluate_chorus_frame(f64::NAN).is_err());
        assert_eq!(timeline.chorus_frame_words, empty_camera_frame());
        assert_eq!(timeline.frame_words, score_frame);
        assert!(timeline.evaluate_chorus_frame(10.0 - 1.0 / 60.0).unwrap());
        assert!(!timeline.evaluate_chorus_frame(10.0).unwrap());
        assert_eq!(timeline.chorus_frame_words, empty_camera_frame());
        assert_eq!(timeline.frame_words, score_frame);
        assert_eq!(timeline.frame_buffer_ptr(), score_pointer);
        assert_eq!(timeline.chorus_frame_buffer_ptr(), chorus_pointer);
    }

    #[test]
    fn camera_sequences_use_separate_reused_bound_field_regions() {
        let mut legacy = WasmCameraTimeline::from_json(CAMERA_RESOURCE.as_bytes()).unwrap();
        assert!(!legacy.has_introduction());
        assert!(!legacy.has_finish());
        assert!(!legacy.evaluate_introduction_frame(0.0).unwrap());
        assert!(!legacy.evaluate_finish_frame(0.0).unwrap());
        assert_eq!(
            legacy.introduction_frame_words,
            empty_camera_introduction_frame()
        );
        assert_eq!(legacy.finish_frame_words, empty_camera_finish_frame());

        let mut timeline =
            WasmCameraTimeline::from_json(CAMERA_INTRODUCTION_RESOURCE.as_bytes()).unwrap();
        assert!(timeline.has_introduction());
        assert_eq!(timeline.introduction_duration_seconds().unwrap(), 4.0);
        assert_eq!(timeline.introduction_frame_rate().unwrap(), 60.0);
        assert!(
            timeline
                .introduction_inherits_orientation_and_lens()
                .unwrap()
        );
        assert_eq!(
            timeline.introduction_frame_buffer_len(),
            CAMERA_INTRODUCTION_FRAME_WORDS
        );

        let pointer = timeline.introduction_frame_buffer_ptr();
        assert!(timeline.evaluate_introduction_frame(1.5).unwrap());
        assert_eq!(timeline.introduction_frame_buffer_ptr(), pointer);
        assert_eq!(
            timeline.introduction_frame_words[CAMERA_INTRODUCTION_FRAME_ABI],
            1.0
        );
        assert_eq!(
            timeline.introduction_frame_words[CAMERA_INTRODUCTION_FRAME_AVAILABLE],
            1.0
        );
        assert_eq!(
            &timeline.introduction_frame_words
                [CAMERA_INTRODUCTION_FRAME_POSITION_X..=CAMERA_INTRODUCTION_FRAME_POSITION_Z],
            &[11.0, 20.125, 30.75]
        );

        assert!(timeline.evaluate_introduction_frame(f64::NAN).is_err());
        assert_eq!(
            timeline.introduction_frame_words,
            empty_camera_introduction_frame()
        );
        assert!(timeline.evaluate_introduction_frame(1.5).unwrap());
        assert!(!timeline.evaluate_introduction_frame(4.0 + 1e-9).unwrap());
        assert_eq!(timeline.introduction_frame_buffer_ptr(), pointer);
        assert_eq!(
            timeline.introduction_frame_words,
            empty_camera_introduction_frame()
        );

        assert!(timeline.has_finish());
        assert_eq!(timeline.finish_duration_seconds().unwrap(), 3.0);
        assert_eq!(timeline.finish_frame_rate().unwrap(), 60.0);
        assert_eq!(
            timeline.finish_frame_buffer_len(),
            CAMERA_FINISH_FRAME_WORDS
        );
        let finish_pointer = timeline.finish_frame_buffer_ptr();
        assert!(timeline.evaluate_finish_frame(1.5).unwrap());
        assert_eq!(timeline.finish_frame_buffer_ptr(), finish_pointer);
        assert_eq!(timeline.finish_frame_words[CAMERA_FINISH_FRAME_ABI], 1.0);
        assert_eq!(
            timeline.finish_frame_words[CAMERA_FINISH_FRAME_AVAILABLE],
            1.0
        );
        assert_eq!(
            timeline.finish_frame_words[CAMERA_FINISH_FRAME_ACTIVE_CLIPS],
            2.0
        );
        assert_eq!(
            timeline.finish_frame_words[CAMERA_FINISH_FRAME_INCOMING_WEIGHT],
            0.5
        );
        assert_eq!(
            timeline.finish_frame_words[CAMERA_FINISH_FRAME_INCOMING_INDEX],
            1.0
        );
        assert_eq!(
            timeline.finish_frame_words[CAMERA_FINISH_FRAME_VERTICAL_FOV],
            55.0
        );
        assert!(timeline.evaluate_finish_frame(f64::NAN).is_err());
        assert_eq!(timeline.finish_frame_words, empty_camera_finish_frame());
        assert!(timeline.evaluate_finish_frame(1.5).unwrap());
        assert!(!timeline.evaluate_finish_frame(3.0).unwrap());
        assert_eq!(timeline.finish_frame_buffer_ptr(), finish_pointer);
        assert_eq!(timeline.finish_frame_words, empty_camera_finish_frame());
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
