use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

use crate::timing::TimeMicros;

pub const PENLIGHT_COLUMNS: u16 = 30;
pub const PENLIGHT_ROWS: u16 = 24;
pub const PENLIGHT_INSTANCE_COUNT: u16 = PENLIGHT_COLUMNS * PENLIGHT_ROWS;
pub const PLAYBACK_RATE_SCALE: u32 = 1_000_000;
pub const DEFAULT_SCREEN_MODE: ScreenMode = ScreenMode::Lightweight;

/// Stable presentation values used by native and web hosts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum ScreenMode {
    RichLive2D = 0,
    SimpleLive2D = 1,
    MV = 2,
    #[default]
    Lightweight = 3,
}

impl Serialize for ScreenMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8((*self).into())
    }
}

impl<'de> Deserialize<'de> for ScreenMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(u8::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl TryFrom<u8> for ScreenMode {
    type Error = LivePerformanceError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::RichLive2D),
            1 => Ok(Self::SimpleLive2D),
            2 => Ok(Self::MV),
            3 => Ok(Self::Lightweight),
            _ => Err(LivePerformanceError::InvalidScreenMode(value)),
        }
    }
}

impl From<ScreenMode> for u8 {
    fn from(value: ScreenMode) -> Self {
        value as u8
    }
}

/// Camera quality is selected independently from [`ScreenMode`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum CameraProfile {
    #[default]
    Low = 0,
    Mid = 1,
    High = 2,
}

impl Serialize for CameraProfile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for CameraProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(u8::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl TryFrom<u8> for CameraProfile {
    type Error = LivePerformanceError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Low),
            1 => Ok(Self::Mid),
            2 => Ok(Self::High),
            _ => Err(LivePerformanceError::InvalidCameraProfile(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenResources {
    pub live2d_stage: bool,
    pub main_music_video: bool,
    pub band_video_jockey: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScreenFallback {
    None,
    MainMusicVideoUnavailable,
    Live2dStageUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenSelection {
    pub requested: ScreenMode,
    pub effective: ScreenMode,
    pub fallback: ScreenFallback,
    pub uses_live2d_stage: bool,
    pub uses_main_music_video: bool,
    pub uses_band_video_jockey: bool,
}

/// Resolves only downward fallbacks and never enables a mode not requested.
pub const fn resolve_screen_mode(
    requested: ScreenMode,
    resources: ScreenResources,
) -> ScreenSelection {
    let (effective, fallback) = match requested {
        ScreenMode::Lightweight => (ScreenMode::Lightweight, ScreenFallback::None),
        ScreenMode::MV if resources.main_music_video => (ScreenMode::MV, ScreenFallback::None),
        ScreenMode::MV => (
            ScreenMode::Lightweight,
            ScreenFallback::MainMusicVideoUnavailable,
        ),
        ScreenMode::SimpleLive2D if resources.live2d_stage => {
            (ScreenMode::SimpleLive2D, ScreenFallback::None)
        }
        ScreenMode::SimpleLive2D => (
            ScreenMode::Lightweight,
            ScreenFallback::Live2dStageUnavailable,
        ),
        ScreenMode::RichLive2D if !resources.live2d_stage => (
            ScreenMode::Lightweight,
            ScreenFallback::Live2dStageUnavailable,
        ),
        ScreenMode::RichLive2D if !resources.main_music_video => (
            ScreenMode::SimpleLive2D,
            ScreenFallback::MainMusicVideoUnavailable,
        ),
        ScreenMode::RichLive2D => (ScreenMode::RichLive2D, ScreenFallback::None),
    };
    ScreenSelection {
        requested,
        effective,
        fallback,
        uses_live2d_stage: matches!(effective, ScreenMode::RichLive2D | ScreenMode::SimpleLive2D),
        uses_main_music_video: matches!(effective, ScreenMode::RichLive2D | ScreenMode::MV),
        uses_band_video_jockey: matches!(effective, ScreenMode::SimpleLive2D)
            && resources.band_video_jockey,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlaybackRate(u32);

impl PlaybackRate {
    pub const NORMAL: Self = Self(PLAYBACK_RATE_SCALE);

    pub const fn from_millionths(value: u32) -> Result<Self, LivePerformanceError> {
        if value == 0 {
            Err(LivePerformanceError::InvalidPlaybackRate)
        } else {
            Ok(Self(value))
        }
    }

    pub const fn millionths(self) -> u32 {
        self.0
    }
}

impl Default for PlaybackRate {
    fn default() -> Self {
        Self::NORMAL
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackState {
    Playing,
    #[default]
    Paused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceSnapshot {
    pub time: TimeMicros,
    pub rate: PlaybackRate,
    pub state: PlaybackState,
    /// Changes after every transport discontinuity so schedulers can rebuild.
    pub revision: u64,
}

/// Host-clock anchored transport shared by every performance scheduler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PerformanceClock {
    anchor_host: TimeMicros,
    anchor_performance: TimeMicros,
    rate: PlaybackRate,
    state: PlaybackState,
    revision: u64,
}

impl PerformanceClock {
    pub const fn new(host_time: TimeMicros, performance_time: TimeMicros) -> Self {
        Self {
            anchor_host: host_time,
            anchor_performance: performance_time,
            rate: PlaybackRate::NORMAL,
            state: PlaybackState::Paused,
            revision: 0,
        }
    }

    pub fn snapshot(
        &self,
        host_time: TimeMicros,
    ) -> Result<PerformanceSnapshot, LivePerformanceError> {
        Ok(PerformanceSnapshot {
            time: self.performance_time(host_time)?,
            rate: self.rate,
            state: self.state,
            revision: self.revision,
        })
    }

    pub fn pause(&mut self, host_time: TimeMicros) -> Result<(), LivePerformanceError> {
        self.reanchor(host_time, self.rate, PlaybackState::Paused)
    }

    pub fn resume(&mut self, host_time: TimeMicros) -> Result<(), LivePerformanceError> {
        self.reanchor(host_time, self.rate, PlaybackState::Playing)
    }

    pub fn seek(
        &mut self,
        host_time: TimeMicros,
        performance_time: TimeMicros,
    ) -> Result<(), LivePerformanceError> {
        self.ensure_host_time(host_time)?;
        self.anchor_host = host_time;
        self.anchor_performance = performance_time;
        self.bump_revision();
        Ok(())
    }

    pub fn set_rate(
        &mut self,
        host_time: TimeMicros,
        rate: PlaybackRate,
    ) -> Result<(), LivePerformanceError> {
        self.reanchor(host_time, rate, self.state)
    }

    /// Atomically restores a transport from authoritative media state.
    pub fn rebuild(
        &mut self,
        host_time: TimeMicros,
        performance_time: TimeMicros,
        rate: PlaybackRate,
        state: PlaybackState,
    ) {
        self.anchor_host = host_time;
        self.anchor_performance = performance_time;
        self.rate = rate;
        self.state = state;
        self.bump_revision();
    }

    fn reanchor(
        &mut self,
        host_time: TimeMicros,
        rate: PlaybackRate,
        state: PlaybackState,
    ) -> Result<(), LivePerformanceError> {
        let performance_time = self.performance_time(host_time)?;
        self.anchor_host = host_time;
        self.anchor_performance = performance_time;
        self.rate = rate;
        self.state = state;
        self.bump_revision();
        Ok(())
    }

    fn performance_time(&self, host_time: TimeMicros) -> Result<TimeMicros, LivePerformanceError> {
        self.ensure_host_time(host_time)?;
        if self.state == PlaybackState::Paused {
            return Ok(self.anchor_performance);
        }
        let host_delta = i128::from(host_time.0) - i128::from(self.anchor_host.0);
        let scaled =
            host_delta * i128::from(self.rate.millionths()) / i128::from(PLAYBACK_RATE_SCALE);
        let performance_delta =
            i64::try_from(scaled).map_err(|_| LivePerformanceError::Overflow)?;
        self.anchor_performance
            .0
            .checked_add(performance_delta)
            .map(TimeMicros)
            .ok_or(LivePerformanceError::Overflow)
    }

    fn ensure_host_time(&self, host_time: TimeMicros) -> Result<(), LivePerformanceError> {
        if host_time < self.anchor_host {
            Err(LivePerformanceError::HostTimeWentBackwards)
        } else {
            Ok(())
        }
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PenlightColorSlot {
    pub weight: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PenlightInstance {
    pub index: u16,
    pub column: u16,
    pub row: u16,
    pub color_slot: u16,
    pub phase_turns: u32,
}

/// Stateless 30×24 penlight layout. Sampling performs no heap allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PenlightScheduler {
    seed: u64,
}

impl PenlightScheduler {
    pub const fn new(seed: u64) -> Self {
        Self { seed }
    }

    pub fn instance(&self, index: u16, palette: &[PenlightColorSlot]) -> Option<PenlightInstance> {
        if index >= PENLIGHT_INSTANCE_COUNT || palette.is_empty() {
            return None;
        }
        let random = splitmix64(self.seed ^ u64::from(index));
        let total_weight = palette.iter().fold(0_u64, |sum, slot| {
            sum.saturating_add(u64::from(slot.weight))
        });
        if total_weight == 0 {
            return None;
        }
        let target = random % total_weight;
        let mut cursor = 0_u64;
        let mut color_slot = 0_u16;
        for (slot_index, slot) in palette.iter().enumerate() {
            cursor = cursor.saturating_add(u64::from(slot.weight));
            if target < cursor {
                color_slot = u16::try_from(slot_index).ok()?;
                break;
            }
        }
        let phase_random = splitmix64(random);
        Some(PenlightInstance {
            index,
            column: index % PENLIGHT_COLUMNS,
            row: index / PENLIGHT_COLUMNS,
            color_slot,
            phase_turns: phase_random as u32,
        })
    }

    /// Returns a wrapping fixed-point phase at 0.9 cycles per second.
    pub fn phase_at(&self, instance: PenlightInstance, time: TimeMicros) -> u32 {
        const CYCLES_PER_SECOND_MILLIONTHS: i128 = 900_000;
        let turns = i128::from(time.0) * CYCLES_PER_SECOND_MILLIONTHS * (i128::from(u32::MAX) + 1)
            / 1_000_000_i128
            / 1_000_000_i128;
        instance.phase_turns.wrapping_add(turns as u32)
    }
}

const fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LivePerformanceError {
    InvalidScreenMode(u8),
    InvalidCameraProfile(u8),
    InvalidPlaybackRate,
    HostTimeWentBackwards,
    Overflow,
}

impl fmt::Display for LivePerformanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScreenMode(value) => write!(formatter, "invalid screen mode: {value}"),
            Self::InvalidCameraProfile(value) => {
                write!(formatter, "invalid camera profile: {value}")
            }
            Self::InvalidPlaybackRate => formatter.write_str("playback rate must be positive"),
            Self::HostTimeWentBackwards => formatter.write_str("host time moved backwards"),
            Self::Overflow => formatter.write_str("performance clock overflowed"),
        }
    }
}

impl std::error::Error for LivePerformanceError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_values_and_default_are_stable() {
        assert_eq!(ScreenMode::RichLive2D as u8, 0);
        assert_eq!(ScreenMode::SimpleLive2D as u8, 1);
        assert_eq!(ScreenMode::MV as u8, 2);
        assert_eq!(ScreenMode::Lightweight as u8, 3);
        assert_eq!(ScreenMode::default(), ScreenMode::Lightweight);
        assert_eq!(CameraProfile::Low as u8, 0);
        assert_eq!(CameraProfile::Mid as u8, 1);
        assert_eq!(CameraProfile::High as u8, 2);
        assert_eq!(serde_json::to_string(&ScreenMode::MV).unwrap(), "2");
        assert_eq!(
            serde_json::from_str::<ScreenMode>("3").unwrap(),
            ScreenMode::Lightweight
        );
        assert_eq!(serde_json::to_string(&CameraProfile::High).unwrap(), "2");
    }

    #[test]
    fn resolver_only_uses_requested_resources_and_downward_fallbacks() {
        let all = ScreenResources {
            live2d_stage: true,
            main_music_video: true,
            band_video_jockey: true,
        };
        for mode in [
            ScreenMode::RichLive2D,
            ScreenMode::SimpleLive2D,
            ScreenMode::MV,
            ScreenMode::Lightweight,
        ] {
            assert_eq!(resolve_screen_mode(mode, all).effective, mode);
        }

        let stage_only = ScreenResources {
            live2d_stage: true,
            main_music_video: false,
            band_video_jockey: true,
        };
        assert_eq!(
            resolve_screen_mode(ScreenMode::RichLive2D, stage_only).effective,
            ScreenMode::SimpleLive2D
        );
        assert_eq!(
            resolve_screen_mode(ScreenMode::MV, stage_only).effective,
            ScreenMode::Lightweight
        );

        let video_only = ScreenResources {
            live2d_stage: false,
            main_music_video: true,
            band_video_jockey: false,
        };
        assert_eq!(
            resolve_screen_mode(ScreenMode::SimpleLive2D, video_only).effective,
            ScreenMode::Lightweight
        );
        assert_eq!(
            resolve_screen_mode(ScreenMode::RichLive2D, video_only).effective,
            ScreenMode::Lightweight
        );
        assert_eq!(
            resolve_screen_mode(ScreenMode::Lightweight, all).effective,
            ScreenMode::Lightweight
        );

        for bits in 0_u8..8 {
            let resources = ScreenResources {
                live2d_stage: bits & 1 != 0,
                main_music_video: bits & 2 != 0,
                band_video_jockey: bits & 4 != 0,
            };
            for requested in [
                ScreenMode::RichLive2D,
                ScreenMode::SimpleLive2D,
                ScreenMode::MV,
                ScreenMode::Lightweight,
            ] {
                let selection = resolve_screen_mode(requested, resources);
                assert!(!selection.uses_live2d_stage || resources.live2d_stage);
                assert!(!selection.uses_main_music_video || resources.main_music_video);
                assert!(!selection.uses_band_video_jockey || resources.band_video_jockey);
                match requested {
                    ScreenMode::Lightweight => {
                        assert_eq!(selection.effective, ScreenMode::Lightweight)
                    }
                    ScreenMode::MV => assert!(matches!(
                        selection.effective,
                        ScreenMode::MV | ScreenMode::Lightweight
                    )),
                    ScreenMode::SimpleLive2D => assert!(matches!(
                        selection.effective,
                        ScreenMode::SimpleLive2D | ScreenMode::Lightweight
                    )),
                    ScreenMode::RichLive2D => assert!(matches!(
                        selection.effective,
                        ScreenMode::RichLive2D | ScreenMode::SimpleLive2D | ScreenMode::Lightweight
                    )),
                }
            }
        }
    }

    #[test]
    fn clock_rebuilds_exactly_after_transport_changes() {
        let mut clock = PerformanceClock::new(TimeMicros(10_000), TimeMicros(0));
        clock.resume(TimeMicros(10_000)).unwrap();
        assert_eq!(
            clock.snapshot(TimeMicros(510_000)).unwrap().time,
            TimeMicros(500_000)
        );

        clock
            .set_rate(
                TimeMicros(510_000),
                PlaybackRate::from_millionths(2_000_000).unwrap(),
            )
            .unwrap();
        assert_eq!(
            clock.snapshot(TimeMicros(760_000)).unwrap().time,
            TimeMicros(1_000_000)
        );
        clock.pause(TimeMicros(760_000)).unwrap();
        assert_eq!(
            clock.snapshot(TimeMicros(2_000_000)).unwrap().time,
            TimeMicros(1_000_000)
        );
        clock
            .seek(TimeMicros(2_000_000), TimeMicros(7_000_000))
            .unwrap();
        clock.resume(TimeMicros(2_000_000)).unwrap();
        assert_eq!(
            clock.snapshot(TimeMicros(2_125_000)).unwrap().time,
            TimeMicros(7_250_000)
        );

        clock.rebuild(
            TimeMicros(3_000_000),
            TimeMicros(0),
            PlaybackRate::from_millionths(500_000).unwrap(),
            PlaybackState::Playing,
        );
        assert_eq!(
            clock.snapshot(TimeMicros(5_000_000)).unwrap().time,
            TimeMicros(1_000_000)
        );

        let revision = clock.snapshot(TimeMicros(5_000_000)).unwrap().revision;
        clock.rebuild(
            TimeMicros(1),
            TimeMicros(123),
            PlaybackRate::NORMAL,
            PlaybackState::Paused,
        );
        let rebuilt = clock.snapshot(TimeMicros(1)).unwrap();
        assert_eq!(rebuilt.time, TimeMicros(123));
        assert!(rebuilt.revision > revision);
    }

    #[test]
    fn clock_rejects_non_monotonic_host_samples() {
        let mut clock = PerformanceClock::new(TimeMicros(100), TimeMicros(0));
        clock.resume(TimeMicros(100)).unwrap();
        assert_eq!(
            clock.snapshot(TimeMicros(99)),
            Err(LivePerformanceError::HostTimeWentBackwards)
        );
    }

    #[test]
    fn penlight_layout_is_stable_weighted_and_allocation_free_to_sample() {
        let scheduler = PenlightScheduler::new(42);
        let palette = [
            PenlightColorSlot { weight: 1 },
            PenlightColorSlot { weight: 3 },
        ];
        let first = scheduler.instance(0, &palette).unwrap();
        assert_eq!((first.column, first.row), (0, 0));
        assert_eq!(
            scheduler
                .instance(PENLIGHT_INSTANCE_COUNT - 1, &palette)
                .map(|item| (item.column, item.row)),
            Some((29, 23))
        );
        assert_eq!(scheduler.instance(0, &palette), Some(first));
        assert!(
            scheduler
                .instance(PENLIGHT_INSTANCE_COUNT, &palette)
                .is_none()
        );
        assert!(scheduler.instance(0, &[]).is_none());
        assert_ne!(
            scheduler.phase_at(first, TimeMicros(0)),
            scheduler.phase_at(first, TimeMicros(500_000))
        );
    }
}
