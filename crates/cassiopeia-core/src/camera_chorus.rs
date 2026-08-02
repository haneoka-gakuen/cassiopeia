use serde::Deserialize;
use std::fmt;

use crate::{CameraCubicSegment, CameraState, evaluate_camera_cubic_segments};

pub const CAMERA_CHORUS_SCHEMA: &str = "org.haneoka.caph.live-chorus-camera";
pub const CAMERA_CHORUS_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraChorusResource {
    pub schema: String,
    pub schema_version: u32,
    pub duration_seconds: f64,
    pub frame_rate: f64,
    pub playback: CameraChorusPlayback,
    pub completion: CameraChorusCompletion,
    pub coefficient_tuple: [String; 5],
    pub channels: CameraChorusChannels,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CameraChorusPlayback {
    Once,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
pub enum CameraChorusCompletion {
    #[serde(rename = "restore-score-timeline")]
    RestoreScoreTimeline,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraChorusChannels {
    pub position: CameraChorusVectorCurves,
    pub euler_degrees: CameraChorusVectorCurves,
    pub vertical_fov_degrees: Vec<CameraCubicSegment>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CameraChorusVectorCurves {
    pub x: Vec<CameraCubicSegment>,
    pub y: Vec<CameraCubicSegment>,
    pub z: Vec<CameraCubicSegment>,
}

impl CameraChorusResource {
    pub fn validate(&self) -> Result<(), CameraChorusError> {
        if self.schema != CAMERA_CHORUS_SCHEMA {
            return Err(CameraChorusError::UnsupportedSchema(self.schema.clone()));
        }
        if self.schema_version != CAMERA_CHORUS_SCHEMA_VERSION {
            return Err(CameraChorusError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        if self.coefficient_tuple != ["timeSeconds", "a", "b", "c", "d"] {
            return Err(CameraChorusError::InvalidCoefficientTuple);
        }
        if !self.duration_seconds.is_finite() || self.duration_seconds <= 0.0 {
            return Err(CameraChorusError::InvalidDuration);
        }
        if !self.frame_rate.is_finite() || self.frame_rate <= 0.0 {
            return Err(CameraChorusError::InvalidFrameRate);
        }
        for (name, segments) in self.curves() {
            if segments.is_empty() {
                return Err(CameraChorusError::MissingCurve(name));
            }
            let mut previous = f32::NEG_INFINITY;
            for segment in segments {
                let values = [segment.0, segment.1, segment.2, segment.3, segment.4];
                if !values.iter().all(|value| value.is_finite()) {
                    return Err(CameraChorusError::NonFiniteCurve(name));
                }
                if segment.0 < previous {
                    return Err(CameraChorusError::UnsortedCurve(name));
                }
                previous = segment.0;
            }
        }
        Ok(())
    }

    /// Samples the full one-shot camera. The upper bound is exclusive because
    /// completion restores the score Timeline in the same update.
    pub fn evaluate(&self, time_seconds: f64) -> Result<Option<CameraState>, CameraChorusError> {
        if !time_seconds.is_finite() {
            return Err(CameraChorusError::NonFiniteTime);
        }
        if time_seconds < 0.0 || time_seconds >= self.duration_seconds {
            return Ok(None);
        }
        let position = [
            self.sample("position.x", &self.channels.position.x, time_seconds)?,
            self.sample("position.y", &self.channels.position.y, time_seconds)?,
            self.sample("position.z", &self.channels.position.z, time_seconds)?,
        ];
        let euler_degrees = [
            self.sample(
                "eulerDegrees.x",
                &self.channels.euler_degrees.x,
                time_seconds,
            )?,
            self.sample(
                "eulerDegrees.y",
                &self.channels.euler_degrees.y,
                time_seconds,
            )?,
            self.sample(
                "eulerDegrees.z",
                &self.channels.euler_degrees.z,
                time_seconds,
            )?,
        ];
        Ok(Some(CameraState {
            position,
            rotation: unity_euler_zxy_quaternion(euler_degrees),
            vertical_fov_degrees: self.sample(
                "verticalFovDegrees",
                &self.channels.vertical_fov_degrees,
                time_seconds,
            )?,
        }))
    }

    fn sample(
        &self,
        name: &'static str,
        segments: &[CameraCubicSegment],
        time_seconds: f64,
    ) -> Result<f64, CameraChorusError> {
        evaluate_camera_cubic_segments(segments, time_seconds)
            .ok_or(CameraChorusError::MissingCurve(name))
    }

    fn curves(&self) -> [(&'static str, &[CameraCubicSegment]); 7] {
        [
            ("position.x", &self.channels.position.x),
            ("position.y", &self.channels.position.y),
            ("position.z", &self.channels.position.z),
            ("eulerDegrees.x", &self.channels.euler_degrees.x),
            ("eulerDegrees.y", &self.channels.euler_degrees.y),
            ("eulerDegrees.z", &self.channels.euler_degrees.z),
            ("verticalFovDegrees", &self.channels.vertical_fov_degrees),
        ]
    }
}

/// Unity applies Euler rotations around Z, then X, then Y.
fn unity_euler_zxy_quaternion(euler_degrees: [f64; 3]) -> [f64; 4] {
    let [x, y, z] = euler_degrees.map(|value| (value as f32).to_radians() * 0.5);
    let (sin_x, cos_x) = x.sin_cos();
    let (sin_y, cos_y) = y.sin_cos();
    let (sin_z, cos_z) = z.sin_cos();
    let qx = [sin_x, 0.0, 0.0, cos_x];
    let qy = [0.0, sin_y, 0.0, cos_y];
    let qz = [0.0, 0.0, sin_z, cos_z];
    normalize(multiply(multiply(qy, qx), qz)).map(f64::from)
}

const fn multiply(left: [f32; 4], right: [f32; 4]) -> [f32; 4] {
    [
        left[3] * right[0] + left[0] * right[3] + left[1] * right[2] - left[2] * right[1],
        left[3] * right[1] - left[0] * right[2] + left[1] * right[3] + left[2] * right[0],
        left[3] * right[2] + left[0] * right[1] - left[1] * right[0] + left[2] * right[3],
        left[3] * right[3] - left[0] * right[0] - left[1] * right[1] - left[2] * right[2],
    ]
}

fn normalize(mut value: [f32; 4]) -> [f32; 4] {
    let magnitude = value
        .iter()
        .map(|component| component * component)
        .sum::<f32>()
        .sqrt();
    if magnitude <= f32::EPSILON {
        return [0.0, 0.0, 0.0, 1.0];
    }
    for component in &mut value {
        *component /= magnitude;
    }
    value
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CameraChorusError {
    UnsupportedSchema(String),
    UnsupportedSchemaVersion(u32),
    InvalidCoefficientTuple,
    InvalidDuration,
    InvalidFrameRate,
    MissingCurve(&'static str),
    NonFiniteCurve(&'static str),
    UnsortedCurve(&'static str),
    NonFiniteTime,
}

impl fmt::Display for CameraChorusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema(schema) => {
                write!(formatter, "unsupported chorus-camera schema: {schema}")
            }
            Self::UnsupportedSchemaVersion(version) => {
                write!(
                    formatter,
                    "unsupported chorus-camera schema version: {version}"
                )
            }
            Self::InvalidCoefficientTuple => {
                formatter.write_str("invalid chorus-camera coefficient tuple")
            }
            Self::InvalidDuration => formatter.write_str("invalid chorus-camera duration"),
            Self::InvalidFrameRate => formatter.write_str("invalid chorus-camera frame rate"),
            Self::MissingCurve(name) => write!(formatter, "missing chorus-camera curve: {name}"),
            Self::NonFiniteCurve(name) => {
                write!(formatter, "non-finite chorus-camera curve: {name}")
            }
            Self::UnsortedCurve(name) => write!(formatter, "unsorted chorus-camera curve: {name}"),
            Self::NonFiniteTime => formatter.write_str("chorus-camera time must be finite"),
        }
    }
}

impl std::error::Error for CameraChorusError {}

#[cfg(test)]
mod tests {
    use super::*;

    const RESOURCE: &str = r#"{
      "schema":"org.haneoka.caph.live-chorus-camera","schemaVersion":1,
      "durationSeconds":10,"frameRate":60,"playback":"once",
      "completion":"restore-score-timeline","coefficientTuple":["timeSeconds","a","b","c","d"],
      "channels":{
        "position":{"x":[[0,0,0,0,0]],"y":[[0,0,0,0,1]],"z":[[0,0,0,0,-15]]},
        "eulerDegrees":{"x":[[0,0,0,0,-4]],"y":[[0,0,0,0,0]],"z":[[0,0,0,0,0]]},
        "verticalFovDegrees":[[0,0,0,0,20]]
      }
    }"#;

    #[test]
    fn evaluates_an_exclusive_one_shot_full_camera() {
        let resource: CameraChorusResource = serde_json::from_str(RESOURCE).unwrap();
        resource.validate().unwrap();
        assert!(resource.evaluate(-f64::EPSILON).unwrap().is_none());
        let frame = resource.evaluate(0.0).unwrap().unwrap();
        assert_eq!(frame.position, [0.0, 1.0, -15.0]);
        assert!((frame.rotation[0] - -0.03489949554204941).abs() < 1e-7);
        assert_eq!(frame.rotation[1], 0.0);
        assert_eq!(frame.rotation[2], 0.0);
        assert!((frame.rotation[3] - 0.9993908405303955).abs() < 1e-7);
        assert_eq!(frame.vertical_fov_degrees, 20.0);
        assert!(resource.evaluate(10.0 - 1.0 / 60.0).unwrap().is_some());
        assert!(resource.evaluate(10.0).unwrap().is_none());
        assert_eq!(
            resource.evaluate(f64::NAN),
            Err(CameraChorusError::NonFiniteTime)
        );
    }
}
