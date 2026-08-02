use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

use crate::live::CameraProfile;

const DIRECTOR_WRAP_MODE_NONE: u8 = 2;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraTimelineResource {
    pub schema: String,
    pub curves: BTreeMap<String, CameraCurve>,
    pub profiles: BTreeMap<String, CameraTimelineProfile>,
    #[serde(default)]
    pub sequences: CameraSequences,
}

impl CameraTimelineResource {
    pub fn profile(&self, profile: CameraProfile) -> Option<&CameraTimelineProfile> {
        let name = match profile {
            CameraProfile::Low => "low",
            CameraProfile::Mid => "mid",
            CameraProfile::High => "high",
        };
        self.profiles.get(name)
    }

    pub fn introduction(&self) -> Option<&CameraIntroductionSequence> {
        self.sequences.introduction.as_ref()
    }

    pub fn finish(&self) -> Option<&CameraFinishSequence> {
        self.sequences.finish.as_ref()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraSequences {
    pub introduction: Option<CameraIntroductionSequence>,
    pub finish: Option<CameraFinishSequence>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraIntroductionSequence {
    #[serde(rename = "type")]
    pub sequence_type: CameraIntroductionType,
    pub duration_seconds: f64,
    pub frame_rate: f64,
    pub wrap_mode: u8,
    pub inherits_orientation_and_lens: bool,
    pub animation: CameraPositionAnimation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
pub enum CameraIntroductionType {
    #[serde(rename = "position-animation")]
    PositionAnimation,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraFinishSequence {
    #[serde(rename = "type")]
    pub sequence_type: CameraFinishType,
    pub duration_seconds: f64,
    pub frame_rate: f64,
    pub wrap_mode: u8,
    pub cameras: BTreeMap<String, CameraState>,
    pub clips: Vec<CameraClip>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
pub enum CameraFinishType {
    #[serde(rename = "camera-blend")]
    CameraBlend,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraPositionAnimation {
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub source_duration_seconds: f64,
    pub pre_extrapolation: CameraExtrapolation,
    pub post_extrapolation: CameraExtrapolation,
    pub position_curves: CameraPositionCurves,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CameraPositionCurves {
    pub x: Vec<CameraCubicSegment>,
    pub y: Vec<CameraCubicSegment>,
    pub z: Vec<CameraCubicSegment>,
}

/// Authored float32 `[timeSeconds, a, b, c, d]`, evaluated as
/// `((a * dt + b) * dt + c) * dt + d`.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct CameraCubicSegment(pub f32, pub f32, pub f32, pub f32, pub f32);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CameraExtrapolation {
    Hold,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraIntroductionEvaluation {
    pub position: [f64; 3],
    pub inherits_orientation_and_lens: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraFinishEvaluation {
    pub vertical_fov_degrees: f64,
    pub active_clips: u8,
    pub incoming_weight: f64,
    pub incoming_serialized_index: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraTimelineProfile {
    pub duration_seconds: f64,
    pub frame_rate: f64,
    pub wrap_mode: u8,
    pub base_intro_camera_name: Option<String>,
    pub base_intro_camera: Option<CameraState>,
    pub cameras: BTreeMap<String, CameraState>,
    pub clips: Vec<CameraClip>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraState {
    pub position: [f64; 3],
    pub rotation: [f64; 4],
    pub vertical_fov_degrees: f64,
}

/// Compact clip tuple used by the external timeline resource.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CameraClip(
    pub String,
    pub f64,
    pub f64,
    pub f64,
    pub f64,
    pub String,
    pub String,
    pub u8,
    pub u8,
    pub f64,
    pub f64,
    pub u32,
);

/// `[time, value, inSlope, outSlope, inWeight, outWeight, weightedMode]`.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct CameraCurveKey(pub f64, pub f64, pub f64, pub f64, pub f64, pub f64, pub u8);

pub type CameraCurve = Vec<CameraCurveKey>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraEvaluation {
    pub state: CameraState,
    pub active_clips: u8,
    pub incoming_weight: f64,
    pub incoming_serialized_index: Option<u32>,
}

/// One virtual-camera input selected by Timeline before the brain blends it.
/// Effects are evaluated on these inputs independently, matching Cinemachine's
/// pipeline order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraBlendInput<'a> {
    pub camera_name: Option<&'a str>,
    pub state: CameraState,
}

/// The raw virtual-camera inputs and weight selected for one Timeline sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraBlendEvaluation<'a> {
    pub outgoing: CameraBlendInput<'a>,
    pub incoming: Option<CameraBlendInput<'a>>,
    pub active_clips: u8,
    pub incoming_weight: f64,
    pub incoming_serialized_index: Option<u32>,
}

impl CameraBlendEvaluation<'_> {
    pub fn evaluate_raw(self) -> CameraEvaluation {
        let state = self
            .incoming
            .map(|incoming| {
                CameraState::interpolate(self.outgoing.state, incoming.state, self.incoming_weight)
            })
            .unwrap_or_else(|| self.outgoing.state.canonicalized());
        CameraEvaluation {
            state,
            active_clips: self.active_clips,
            incoming_weight: self.incoming_weight,
            incoming_serialized_index: self.incoming_serialized_index,
        }
    }
}

impl CameraIntroductionSequence {
    /// Samples the bound position track without allocating. Orientation and lens
    /// remain owned by the host because this sequence does not bind either one.
    pub fn evaluate(
        &self,
        time_seconds: f64,
    ) -> Result<Option<CameraIntroductionEvaluation>, CameraEvaluationError> {
        if !time_seconds.is_finite() {
            return Err(CameraEvaluationError::NonFiniteTime);
        }
        ensure_director_wrap_none(self.wrap_mode)?;
        if time_seconds < 0.0 || time_seconds > self.duration_seconds {
            return Ok(None);
        }

        let source_time = self.animation.source_time(time_seconds);
        Ok(Some(CameraIntroductionEvaluation {
            position: [
                evaluate_camera_cubic_segments(&self.animation.position_curves.x, source_time)
                    .ok_or(CameraEvaluationError::MissingIntroductionPositionCurve("x"))?,
                evaluate_camera_cubic_segments(&self.animation.position_curves.y, source_time)
                    .ok_or(CameraEvaluationError::MissingIntroductionPositionCurve("y"))?,
                evaluate_camera_cubic_segments(&self.animation.position_curves.z, source_time)
                    .ok_or(CameraEvaluationError::MissingIntroductionPositionCurve("z"))?,
            ],
            inherits_orientation_and_lens: self.inherits_orientation_and_lens,
        }))
    }
}

impl CameraFinishSequence {
    /// Samples only the lens field bound by the finish sequence. The host pose is
    /// intentionally left untouched.
    pub fn evaluate(
        &self,
        curves: &BTreeMap<String, CameraCurve>,
        time_seconds: f64,
    ) -> Result<Option<CameraFinishEvaluation>, CameraEvaluationError> {
        if !time_seconds.is_finite() {
            return Err(CameraEvaluationError::NonFiniteTime);
        }
        ensure_director_wrap_none(self.wrap_mode)?;
        if time_seconds < 0.0 || time_seconds > self.duration_seconds {
            return Ok(None);
        }

        let active = first_two_active_clips(&self.clips, time_seconds);
        let Some(first) = active[0] else {
            return Ok(None);
        };
        let first_fov = self.camera_fov(first)?;
        let Some(second) = active[1] else {
            return Ok(Some(CameraFinishEvaluation {
                vertical_fov_degrees: first_fov,
                active_clips: 1,
                incoming_weight: first.weight(curves, time_seconds)?,
                incoming_serialized_index: Some(first.serialized_index()),
            }));
        };

        let (outgoing, incoming) = if first.runtime_precedes(second) {
            (first, second)
        } else {
            (second, first)
        };
        let incoming_weight = incoming.weight(curves, time_seconds)?;
        Ok(Some(CameraFinishEvaluation {
            vertical_fov_degrees: f64::from(lerp_f32(
                self.camera_fov(outgoing)?,
                self.camera_fov(incoming)?,
                incoming_weight as f32,
            )),
            active_clips: 2,
            incoming_weight,
            incoming_serialized_index: Some(incoming.serialized_index()),
        }))
    }

    fn camera_fov(&self, clip: &CameraClip) -> Result<f64, CameraEvaluationError> {
        self.cameras
            .get(clip.camera())
            .map(|camera| f64::from(camera.vertical_fov_degrees as f32))
            .ok_or_else(|| CameraEvaluationError::MissingFinishCamera(clip.camera().to_owned()))
    }
}

fn ensure_director_wrap_none(wrap_mode: u8) -> Result<(), CameraEvaluationError> {
    if wrap_mode == DIRECTOR_WRAP_MODE_NONE {
        Ok(())
    } else {
        Err(CameraEvaluationError::UnsupportedDirectorWrapMode(
            wrap_mode,
        ))
    }
}

impl CameraPositionAnimation {
    fn source_time(&self, sequence_time: f64) -> f64 {
        let clip_end = self.start_seconds + self.duration_seconds;
        let local_time = if sequence_time < self.start_seconds {
            match self.pre_extrapolation {
                CameraExtrapolation::Hold => 0.0,
            }
        } else if sequence_time > clip_end {
            match self.post_extrapolation {
                CameraExtrapolation::Hold => self.duration_seconds,
            }
        } else {
            sequence_time - self.start_seconds
        };

        // A non-looping AnimationClip holds its final source sample when the
        // authored Timeline clip is longer than the source clip.
        local_time.clamp(0.0, self.source_duration_seconds.max(0.0))
    }
}

/// Samples compact streamed-curve polynomial segments without allocating.
pub fn evaluate_camera_cubic_segments(
    segments: &[CameraCubicSegment],
    time_seconds: f64,
) -> Option<f64> {
    if !time_seconds.is_finite() {
        return None;
    }
    let time_seconds = time_seconds as f32;
    let first = *segments.first()?;
    if time_seconds <= first.0 {
        return Some(f64::from(first.4));
    }
    let last = *segments.last()?;
    if time_seconds >= last.0 {
        return Some(f64::from(last.4));
    }

    let upper = segments.partition_point(|segment| segment.0 <= time_seconds);
    let segment = segments[upper - 1];
    let delta = time_seconds - segment.0;
    Some(f64::from(
        ((segment.1 * delta + segment.2) * delta + segment.3) * delta + segment.4,
    ))
}

impl CameraTimelineProfile {
    /// Evaluates the first two active Timeline inputs without allocating.
    pub fn evaluate(
        &self,
        curves: &BTreeMap<String, CameraCurve>,
        time_seconds: f64,
    ) -> Result<Option<CameraEvaluation>, CameraEvaluationError> {
        Ok(self
            .evaluate_blend_inputs(curves, time_seconds)?
            .map(CameraBlendEvaluation::evaluate_raw))
    }

    /// Resolves the virtual-camera inputs without blending their pipeline
    /// corrections. Hosts that reproduce Cinemachine extensions use this to
    /// sample each input first and blend raw pose and corrections separately.
    pub fn evaluate_blend_inputs(
        &self,
        curves: &BTreeMap<String, CameraCurve>,
        time_seconds: f64,
    ) -> Result<Option<CameraBlendEvaluation<'_>>, CameraEvaluationError> {
        if !time_seconds.is_finite() {
            return Err(CameraEvaluationError::NonFiniteTime);
        }

        let active = first_two_active_clips(&self.clips, time_seconds);

        let Some(first) = active[0] else {
            return Ok(None);
        };
        let first_state = self.camera(first)?;
        let Some(second) = active[1] else {
            let weight = first.weight(curves, time_seconds)?;
            let (outgoing, incoming) = self
                .base_intro_camera
                .map(|base| {
                    (
                        CameraBlendInput {
                            camera_name: self.base_intro_camera_name.as_deref(),
                            state: base,
                        },
                        Some(CameraBlendInput {
                            camera_name: Some(first.camera()),
                            state: first_state,
                        }),
                    )
                })
                .unwrap_or((
                    CameraBlendInput {
                        camera_name: Some(first.camera()),
                        state: first_state,
                    },
                    None,
                ));
            return Ok(Some(CameraBlendEvaluation {
                outgoing,
                incoming,
                active_clips: 1,
                incoming_weight: weight,
                incoming_serialized_index: Some(first.serialized_index()),
            }));
        };

        let (outgoing, incoming) = if first.runtime_precedes(second) {
            (first, second)
        } else {
            (second, first)
        };
        let incoming_weight = incoming.weight(curves, time_seconds)?;
        Ok(Some(CameraBlendEvaluation {
            outgoing: CameraBlendInput {
                camera_name: Some(outgoing.camera()),
                state: self.camera(outgoing)?,
            },
            incoming: Some(CameraBlendInput {
                camera_name: Some(incoming.camera()),
                state: self.camera(incoming)?,
            }),
            active_clips: 2,
            incoming_weight,
            incoming_serialized_index: Some(incoming.serialized_index()),
        }))
    }

    fn camera(&self, clip: &CameraClip) -> Result<CameraState, CameraEvaluationError> {
        self.cameras
            .get(clip.camera())
            .copied()
            .or_else(|| {
                (self.base_intro_camera_name.as_deref() == Some(clip.camera()))
                    .then_some(self.base_intro_camera)
                    .flatten()
            })
            .ok_or_else(|| CameraEvaluationError::MissingCamera(clip.camera().to_owned()))
    }
}

impl CameraClip {
    pub fn camera(&self) -> &str {
        &self.0
    }

    pub const fn start_seconds(&self) -> f64 {
        self.1
    }

    pub const fn duration_seconds(&self) -> f64 {
        self.2
    }

    pub const fn serialized_index(&self) -> u32 {
        self.11
    }

    fn runtime_precedes(&self, other: &Self) -> bool {
        self.start_seconds()
            .total_cmp(&other.start_seconds())
            .then_with(|| other.duration_seconds().total_cmp(&self.duration_seconds()))
            .then_with(|| self.serialized_index().cmp(&other.serialized_index()))
            .is_le()
    }

    fn is_active(&self, time_seconds: f64) -> bool {
        time_seconds >= self.start_seconds()
            && time_seconds < self.start_seconds() + self.duration_seconds().max(0.0)
    }

    fn weight(
        &self,
        curves: &BTreeMap<String, CameraCurve>,
        time_seconds: f64,
    ) -> Result<f64, CameraEvaluationError> {
        let mix_in = clip_factor(
            curves,
            &self.5,
            self.3,
            time_seconds - self.start_seconds(),
            CurveDefault::MixIn,
        )?;
        let mix_out = clip_factor(
            curves,
            &self.6,
            self.4,
            time_seconds - (self.start_seconds() + self.duration_seconds() - self.4),
            CurveDefault::MixOut,
        )?;
        Ok(f64::from((mix_in * mix_out).clamp(0.0, 1.0)))
    }
}

impl CameraState {
    fn canonicalized(self) -> Self {
        Self {
            position: self.position.map(|component| f64::from(component as f32)),
            rotation: self.rotation.map(|component| f64::from(component as f32)),
            vertical_fov_degrees: f64::from(self.vertical_fov_degrees as f32),
        }
    }

    pub fn interpolate(outgoing: Self, incoming: Self, amount: f64) -> Self {
        // Timeline camera weights and Unity's CameraState.Lerp API are
        // single-precision even though PlayableDirector time is a double.
        let amount = (amount as f32).clamp(0.0, 1.0);
        Self {
            position: [
                f64::from(lerp_f32(outgoing.position[0], incoming.position[0], amount)),
                f64::from(lerp_f32(outgoing.position[1], incoming.position[1], amount)),
                f64::from(lerp_f32(outgoing.position[2], incoming.position[2], amount)),
            ],
            rotation: shortest_arc_slerp_f32(outgoing.rotation, incoming.rotation, amount),
            vertical_fov_degrees: f64::from(lerp_f32(
                outgoing.vertical_fov_degrees,
                incoming.vertical_fov_degrees,
                amount,
            )),
        }
    }
}

/// Piecewise cubic interpolation, including weighted tangent handles.
pub fn evaluate_camera_curve(keys: &[CameraCurveKey], time: f64) -> Option<f64> {
    evaluate_camera_curve_f32(keys, time as f32).map(f64::from)
}

fn evaluate_camera_curve_f32(keys: &[CameraCurveKey], time: f32) -> Option<f32> {
    if !time.is_finite() {
        return None;
    }
    let first = *keys.first()?;
    if time <= first.0 as f32 {
        return Some(first.1 as f32);
    }
    let last = *keys.last()?;
    if time >= last.0 as f32 {
        return Some(last.1 as f32);
    }
    let upper = keys.partition_point(|key| key.0 as f32 <= time);
    let left = keys[upper - 1];
    let right = keys[upper];
    let duration = right.0 as f32 - left.0 as f32;
    if duration <= 0.0 {
        return Some(right.1 as f32);
    }
    let normalized = ((time - left.0 as f32) / duration).clamp(0.0, 1.0);
    let out_weight = if left.6 & 2 != 0 {
        (left.5 as f32).clamp(0.0, 1.0)
    } else {
        1.0_f32 / 3.0
    };
    let in_weight = if right.6 & 1 != 0 {
        (right.4 as f32).clamp(0.0, 1.0)
    } else {
        1.0_f32 / 3.0
    };
    let parameter = solve_bezier_x_f32(normalized, out_weight, 1.0 - in_weight);
    let first_control = left.1 as f32 + left.3 as f32 * duration * out_weight;
    let second_control = right.1 as f32 - right.2 as f32 * duration * in_weight;
    Some(cubic_bezier_f32(
        left.1 as f32,
        first_control,
        second_control,
        right.1 as f32,
        parameter,
    ))
}

fn insert_by_serialized_index<'a>(active: &mut [Option<&'a CameraClip>; 2], clip: &'a CameraClip) {
    if active[0].is_none_or(|current| clip.serialized_index() < current.serialized_index()) {
        active[1] = active[0];
        active[0] = Some(clip);
    } else if active[1].is_none_or(|current| clip.serialized_index() < current.serialized_index()) {
        active[1] = Some(clip);
    }
}

fn first_two_active_clips(clips: &[CameraClip], time_seconds: f64) -> [Option<&CameraClip>; 2] {
    let mut active = [None, None];
    for clip in clips {
        if clip.is_active(time_seconds) {
            insert_by_serialized_index(&mut active, clip);
        }
    }
    active
}

#[derive(Clone, Copy)]
enum CurveDefault {
    MixIn,
    MixOut,
}

fn clip_factor(
    curves: &BTreeMap<String, CameraCurve>,
    curve_name: &str,
    duration: f64,
    elapsed: f64,
    default: CurveDefault,
) -> Result<f32, CameraEvaluationError> {
    if duration < 0.0 {
        return Ok(1.0);
    }
    if duration == 0.0 {
        return Ok(1.0);
    }
    // TimelineClip performs its local-time division in double precision and
    // narrows only the normalized sample passed to AnimationCurve.Evaluate.
    let normalized = ((elapsed / duration) as f32).clamp(0.0, 1.0);
    let curve = curves
        .get(curve_name)
        .ok_or_else(|| CameraEvaluationError::MissingCurve(curve_name.to_owned()))?;
    let value = evaluate_camera_curve_f32(curve, normalized).unwrap_or_else(|| match default {
        CurveDefault::MixIn => smoothstep_f32(normalized),
        CurveDefault::MixOut => 1.0 - smoothstep_f32(normalized),
    });
    Ok(value.clamp(0.0, 1.0))
}

fn solve_bezier_x_f32(target: f32, first_control: f32, second_control: f32) -> f32 {
    let mut lower = 0.0_f32;
    let mut upper = 1.0_f32;
    for _ in 0..40 {
        let midpoint = (lower + upper) * 0.5;
        let value = cubic_bezier_f32(0.0, first_control, second_control, 1.0, midpoint);
        if value < target {
            lower = midpoint;
        } else {
            upper = midpoint;
        }
    }
    (lower + upper) * 0.5
}

fn cubic_bezier_f32(a: f32, b: f32, c: f32, d: f32, time: f32) -> f32 {
    let inverse = 1.0 - time;
    inverse * inverse * inverse * a
        + 3.0 * inverse * inverse * time * b
        + 3.0 * inverse * time * time * c
        + time * time * time * d
}

pub(crate) fn shortest_arc_slerp_f32(a: [f64; 4], b: [f64; 4], amount: f32) -> [f64; 4] {
    let mut a = a.map(|component| component as f32);
    let mut b = b.map(|component| component as f32);
    normalize_quaternion_f32(&mut a);
    normalize_quaternion_f32(&mut b);
    let mut dot = quaternion_dot_f32(a, b);
    if dot < 0.0 {
        b = [-b[0], -b[1], -b[2], -b[3]];
        dot = -dot;
    }
    if dot > 0.9995 {
        let mut result = [
            lerp_f32_values(a[0], b[0], amount),
            lerp_f32_values(a[1], b[1], amount),
            lerp_f32_values(a[2], b[2], amount),
            lerp_f32_values(a[3], b[3], amount),
        ];
        normalize_quaternion_f32(&mut result);
        return result.map(f64::from);
    }
    let theta = dot.clamp(-1.0, 1.0).acos();
    let sin_theta = theta.sin();
    let outgoing = ((1.0 - amount) * theta).sin() / sin_theta;
    let incoming = (amount * theta).sin() / sin_theta;
    [
        outgoing * a[0] + incoming * b[0],
        outgoing * a[1] + incoming * b[1],
        outgoing * a[2] + incoming * b[2],
        outgoing * a[3] + incoming * b[3],
    ]
    .map(f64::from)
}

pub(crate) fn unity_euler_zxy_quaternion_f32(euler_degrees: [f32; 3]) -> [f32; 4] {
    let [x, y, z] = euler_degrees.map(|value| value.to_radians() * 0.5);
    let (sin_x, cos_x) = x.sin_cos();
    let (sin_y, cos_y) = y.sin_cos();
    let (sin_z, cos_z) = z.sin_cos();
    let qx = [sin_x, 0.0, 0.0, cos_x];
    let qy = [0.0, sin_y, 0.0, cos_y];
    let qz = [0.0, 0.0, sin_z, cos_z];
    let mut result = quaternion_multiply_f32(quaternion_multiply_f32(qy, qx), qz);
    normalize_quaternion_f32(&mut result);
    result
}

pub(crate) const fn quaternion_multiply_f32(left: [f32; 4], right: [f32; 4]) -> [f32; 4] {
    [
        left[3] * right[0] + left[0] * right[3] + left[1] * right[2] - left[2] * right[1],
        left[3] * right[1] - left[0] * right[2] + left[1] * right[3] + left[2] * right[0],
        left[3] * right[2] + left[0] * right[1] - left[1] * right[0] + left[2] * right[3],
        left[3] * right[3] - left[0] * right[0] - left[1] * right[1] - left[2] * right[2],
    ]
}

fn normalize_quaternion_f32(value: &mut [f32; 4]) {
    let magnitude = quaternion_dot_f32(*value, *value).sqrt();
    if magnitude > f32::EPSILON {
        for component in value {
            *component /= magnitude;
        }
    } else {
        *value = [0.0, 0.0, 0.0, 1.0];
    }
}

const fn quaternion_dot_f32(a: [f32; 4], b: [f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

const fn smoothstep_f32(time: f32) -> f32 {
    time * time * (3.0 - 2.0 * time)
}

fn lerp_f32(a: f64, b: f64, amount: f32) -> f32 {
    lerp_f32_values(a as f32, b as f32, amount)
}

fn lerp_f32_values(a: f32, b: f32, amount: f32) -> f32 {
    a + (b - a) * amount
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CameraEvaluationError {
    NonFiniteTime,
    MissingCamera(String),
    MissingCurve(String),
    MissingIntroductionPositionCurve(&'static str),
    MissingFinishCamera(String),
    UnsupportedDirectorWrapMode(u8),
}

impl fmt::Display for CameraEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteTime => formatter.write_str("camera time must be finite"),
            Self::MissingCamera(name) => write!(formatter, "missing camera: {name}"),
            Self::MissingCurve(name) => write!(formatter, "missing camera curve: {name}"),
            Self::MissingIntroductionPositionCurve(axis) => {
                write!(formatter, "missing introduction position curve: {axis}")
            }
            Self::MissingFinishCamera(name) => write!(formatter, "missing finish camera: {name}"),
            Self::UnsupportedDirectorWrapMode(mode) => {
                write!(formatter, "unsupported camera director wrap mode: {mode}")
            }
        }
    }
}

impl std::error::Error for CameraEvaluationError {}

#[cfg(test)]
mod tests {
    use super::*;

    const RESOURCE: &str = r#"{
      "schema":"caph-live-camera-timelines-v1",
      "curves":{
        "linear":[[0,0,1,1,0,0,0],[1,1,1,1,0,0,0]],
        "empty":[]
      },
      "profiles":{"low":{
        "durationSeconds":2,
        "frameRate":60,
        "wrapMode":2,
        "baseIntroCameraName":"base",
        "baseIntroCamera":{"position":[0,0,0],"rotation":[0,0,0,1],"verticalFovDegrees":40},
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

    const INTRODUCTION_RESOURCE: &str = r#"{
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

    #[test]
    fn compact_resource_deserializes_and_blends_two_inputs() {
        let resource: CameraTimelineResource = serde_json::from_str(RESOURCE).unwrap();
        let profile = &resource.profiles["low"];
        assert!(std::ptr::eq(
            profile,
            resource.profile(CameraProfile::Low).unwrap()
        ));
        let evaluated = profile.evaluate(&resource.curves, 1.5).unwrap().unwrap();
        assert_eq!(evaluated.active_clips, 2);
        assert!((evaluated.incoming_weight - 0.5).abs() < 1e-10);
        for (actual, expected) in evaluated.state.position.into_iter().zip([5.0, 10.0, 15.0]) {
            assert!((actual - expected).abs() < 1e-10);
        }
        assert!((evaluated.state.vertical_fov_degrees - 50.0).abs() < 1e-10);
        assert!(
            (evaluated
                .state
                .rotation
                .into_iter()
                .map(|component| component * component)
                .sum::<f64>()
                - 1.0)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn hermite_curve_uses_tangents_and_weighted_handles() {
        let hermite = [
            CameraCurveKey(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
            CameraCurveKey(1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0),
        ];
        assert!((evaluate_camera_curve(&hermite, 0.5).unwrap() - 0.5).abs() < 1e-10);

        let weighted = [
            CameraCurveKey(0.0, 0.0, 0.0, 1.0, 0.0, 0.8, 2),
            CameraCurveKey(1.0, 1.0, 1.0, 0.0, 0.1, 0.0, 1),
        ];
        let value = evaluate_camera_curve(&weighted, 0.4).unwrap();
        assert!(value.is_finite());
        assert!((0.0..=1.0).contains(&value));
    }

    #[test]
    fn equal_quaternion_signs_follow_the_shortest_arc() {
        let state = CameraState::interpolate(
            CameraState {
                position: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                vertical_fov_degrees: 40.0,
            },
            CameraState {
                position: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, -1.0],
                vertical_fov_degrees: 40.0,
            },
            0.5,
        );
        assert_eq!(state.rotation, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn first_two_active_inputs_follow_serialized_order() {
        let mut resource: CameraTimelineResource = serde_json::from_str(RESOURCE).unwrap();
        let profile = resource.profiles.get_mut("low").unwrap();
        profile.cameras.insert(
            "ignored".into(),
            CameraState {
                position: [99.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                vertical_fov_degrees: 99.0,
            },
        );
        profile.clips.insert(
            0,
            CameraClip(
                "ignored".into(),
                0.0,
                2.0,
                -1.0,
                -1.0,
                "linear".into(),
                "linear".into(),
                0,
                0,
                0.0,
                1.0,
                100,
            ),
        );
        let evaluated = profile.evaluate(&resource.curves, 1.5).unwrap().unwrap();
        assert_ne!(evaluated.state.position, [99.0; 3]);
    }

    #[test]
    fn introduction_holds_before_and_after_its_non_looping_source() {
        let resource: CameraTimelineResource = serde_json::from_str(INTRODUCTION_RESOURCE).unwrap();
        let introduction = resource.introduction().unwrap();

        for time in [0.0, 1.0] {
            let evaluated = introduction.evaluate(time).unwrap().unwrap();
            assert_eq!(evaluated.position, [10.0, 20.0, 30.0]);
            assert!(evaluated.inherits_orientation_and_lens);
        }

        let animated = introduction.evaluate(1.5).unwrap().unwrap();
        assert_eq!(animated.position, [11.0, 20.125, 30.75]);

        for time in [2.0, 3.5, 4.0] {
            assert_eq!(
                introduction.evaluate(time).unwrap().unwrap().position,
                [12.0, 21.0, 33.0]
            );
        }
        assert!(introduction.evaluate(-f64::EPSILON).unwrap().is_none());
        assert!(introduction.evaluate(4.0 + 1e-9).unwrap().is_none());
        assert_eq!(
            introduction.evaluate(f64::NAN),
            Err(CameraEvaluationError::NonFiniteTime)
        );
        let mut unsupported = introduction.clone();
        unsupported.wrap_mode = 0;
        assert_eq!(
            unsupported.evaluate(0.0),
            Err(CameraEvaluationError::UnsupportedDirectorWrapMode(0))
        );
    }

    #[test]
    fn introduction_segment_selection_uses_the_latest_boundary() {
        let segments = [
            CameraCubicSegment(0.0, 0.0, 0.0, 2.0, 10.0),
            CameraCubicSegment(1.0, 0.0, 0.0, 0.0, 12.0),
        ];
        assert_eq!(evaluate_camera_cubic_segments(&segments, -1.0), Some(10.0));
        assert_eq!(evaluate_camera_cubic_segments(&segments, 0.5), Some(11.0));
        assert_eq!(evaluate_camera_cubic_segments(&segments, 1.0), Some(12.0));
        assert_eq!(evaluate_camera_cubic_segments(&segments, 2.0), Some(12.0));
        assert_eq!(evaluate_camera_cubic_segments(&segments, f64::NAN), None);
        assert_eq!(evaluate_camera_cubic_segments(&[], 0.0), None);
    }

    #[test]
    fn finish_sequence_only_samples_its_bound_fov_blend() {
        let resource: CameraTimelineResource = serde_json::from_str(INTRODUCTION_RESOURCE).unwrap();
        let finish = resource.finish().unwrap();

        let first = finish.evaluate(&resource.curves, 0.0).unwrap().unwrap();
        assert_eq!(first.vertical_fov_degrees, 40.0);
        assert_eq!(first.active_clips, 2);
        assert_eq!(first.incoming_weight, 0.0);
        assert_eq!(first.incoming_serialized_index, Some(1));

        let middle = finish.evaluate(&resource.curves, 1.5).unwrap().unwrap();
        assert_eq!(middle.vertical_fov_degrees, 55.0);
        assert_eq!(middle.incoming_weight, 0.5);
        assert!(finish.evaluate(&resource.curves, 3.0).unwrap().is_none());
        assert_eq!(
            finish.evaluate(&resource.curves, f64::NAN),
            Err(CameraEvaluationError::NonFiniteTime)
        );

        let mut unsupported = finish.clone();
        unsupported.wrap_mode = 1;
        assert_eq!(
            unsupported.evaluate(&resource.curves, 0.0),
            Err(CameraEvaluationError::UnsupportedDirectorWrapMode(1))
        );
    }

    #[test]
    fn shorter_clip_is_incoming_when_active_clips_start_together() {
        let mut resource: CameraTimelineResource = serde_json::from_str(RESOURCE).unwrap();
        let profile = resource.profiles.get_mut("low").unwrap();
        profile.cameras.insert(
            "long".into(),
            CameraState {
                position: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                vertical_fov_degrees: 10.0,
            },
        );
        profile.cameras.insert(
            "short".into(),
            CameraState {
                position: [10.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                vertical_fov_degrees: 90.0,
            },
        );
        profile.clips = vec![
            CameraClip(
                "short".into(),
                0.0,
                1.0,
                1.0,
                -1.0,
                "linear".into(),
                "linear".into(),
                0,
                0,
                0.0,
                1.0,
                1,
            ),
            CameraClip(
                "long".into(),
                0.0,
                2.0,
                -1.0,
                -1.0,
                "linear".into(),
                "linear".into(),
                0,
                0,
                0.0,
                1.0,
                9,
            ),
        ];

        let evaluated = profile.evaluate(&resource.curves, 0.5).unwrap().unwrap();
        assert_eq!(evaluated.incoming_serialized_index, Some(1));
        assert!((evaluated.incoming_weight - 0.5).abs() < 1e-10);
        assert!((evaluated.state.vertical_fov_degrees - 50.0).abs() < 1e-10);
    }
}
