use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

use crate::live::CameraProfile;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraTimelineResource {
    pub schema: String,
    pub curves: BTreeMap<String, CameraCurve>,
    pub profiles: BTreeMap<String, CameraTimelineProfile>,
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

impl CameraTimelineProfile {
    /// Evaluates the first two active Timeline inputs without allocating.
    pub fn evaluate(
        &self,
        curves: &BTreeMap<String, CameraCurve>,
        time_seconds: f64,
    ) -> Result<Option<CameraEvaluation>, CameraEvaluationError> {
        if !time_seconds.is_finite() {
            return Err(CameraEvaluationError::NonFiniteTime);
        }

        let mut active: [Option<&CameraClip>; 2] = [None, None];
        for clip in &self.clips {
            if !clip.is_active(time_seconds) {
                continue;
            }
            insert_by_serialized_index(&mut active, clip);
        }

        let Some(first) = active[0] else {
            return Ok(None);
        };
        let first_state = self.camera(first)?;
        let Some(second) = active[1] else {
            let weight = first.weight(curves, time_seconds)?;
            let state = self
                .base_intro_camera
                .map(|base| CameraState::interpolate(base, first_state, weight))
                .unwrap_or(first_state);
            return Ok(Some(CameraEvaluation {
                state,
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
        let outgoing_state = self.camera(outgoing)?;
        let incoming_state = self.camera(incoming)?;
        let incoming_weight = incoming.weight(curves, time_seconds)?;
        Ok(Some(CameraEvaluation {
            state: CameraState::interpolate(outgoing_state, incoming_state, incoming_weight),
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
        Ok((mix_in * mix_out).clamp(0.0, 1.0))
    }
}

impl CameraState {
    pub fn interpolate(outgoing: Self, incoming: Self, amount: f64) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        Self {
            position: [
                lerp(outgoing.position[0], incoming.position[0], amount),
                lerp(outgoing.position[1], incoming.position[1], amount),
                lerp(outgoing.position[2], incoming.position[2], amount),
            ],
            rotation: shortest_arc_slerp(outgoing.rotation, incoming.rotation, amount),
            vertical_fov_degrees: lerp(
                outgoing.vertical_fov_degrees,
                incoming.vertical_fov_degrees,
                amount,
            ),
        }
    }
}

/// Piecewise cubic interpolation, including weighted tangent handles.
pub fn evaluate_camera_curve(keys: &[CameraCurveKey], time: f64) -> Option<f64> {
    let first = *keys.first()?;
    if time <= first.0 {
        return Some(first.1);
    }
    let last = *keys.last()?;
    if time >= last.0 {
        return Some(last.1);
    }
    let upper = keys.partition_point(|key| key.0 <= time);
    let left = keys[upper - 1];
    let right = keys[upper];
    let duration = right.0 - left.0;
    if duration <= 0.0 {
        return Some(right.1);
    }
    let normalized = ((time - left.0) / duration).clamp(0.0, 1.0);
    let out_weight = if left.6 & 2 != 0 {
        left.5.clamp(0.0, 1.0)
    } else {
        1.0 / 3.0
    };
    let in_weight = if right.6 & 1 != 0 {
        right.4.clamp(0.0, 1.0)
    } else {
        1.0 / 3.0
    };
    let parameter = solve_bezier_x(normalized, out_weight, 1.0 - in_weight);
    let first_control = left.1 + left.3 * duration * out_weight;
    let second_control = right.1 - right.2 * duration * in_weight;
    Some(cubic_bezier(
        left.1,
        first_control,
        second_control,
        right.1,
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
) -> Result<f64, CameraEvaluationError> {
    if duration < 0.0 {
        return Ok(1.0);
    }
    if duration == 0.0 {
        return Ok(1.0);
    }
    let normalized = (elapsed / duration).clamp(0.0, 1.0);
    let curve = curves
        .get(curve_name)
        .ok_or_else(|| CameraEvaluationError::MissingCurve(curve_name.to_owned()))?;
    let value = evaluate_camera_curve(curve, normalized).unwrap_or_else(|| match default {
        CurveDefault::MixIn => smoothstep(normalized),
        CurveDefault::MixOut => 1.0 - smoothstep(normalized),
    });
    Ok(value.clamp(0.0, 1.0))
}

fn solve_bezier_x(target: f64, first_control: f64, second_control: f64) -> f64 {
    let mut lower = 0.0;
    let mut upper = 1.0;
    for _ in 0..40 {
        let midpoint = (lower + upper) * 0.5;
        let value = cubic_bezier(0.0, first_control, second_control, 1.0, midpoint);
        if value < target {
            lower = midpoint;
        } else {
            upper = midpoint;
        }
    }
    (lower + upper) * 0.5
}

fn cubic_bezier(a: f64, b: f64, c: f64, d: f64, time: f64) -> f64 {
    let inverse = 1.0 - time;
    inverse * inverse * inverse * a
        + 3.0 * inverse * inverse * time * b
        + 3.0 * inverse * time * time * c
        + time * time * time * d
}

fn shortest_arc_slerp(mut a: [f64; 4], mut b: [f64; 4], amount: f64) -> [f64; 4] {
    normalize_quaternion(&mut a);
    normalize_quaternion(&mut b);
    let mut dot = quaternion_dot(a, b);
    if dot < 0.0 {
        b = [-b[0], -b[1], -b[2], -b[3]];
        dot = -dot;
    }
    if dot > 0.9995 {
        let mut result = [
            lerp(a[0], b[0], amount),
            lerp(a[1], b[1], amount),
            lerp(a[2], b[2], amount),
            lerp(a[3], b[3], amount),
        ];
        normalize_quaternion(&mut result);
        return result;
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
}

fn normalize_quaternion(value: &mut [f64; 4]) {
    let magnitude = quaternion_dot(*value, *value).sqrt();
    if magnitude > f64::EPSILON {
        for component in value {
            *component /= magnitude;
        }
    } else {
        *value = [0.0, 0.0, 0.0, 1.0];
    }
}

const fn quaternion_dot(a: [f64; 4], b: [f64; 4]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

const fn smoothstep(time: f64) -> f64 {
    time * time * (3.0 - 2.0 * time)
}

fn lerp(a: f64, b: f64, amount: f64) -> f64 {
    a + (b - a) * amount
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CameraEvaluationError {
    NonFiniteTime,
    MissingCamera(String),
    MissingCurve(String),
}

impl fmt::Display for CameraEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteTime => formatter.write_str("camera time must be finite"),
            Self::MissingCamera(name) => write!(formatter, "missing camera: {name}"),
            Self::MissingCurve(name) => write!(formatter, "missing camera curve: {name}"),
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
            (quaternion_dot(evaluated.state.rotation, evaluated.state.rotation) - 1.0).abs()
                < 1e-10
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
}
