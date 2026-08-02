use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{
    camera::{
        CameraBlendEvaluation, CameraBlendInput, CameraEvaluation, CameraEvaluationError,
        CameraState, CameraTimelineResource, quaternion_multiply_f32, shortest_arc_slerp_f32,
        unity_euler_zxy_quaternion_f32,
    },
    live::CameraProfile,
};

pub const CAMERA_EFFECTS_SCHEMA: &str = "org.haneoka.caph.live-camera-effects";
pub const CAMERA_EFFECTS_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraEffectsResource {
    pub schema: String,
    pub schema_version: u32,
    pub channel_tuple: [String; 3],
    pub octave_tuple: [String; 3],
    pub sampling: CameraEffectsSampling,
    pub noise_profiles: BTreeMap<String, CameraNoiseProfile>,
    pub profiles: BTreeMap<String, CameraEffectsProfile>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraEffectsSampling {
    pub cinemachine: String,
    pub time_base: String,
    pub noise_function: String,
    pub implementation_status: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraNoiseProfile {
    pub position_octaves: Vec<CameraNoiseOctave>,
    pub orientation_octaves: Vec<CameraNoiseOctave>,
}

/// `[x, y, z]`, with each channel stored as `[amplitude, frequency, constant]`.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct CameraNoiseOctave(
    pub CameraNoiseChannel,
    pub CameraNoiseChannel,
    pub CameraNoiseChannel,
);

/// `[amplitude, frequency, constant]`.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct CameraNoiseChannel(pub f64, pub f64, pub bool);

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraEffectsProfile {
    pub camera_names: Vec<String>,
    pub custom_look_at_target: Vec<String>,
    pub perlin: Option<CameraPerlinProfile>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraPerlinProfile {
    pub enabled: bool,
    pub noise_profile: String,
    pub pivot_offset: [f64; 3],
    pub noise_offsets: [f64; 3],
    /// `[amplitudeGain, frequencyGain]`.
    pub default_gain: [f64; 2],
    /// `[amplitudeGain, frequencyGain]` by camera name.
    pub gain_overrides: BTreeMap<String, [f64; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraEffect<'a> {
    pub custom_look_at_target: bool,
    pub perlin: Option<CameraPerlinEffect<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPerlinEffect<'a> {
    pub enabled: bool,
    pub amplitude_gain: f64,
    pub frequency_gain: f64,
    pub pivot_offset: [f64; 3],
    pub noise_offsets: [f64; 3],
    pub noise_profile: &'a CameraNoiseProfile,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPerlinSample {
    pub noise_time_seconds: f64,
    pub orientation_degrees: [f64; 3],
    pub orientation_correction: [f64; 4],
}

impl CameraEffectsResource {
    pub fn profile(&self, profile: CameraProfile) -> Option<&CameraEffectsProfile> {
        self.profiles.get(profile_name(profile))
    }

    pub fn camera_effect(
        &self,
        profile: CameraProfile,
        camera_name: &str,
    ) -> Option<CameraEffect<'_>> {
        let profile = self.profile(profile)?;
        if !profile.camera_names.iter().any(|name| name == camera_name) {
            return None;
        }
        let perlin = profile.perlin.as_ref().and_then(|perlin| {
            let gain = perlin
                .gain_overrides
                .get(camera_name)
                .copied()
                .unwrap_or(perlin.default_gain);
            self.noise_profiles
                .get(&perlin.noise_profile)
                .map(|noise_profile| CameraPerlinEffect {
                    enabled: perlin.enabled,
                    amplitude_gain: gain[0],
                    frequency_gain: gain[1],
                    pivot_offset: perlin.pivot_offset,
                    noise_offsets: perlin.noise_offsets,
                    noise_profile,
                })
        });
        Some(CameraEffect {
            custom_look_at_target: profile
                .custom_look_at_target
                .iter()
                .any(|name| name == camera_name),
            perlin,
        })
    }

    pub fn validate(&self) -> Result<(), CameraEffectsError> {
        if self.schema != CAMERA_EFFECTS_SCHEMA {
            return Err(CameraEffectsError::UnsupportedSchema(self.schema.clone()));
        }
        if self.schema_version != CAMERA_EFFECTS_SCHEMA_VERSION {
            return Err(CameraEffectsError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        if self.channel_tuple != ["amplitude", "frequency", "constant"] {
            return Err(CameraEffectsError::InvalidTupleContract("channelTuple"));
        }
        if self.octave_tuple != ["x", "y", "z"] {
            return Err(CameraEffectsError::InvalidTupleContract("octaveTuple"));
        }

        for (name, noise_profile) in &self.noise_profiles {
            for octave in noise_profile
                .position_octaves
                .iter()
                .chain(&noise_profile.orientation_octaves)
            {
                for channel in [&octave.0, &octave.1, &octave.2] {
                    if !channel.0.is_finite() || !channel.1.is_finite() {
                        return Err(CameraEffectsError::NonFiniteNoiseProfile(name.clone()));
                    }
                }
            }
        }

        for (profile_name, profile) in &self.profiles {
            let mut names = BTreeSet::new();
            for camera_name in &profile.camera_names {
                if !names.insert(camera_name) {
                    return Err(CameraEffectsError::DuplicateCamera {
                        profile: profile_name.clone(),
                        camera: camera_name.clone(),
                    });
                }
            }
            for camera_name in &profile.custom_look_at_target {
                if !names.contains(camera_name) {
                    return Err(CameraEffectsError::UnknownCamera {
                        profile: profile_name.clone(),
                        camera: camera_name.clone(),
                    });
                }
            }
            let Some(perlin) = &profile.perlin else {
                continue;
            };
            if !self.noise_profiles.contains_key(&perlin.noise_profile) {
                return Err(CameraEffectsError::MissingNoiseProfile(
                    perlin.noise_profile.clone(),
                ));
            }
            if !all_finite(&perlin.pivot_offset)
                || !all_finite(&perlin.noise_offsets)
                || !all_finite(&perlin.default_gain)
            {
                return Err(CameraEffectsError::NonFinitePerlinProfile(
                    profile_name.clone(),
                ));
            }
            for (camera_name, gain) in &perlin.gain_overrides {
                if !names.contains(camera_name) {
                    return Err(CameraEffectsError::UnknownCamera {
                        profile: profile_name.clone(),
                        camera: camera_name.clone(),
                    });
                }
                if !all_finite(gain) {
                    return Err(CameraEffectsError::NonFinitePerlinProfile(
                        profile_name.clone(),
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn validate_timeline_coverage(
        &self,
        timeline: &CameraTimelineResource,
    ) -> Result<(), CameraEffectsError> {
        self.validate()?;
        for profile in [CameraProfile::Low, CameraProfile::Mid, CameraProfile::High] {
            let Some(timeline_profile) = timeline.profile(profile) else {
                continue;
            };
            let effects_profile = self
                .profile(profile)
                .ok_or_else(|| CameraEffectsError::MissingProfile(profile_name(profile).into()))?;
            for camera_name in timeline_profile.cameras.keys().map(String::as_str).chain(
                timeline_profile
                    .base_intro_camera_name
                    .iter()
                    .map(String::as_str),
            ) {
                if !effects_profile
                    .camera_names
                    .iter()
                    .any(|name| name == camera_name)
                {
                    return Err(CameraEffectsError::MissingTimelineCamera {
                        profile: profile_name(profile).into(),
                        camera: camera_name.into(),
                    });
                }
            }
        }
        Ok(())
    }

    /// True for the retained stage contract whose Perlin pipeline consists of
    /// orientation noise with a zero pivot. Position noise, constant channels,
    /// and non-zero pivots are deliberately rejected until native samples pin
    /// their pipeline order too.
    pub fn supports_exact_sampling(&self) -> bool {
        if self.sampling.cinemachine != "3.1"
            || self.sampling.time_base != "absolute-seconds-times-frequency-gain"
            || self.sampling.noise_function != "unity-mathf-perlin-noise-2d"
        {
            return false;
        }
        self.profiles.values().all(|profile| {
            let Some(perlin) = profile.perlin.as_ref().filter(|perlin| perlin.enabled) else {
                return true;
            };
            perlin
                .pivot_offset
                .iter()
                .all(|component| *component == 0.0)
                && self
                    .noise_profiles
                    .get(&perlin.noise_profile)
                    .is_some_and(|noise| {
                        noise.position_octaves.is_empty()
                            && noise.orientation_octaves.iter().all(|octave| {
                                [&octave.0, &octave.1, &octave.2]
                                    .iter()
                                    .all(|channel| !channel.2)
                            })
                    })
        })
    }

    /// Evaluates the score Timeline using a pauseable scene/game uptime that is
    /// intentionally independent of score time. Seeking score time therefore
    /// does not reset Perlin phase.
    pub fn evaluate_timeline(
        &self,
        timeline: &CameraTimelineResource,
        profile: CameraProfile,
        score_time_seconds: f64,
        scene_uptime_seconds: f64,
    ) -> Result<Option<CameraEvaluation>, CameraEffectsEvaluationError> {
        if !scene_uptime_seconds.is_finite() {
            return Err(CameraEffectsEvaluationError::NonFiniteSceneUptime);
        }
        if !self.supports_exact_sampling() {
            return Err(CameraEffectsEvaluationError::UnsupportedEffectsContract);
        }
        let timeline_profile = timeline.profile(profile).ok_or_else(|| {
            CameraEffectsEvaluationError::MissingTimelineProfile(profile_name(profile).into())
        })?;
        let Some(inputs) = timeline_profile
            .evaluate_blend_inputs(&timeline.curves, score_time_seconds)
            .map_err(CameraEffectsEvaluationError::Camera)?
        else {
            return Ok(None);
        };
        self.evaluate_blend(profile, inputs, scene_uptime_seconds)
            .map(Some)
    }

    fn evaluate_blend(
        &self,
        profile: CameraProfile,
        inputs: CameraBlendEvaluation<'_>,
        scene_uptime_seconds: f64,
    ) -> Result<CameraEvaluation, CameraEffectsEvaluationError> {
        let raw = inputs.evaluate_raw();
        let outgoing_correction =
            self.orientation_correction(profile, inputs.outgoing, scene_uptime_seconds)?;
        let (raw_orientation, correction) = if let Some(incoming) = inputs.incoming {
            let amount = (inputs.incoming_weight as f32).clamp(0.0, 1.0);
            let incoming_correction =
                self.orientation_correction(profile, incoming, scene_uptime_seconds)?;
            let correction = shortest_arc_slerp_f32(
                outgoing_correction.map(f64::from),
                incoming_correction.map(f64::from),
                amount,
            );
            (
                raw.state.rotation.map(|component| component as f32),
                correction.map(|component| component as f32),
            )
        } else {
            (
                inputs
                    .outgoing
                    .state
                    .rotation
                    .map(|component| component as f32),
                outgoing_correction,
            )
        };
        let final_orientation = quaternion_multiply_f32(raw_orientation, correction);
        Ok(CameraEvaluation {
            state: CameraState {
                rotation: final_orientation.map(f64::from),
                ..raw.state
            },
            ..raw
        })
    }

    fn orientation_correction(
        &self,
        profile: CameraProfile,
        input: CameraBlendInput<'_>,
        scene_uptime_seconds: f64,
    ) -> Result<[f32; 4], CameraEffectsEvaluationError> {
        let Some(camera_name) = input.camera_name else {
            return Ok([0.0, 0.0, 0.0, 1.0]);
        };
        let effect = self
            .camera_effect(profile, camera_name)
            .ok_or_else(|| CameraEffectsEvaluationError::MissingCameraEffect(camera_name.into()))?;
        let Some(perlin) = effect.perlin.filter(|perlin| perlin.enabled) else {
            return Ok([0.0, 0.0, 0.0, 1.0]);
        };
        perlin
            .sample(scene_uptime_seconds)
            .map(|sample| sample.orientation_correction.map(|value| value as f32))
    }
}

impl CameraPerlinEffect<'_> {
    /// Samples one virtual camera's BasicMultiChannelPerlin orientation. All
    /// arithmetic is narrowed to float32 in Unity's operation order.
    pub fn sample(
        self,
        scene_uptime_seconds: f64,
    ) -> Result<CameraPerlinSample, CameraEffectsEvaluationError> {
        if !scene_uptime_seconds.is_finite() {
            return Err(CameraEffectsEvaluationError::NonFiniteSceneUptime);
        }
        if !self.noise_profile.position_octaves.is_empty()
            || self.pivot_offset.iter().any(|component| *component != 0.0)
            || self
                .noise_profile
                .orientation_octaves
                .iter()
                .flat_map(|octave| [&octave.0, &octave.1, &octave.2])
                .any(|channel| channel.2)
        {
            return Err(CameraEffectsEvaluationError::UnsupportedEffectsContract);
        }

        let scene_uptime = scene_uptime_seconds as f32;
        let frequency_gain = self.frequency_gain as f32;
        let noise_time = scene_uptime * frequency_gain;
        if !self.enabled {
            return Ok(CameraPerlinSample {
                noise_time_seconds: f64::from(noise_time),
                orientation_degrees: [0.0; 3],
                orientation_correction: [0.0, 0.0, 0.0, 1.0],
            });
        }
        let mut orientation = [0.0_f32; 3];
        for octave in &self.noise_profile.orientation_octaves {
            for (axis, channel) in [&octave.0, &octave.1, &octave.2].into_iter().enumerate() {
                orientation[axis] += sample_channel(noise_time, self.noise_offsets[axis], channel);
            }
        }
        let amplitude_gain = self.amplitude_gain as f32;
        for component in &mut orientation {
            *component *= amplitude_gain;
        }
        let correction = unity_euler_zxy_quaternion_f32(orientation);
        Ok(CameraPerlinSample {
            noise_time_seconds: f64::from(noise_time),
            orientation_degrees: orientation.map(f64::from),
            orientation_correction: correction.map(f64::from),
        })
    }
}

fn sample_channel(noise_time: f32, axis_offset: f64, channel: &CameraNoiseChannel) -> f32 {
    let amplitude = channel.0 as f32;
    let frequency = channel.1 as f32;
    let sample_time = noise_time * frequency;
    let perlin_x = sample_time + axis_offset as f32;
    let centered = unity_mathf_perlin_noise(perlin_x, 0.0) - 0.5_f32;
    centered * amplitude
}

const fn profile_name(profile: CameraProfile) -> &'static str {
    match profile {
        CameraProfile::Low => "low",
        CameraProfile::Mid => "mid",
        CameraProfile::High => "high",
    }
}

fn all_finite<const N: usize>(values: &[f64; N]) -> bool {
    values.iter().all(|value| value.is_finite())
}

/// Unity 6000.3's native `Mathf.PerlinNoise` path: improved Perlin with the
/// standard permutation table, absolute inputs, and Unity's float32 remap.
pub fn unity_mathf_perlin_noise(x: f32, y: f32) -> f32 {
    if !x.is_finite() || !y.is_finite() {
        return f32::NAN;
    }

    let x = x.abs();
    let y = y.abs();
    let x_floor = x.floor();
    let y_floor = y.floor();
    let lattice_x = x_floor as i64 as usize;
    let lattice_y = y_floor as i64 as usize;
    let local_x = x - x_floor;
    let local_y = y - y_floor;
    let fade_x = perlin_fade(local_x);
    let fade_y = perlin_fade(local_y);

    let a = usize::from(permutation(lattice_x)) + lattice_y;
    let b = usize::from(permutation(lattice_x + 1)) + lattice_y;
    // The final lookup is intentional: Unity hashes the z=0 lattice plane with
    // the same third permutation used by the 3D improved-Perlin implementation.
    let hash_aa = permutation(usize::from(permutation(a)));
    let hash_ba = permutation(usize::from(permutation(b)));
    let hash_ab = permutation(usize::from(permutation(a + 1)));
    let hash_bb = permutation(usize::from(permutation(b + 1)));

    let lower_left = perlin_gradient(hash_aa, local_x, local_y, 0.0);
    let lower_right = perlin_gradient(hash_ba, local_x - 1.0, local_y, 0.0);
    let upper_left = perlin_gradient(hash_ab, local_x, local_y - 1.0, 0.0);
    let upper_right = perlin_gradient(hash_bb, local_x - 1.0, local_y - 1.0, 0.0);
    let lower = perlin_lerp(fade_x, lower_left, lower_right);
    let upper = perlin_lerp(fade_x, upper_left, upper_right);
    let raw = perlin_lerp(fade_y, lower, upper);
    let shifted = raw + 0.69_f32;
    shifted / 1.483_f32
}

fn perlin_fade(value: f32) -> f32 {
    let squared = value * value;
    let cubed = squared * value;
    let six_t = value * 6.0_f32;
    let inner = six_t - 15.0_f32;
    let quadratic = value * inner;
    let polynomial = quadratic + 10.0_f32;
    cubed * polynomial
}

fn perlin_lerp(amount: f32, outgoing: f32, incoming: f32) -> f32 {
    let difference = incoming - outgoing;
    let weighted = amount * difference;
    outgoing + weighted
}

fn perlin_gradient(hash: u8, x: f32, y: f32, z: f32) -> f32 {
    let hash = hash & 15;
    let u = if hash < 8 { x } else { y };
    let v = if hash < 4 {
        y
    } else if hash == 12 || hash == 14 {
        x
    } else {
        z
    };
    let first = if hash & 1 == 0 { u } else { -u };
    let second = if hash & 2 == 0 { v } else { -v };
    first + second
}

fn permutation(index: usize) -> u8 {
    PERMUTATION[index & 255]
}

#[rustfmt::skip]
const PERMUTATION: [u8; 256] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225,
    140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148,
    247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32,
    57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
    74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122,
    60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54,
    65, 25, 63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169,
    200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186, 3, 64,
    52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212,
    207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213,
    119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9,
    129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104,
    218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162, 241,
    81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157,
    184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93,
    222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CameraEffectsEvaluationError {
    NonFiniteSceneUptime,
    UnsupportedEffectsContract,
    MissingTimelineProfile(String),
    MissingCameraEffect(String),
    Camera(CameraEvaluationError),
}

impl fmt::Display for CameraEffectsEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteSceneUptime => formatter.write_str("camera scene uptime must be finite"),
            Self::UnsupportedEffectsContract => formatter.write_str(
                "camera effects require unsupported position, pivot, or constant-channel sampling",
            ),
            Self::MissingTimelineProfile(profile) => {
                write!(
                    formatter,
                    "camera timeline profile is unavailable: {profile}"
                )
            }
            Self::MissingCameraEffect(camera) => {
                write!(formatter, "camera effect is unavailable: {camera}")
            }
            Self::Camera(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CameraEffectsEvaluationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CameraEffectsError {
    UnsupportedSchema(String),
    UnsupportedSchemaVersion(u32),
    InvalidTupleContract(&'static str),
    MissingNoiseProfile(String),
    MissingProfile(String),
    DuplicateCamera { profile: String, camera: String },
    UnknownCamera { profile: String, camera: String },
    MissingTimelineCamera { profile: String, camera: String },
    NonFiniteNoiseProfile(String),
    NonFinitePerlinProfile(String),
}

impl fmt::Display for CameraEffectsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema(schema) => {
                write!(formatter, "unsupported camera-effects schema: {schema}")
            }
            Self::UnsupportedSchemaVersion(version) => {
                write!(
                    formatter,
                    "unsupported camera-effects schema version: {version}"
                )
            }
            Self::InvalidTupleContract(name) => {
                write!(formatter, "invalid camera-effects tuple: {name}")
            }
            Self::MissingNoiseProfile(name) => {
                write!(formatter, "missing camera noise profile: {name}")
            }
            Self::MissingProfile(name) => {
                write!(formatter, "missing camera-effects profile: {name}")
            }
            Self::DuplicateCamera { profile, camera } => {
                write!(
                    formatter,
                    "duplicate camera-effects camera: {profile}/{camera}"
                )
            }
            Self::UnknownCamera { profile, camera } => {
                write!(
                    formatter,
                    "unknown camera-effects camera: {profile}/{camera}"
                )
            }
            Self::MissingTimelineCamera { profile, camera } => {
                write!(
                    formatter,
                    "camera effects do not cover timeline camera: {profile}/{camera}"
                )
            }
            Self::NonFiniteNoiseProfile(name) => {
                write!(
                    formatter,
                    "camera noise profile contains a non-finite value: {name}"
                )
            }
            Self::NonFinitePerlinProfile(name) => {
                write!(
                    formatter,
                    "camera Perlin profile contains a non-finite value: {name}"
                )
            }
        }
    }
}

impl std::error::Error for CameraEffectsError {}

#[cfg(test)]
mod tests {
    use super::*;

    const EFFECTS: &str = r#"{
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
        "cameraNames":["base","a","b"],
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

    const TIMELINE: &str = r#"{
      "schema":"caph-live-camera-timelines-v1",
      "curves":{},
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
        "clips":[]
      }}
    }"#;

    #[test]
    fn compact_effects_resolve_without_sampling_unproven_noise() {
        let effects: CameraEffectsResource = serde_json::from_str(EFFECTS).unwrap();
        effects.validate().unwrap();
        let effect = effects.camera_effect(CameraProfile::Low, "b").unwrap();
        assert!(effect.custom_look_at_target);
        let perlin = effect.perlin.unwrap();
        assert_eq!(perlin.amplitude_gain, 0.2);
        assert_eq!(perlin.frequency_gain, 2.0);
        assert_eq!(perlin.noise_profile.position_octaves, []);
        assert_eq!(perlin.noise_profile.orientation_octaves.len(), 1);
        assert!(
            effects
                .camera_effect(CameraProfile::Low, "missing")
                .is_none()
        );
    }

    #[test]
    fn effects_validate_against_the_timeline_contract() {
        let effects: CameraEffectsResource = serde_json::from_str(EFFECTS).unwrap();
        let timeline: CameraTimelineResource = serde_json::from_str(TIMELINE).unwrap();
        effects.validate_timeline_coverage(&timeline).unwrap();
    }

    fn handheld_effect(frequency_gain: f64) -> CameraPerlinEffect<'static> {
        let profile = Box::leak(Box::new(CameraNoiseProfile {
            position_octaves: Vec::new(),
            orientation_octaves: vec![
                CameraNoiseOctave(
                    CameraNoiseChannel(4.0, 0.2, false),
                    CameraNoiseChannel(2.0, 0.15, false),
                    CameraNoiseChannel(0.0, 0.0, false),
                ),
                CameraNoiseOctave(
                    CameraNoiseChannel(2.0, 0.4, false),
                    CameraNoiseChannel(2.0, 0.5, false),
                    CameraNoiseChannel(0.0, 0.0, false),
                ),
                CameraNoiseOctave(
                    CameraNoiseChannel(1.0, 0.7, false),
                    CameraNoiseChannel(1.0, 0.6, false),
                    CameraNoiseChannel(0.0, 0.0, false),
                ),
            ],
        }));
        CameraPerlinEffect {
            enabled: true,
            amplitude_gain: 1.0,
            frequency_gain,
            pivot_offset: [0.0; 3],
            noise_offsets: [347.368896484375, 731.6524658203125, -17.897705078125],
            noise_profile: profile,
        }
    }

    #[test]
    fn unity_native_handheld_orientation_matches_bit_goldens() {
        let cases = [
            (1.0, 0.0, 0x3f0c_05e8, 0xbe31_cd44),
            (1.0, 0.25, 0x3f4d_42e4, 0xbe31_cd44),
            (1.0, 1.0, 0x3f39_487e, 0xbe84_92c7),
            (1.0, 4.0, 0xbf49_37ca, 0xbecd_5bfa),
            (0.1, 0.0, 0x3f0c_05e8, 0xbe31_cd44),
            (0.1, 0.25, 0x3f13_8659, 0xbe31_cd44),
            (0.1, 1.0, 0x3f29_1482, 0xbe31_cd44),
            (0.1, 4.0, 0x3f64_304e, 0xbe31_cd44),
            (2.0, 0.0, 0x3f0c_05e8, 0xbe31_cd44),
            (2.0, 0.25, 0x3f6a_fc1d, 0xbe31_cd44),
            (2.0, 1.0, 0xbdf4_f454, 0xbf21_7ea8),
            (2.0, 4.0, 0x3d6e_1a98, 0xbed6_a2ec),
        ];
        for (frequency_gain, scene_uptime, expected_x, expected_y) in cases {
            let sample = handheld_effect(frequency_gain)
                .sample(scene_uptime)
                .unwrap();
            assert_eq!(
                (sample.orientation_degrees[0] as f32).to_bits(),
                expected_x,
                "x at frequencyGain={frequency_gain}, sceneUptime={scene_uptime}"
            );
            assert_eq!(
                (sample.orientation_degrees[1] as f32).to_bits(),
                expected_y,
                "y at frequencyGain={frequency_gain}, sceneUptime={scene_uptime}"
            );
            assert_eq!(sample.orientation_degrees[2], 0.0);
        }

        for (amplitude_gain, expected_x, expected_y) in [
            (0.1, 0x3d60_0973, 0xbc8e_3dd0),
            (0.2, 0x3de0_0973, 0xbd0e_3dd0),
            (1.0, 0x3f0c_05e8, 0xbe31_cd44),
        ] {
            let mut effect = handheld_effect(1.0);
            effect.amplitude_gain = amplitude_gain;
            let sample = effect.sample(0.0).unwrap();
            assert_eq!((sample.orientation_degrees[0] as f32).to_bits(), expected_x);
            assert_eq!((sample.orientation_degrees[1] as f32).to_bits(), expected_y);
        }
    }

    #[test]
    fn timeline_blends_raw_orientation_and_per_camera_corrections_separately() {
        let effects: CameraEffectsResource = serde_json::from_str(EFFECTS).unwrap();
        let mut timeline: CameraTimelineResource = serde_json::from_str(TIMELINE).unwrap();
        timeline.curves.insert(
            "linear".into(),
            vec![
                crate::camera::CameraCurveKey(0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0),
                crate::camera::CameraCurveKey(1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0),
            ],
        );
        timeline.profiles.get_mut("low").unwrap().clips = vec![
            crate::camera::CameraClip(
                "a".into(),
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
                7,
            ),
            crate::camera::CameraClip(
                "b".into(),
                1.0,
                1.0,
                1.0,
                -1.0,
                "linear".into(),
                "linear".into(),
                0,
                0,
                0.0,
                1.0,
                8,
            ),
        ];
        let profile = timeline.profile(CameraProfile::Low).unwrap();
        let inputs = profile
            .evaluate_blend_inputs(&timeline.curves, 1.5)
            .unwrap()
            .unwrap();
        let incoming = inputs.incoming.unwrap();
        let amount = inputs.incoming_weight as f32;
        let outgoing_correction = effects
            .camera_effect(CameraProfile::Low, inputs.outgoing.camera_name.unwrap())
            .unwrap()
            .perlin
            .unwrap()
            .sample(1.0)
            .unwrap()
            .orientation_correction;
        let incoming_correction = effects
            .camera_effect(CameraProfile::Low, incoming.camera_name.unwrap())
            .unwrap()
            .perlin
            .unwrap()
            .sample(1.0)
            .unwrap()
            .orientation_correction;
        let raw = shortest_arc_slerp_f32(
            inputs.outgoing.state.rotation,
            incoming.state.rotation,
            amount,
        );
        let correction = shortest_arc_slerp_f32(outgoing_correction, incoming_correction, amount);
        let expected = quaternion_multiply_f32(
            raw.map(|component| component as f32),
            correction.map(|component| component as f32),
        );
        let sampled = effects
            .evaluate_timeline(&timeline, CameraProfile::Low, 1.5, 1.0)
            .unwrap()
            .unwrap();
        assert_eq!(sampled.state.rotation, expected.map(f64::from));

        let outgoing_final = quaternion_multiply_f32(
            inputs
                .outgoing
                .state
                .rotation
                .map(|component| component as f32),
            outgoing_correction.map(|component| component as f32),
        );
        let incoming_final = quaternion_multiply_f32(
            incoming.state.rotation.map(|component| component as f32),
            incoming_correction.map(|component| component as f32),
        );
        let incorrectly_postcomposed = shortest_arc_slerp_f32(
            outgoing_final.map(f64::from),
            incoming_final.map(f64::from),
            amount,
        );
        assert_ne!(sampled.state.rotation, incorrectly_postcomposed);
    }
}
