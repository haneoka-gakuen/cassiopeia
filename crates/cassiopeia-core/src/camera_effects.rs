use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{camera::CameraTimelineResource, live::CameraProfile};

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
}
