use haneoka_cassiopeia_core::{
    CameraChorusResource, CameraEffectsResource, CameraProfile, CameraTimelineResource,
};
use std::path::{Path, PathBuf};

fn resource_path(variable: &str) -> Option<PathBuf> {
    let value = std::env::var_os(variable);
    if value.is_none() && std::env::var_os("CASSIOPEIA_REQUIRE_CAPH_CAMERA_RESOURCES").is_some() {
        panic!("missing required Caph camera resource environment variable: {variable}");
    }
    value.map(PathBuf::from)
}

fn parse<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn retained_timeline_and_effects_contracts_match() {
    let Some(timeline_path) = resource_path("CAPH_CAMERA_TIMELINES") else {
        return;
    };
    let Some(effects_path) = resource_path("CAPH_CAMERA_EFFECTS") else {
        return;
    };
    let timeline: CameraTimelineResource = parse(&timeline_path);
    let effects: CameraEffectsResource = parse(&effects_path);
    effects.validate_timeline_coverage(&timeline).unwrap();
    assert!(effects.supports_exact_sampling());

    let perlin = effects
        .camera_effect(CameraProfile::Low, "intro_2")
        .unwrap()
        .perlin
        .unwrap();
    assert_eq!(perlin.amplitude_gain as f32, 1.0);
    assert_eq!(perlin.frequency_gain as f32, 1.0);
    for (time, expected_x, expected_y) in [
        (0.0, 0x3f0c_05e8, 0xbe31_cd44),
        (0.25, 0x3f4d_42e4, 0xbe31_cd44),
        (1.0, 0x3f39_487e, 0xbe84_92c7),
        (4.0, 0xbf49_37ca, 0xbecd_5bfa),
    ] {
        let sample = perlin.sample(time).unwrap();
        assert_eq!((sample.orientation_degrees[0] as f32).to_bits(), expected_x);
        assert_eq!((sample.orientation_degrees[1] as f32).to_bits(), expected_y);
    }
}

#[test]
fn retained_chorus_camera_matches_streamed_goldens() {
    let Some(path) = resource_path("CAPH_CHORUS_CAMERA") else {
        return;
    };
    let resource: CameraChorusResource = parse(&path);
    resource.validate().unwrap();

    let expected = [
        (0.0, [0.0, 1.0, -15.0, -4.0, 20.0]),
        (
            1.0,
            [
                0.0,
                0.27976134419441223,
                -18.94085693359375,
                -7.891794681549072,
                28.560626983642578,
            ],
        ),
        (4.0, [0.0, -2.299999952316284, -20.0, -12.0, 33.0]),
    ];
    for (time, expected) in expected {
        let frame = resource.evaluate(time).unwrap().unwrap();
        assert_eq!(frame.position, [expected[0], expected[1], expected[2]],);
        assert!(
            (quaternion_x_degrees(frame.rotation) - expected[3]).abs() < 1e-5,
            "unexpected X rotation at {time}s"
        );
        assert_eq!(frame.vertical_fov_degrees, expected[4]);
    }
    assert!(resource.evaluate(10.0 - 1.0 / 60.0).unwrap().is_some());
    assert!(resource.evaluate(10.0).unwrap().is_none());
}

fn quaternion_x_degrees(quaternion: [f64; 4]) -> f64 {
    f64::from((2.0_f32 * (quaternion[0] as f32).atan2(quaternion[3] as f32)).to_degrees())
}
