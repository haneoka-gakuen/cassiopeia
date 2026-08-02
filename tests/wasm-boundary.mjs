import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import init, {
  CameraFrameWord,
  CassiopeiaCameraTimeline,
  CassiopeiaPerformanceClock,
  cassiopeiaMemory,
  initSync,
  resolvePerformanceScreenMode,
} from "@haneoka/cassiopeia/wasm";

const moduleUrl = import.meta.resolve("@haneoka/cassiopeia/wasm/module");
const moduleBytes = readFileSync(new URL(moduleUrl));
const asyncWasm = await import(
  new URL("../dist/wasm/cassiopeia_wasm.js?async-boundary", import.meta.url)
);
const asyncInitialized = await asyncWasm.default({ module_or_path: moduleBytes });
assert.ok(asyncInitialized.memory instanceof WebAssembly.Memory);
assert.equal(asyncWasm.resolvePerformanceScreenMode(2, false, false, false), 3);

const initialized = initSync({ module: moduleBytes });
assert.ok(initialized.memory instanceof WebAssembly.Memory);
assert.strictEqual(await init({ module_or_path: moduleBytes }), initialized);

assert.equal(resolvePerformanceScreenMode(3, true, true, true), 3);
assert.equal(resolvePerformanceScreenMode(2, false, false, false), 3);
assert.equal(resolvePerformanceScreenMode(0, true, false, true), 1);

const cameraResource = new TextEncoder().encode(
  JSON.stringify({
    schema: "caph-live-camera-timelines-v1",
    curves: {
      linear: [
        [0, 0, 1, 1, 0, 0, 0],
        [1, 1, 1, 1, 0, 0, 0],
      ],
    },
    profiles: {
      low: {
        durationSeconds: 2,
        frameRate: 60,
        wrapMode: 2,
        baseIntroCameraName: null,
        baseIntroCamera: null,
        cameras: {
          a: {
            position: [0, 0, 0],
            rotation: [0, 0, 0, 1],
            verticalFovDegrees: 40,
          },
          b: {
            position: [10, 20, 30],
            rotation: [0, 0, 1, 0],
            verticalFovDegrees: 60,
          },
        },
        clips: [
          ["a", 0, 2, -1, -1, "linear", "linear", 0, 0, 0, 1, 7],
          ["b", 1, 1, 1, -1, "linear", "linear", 0, 0, 0, 1, 8],
        ],
      },
    },
  }),
);

const cameraEffects = new TextEncoder().encode(
  JSON.stringify({
    schema: "org.haneoka.caph.live-camera-effects",
    schemaVersion: 1,
    channelTuple: ["amplitude", "frequency", "constant"],
    octaveTuple: ["x", "y", "z"],
    sampling: {
      cinemachine: "3.1",
      timeBase: "absolute-seconds-times-frequency-gain",
      noiseFunction: "unity-mathf-perlin-noise-2d",
      implementationStatus: "parameters-only",
    },
    noiseProfiles: {},
    profiles: {
      low: {
        cameraNames: ["a", "b"],
        customLookAtTarget: [],
        perlin: null,
      },
    },
  }),
);

const chorusCamera = new TextEncoder().encode(
  JSON.stringify({
    schema: "org.haneoka.caph.live-chorus-camera",
    schemaVersion: 1,
    durationSeconds: 10,
    frameRate: 60,
    playback: "once",
    completion: "restore-score-timeline",
    coefficientTuple: ["timeSeconds", "a", "b", "c", "d"],
    channels: {
      position: {
        x: [[0, 0, 0, 0, 0]],
        y: [[0, 0, 0, 0, 1]],
        z: [[0, 0, 0, 0, -15]],
      },
      eulerDegrees: {
        x: [[0, 0, 0, 0, -4]],
        y: [[0, 0, 0, 0, 0]],
        z: [[0, 0, 0, 0, 0]],
      },
      verticalFovDegrees: [[0, 0, 0, 0, 20]],
    },
  }),
);

const timeline = new CassiopeiaCameraTimeline(cameraResource);
const framePointer = timeline.frameBufferPtr();
assert.equal(timeline.frameBufferLen(), 13);
assert.equal(timeline.durationSeconds(0), 2);
assert.equal(timeline.frameRate(0), 60);
assert.equal(timeline.evaluate(0, 1.5), true);
assert.equal(timeline.frameBufferPtr(), framePointer);

const frame = new Float64Array(
  cassiopeiaMemory().buffer,
  framePointer,
  timeline.frameBufferLen(),
);
assert.equal(frame[CameraFrameWord.AbiVersion], 1);
assert.equal(frame[CameraFrameWord.Available], 1);
assert.equal(frame[CameraFrameWord.ActiveClips], 2);
assert.ok(Math.abs(frame[CameraFrameWord.IncomingWeight] - 0.5) < 1e-10);
assert.ok(Math.abs(frame[CameraFrameWord.PositionX] - 5) < 1e-10);
assert.ok(Math.abs(frame[CameraFrameWord.VerticalFovDegrees] - 50) < 1e-10);

const chorusTimeline = CassiopeiaCameraTimeline.withEffectsAndChorus(
  cameraResource,
  cameraEffects,
  chorusCamera,
);
const scorePointer = chorusTimeline.frameBufferPtr();
const chorusPointer = chorusTimeline.chorusFrameBufferPtr();
assert.notEqual(scorePointer, chorusPointer);
assert.equal(chorusTimeline.chorusFrameBufferLen(), 13);
assert.equal(chorusTimeline.hasChorus(), true);
assert.equal(chorusTimeline.chorusDurationSeconds(), 10);
assert.equal(chorusTimeline.chorusFrameRate(), 60);

assert.equal(chorusTimeline.evaluate(0, 1.5), true);
const scoreFrame = new Float64Array(
  cassiopeiaMemory().buffer,
  scorePointer,
  chorusTimeline.frameBufferLen(),
);
const scoreSnapshot = Array.from(scoreFrame);
assert.equal(chorusTimeline.evaluateChorus(0), true);
const chorusFrame = new Float64Array(
  cassiopeiaMemory().buffer,
  chorusPointer,
  chorusTimeline.chorusFrameBufferLen(),
);
assert.deepEqual(Array.from(scoreFrame), scoreSnapshot);
assert.equal(chorusFrame[CameraFrameWord.AbiVersion], 1);
assert.equal(chorusFrame[CameraFrameWord.Available], 1);
assert.equal(chorusFrame[CameraFrameWord.ActiveClips], 1);
assert.equal(chorusFrame[CameraFrameWord.IncomingWeight], 1);
assert.equal(chorusFrame[CameraFrameWord.IncomingSerializedIndex], -1);
assert.deepEqual(
  Array.from(
    chorusFrame.subarray(
      CameraFrameWord.PositionX,
      CameraFrameWord.PositionZ + 1,
    ),
  ),
  [0, 1, -15],
);
assert.ok(
  Math.abs(chorusFrame[CameraFrameWord.RotationX] + 0.03489949554204941) <
    1e-7,
);
assert.equal(chorusFrame[CameraFrameWord.VerticalFovDegrees], 20);
assert.throws(() => chorusTimeline.evaluateChorus(Number.NaN));
assert.equal(chorusFrame[CameraFrameWord.Available], 0);
assert.deepEqual(Array.from(scoreFrame), scoreSnapshot);
assert.equal(chorusTimeline.evaluateChorus(10 - 1 / 60), true);
assert.equal(chorusTimeline.evaluateChorus(10), false);
assert.equal(chorusFrame[CameraFrameWord.Available], 0);
assert.equal(chorusTimeline.frameBufferPtr(), scorePointer);
assert.equal(chorusTimeline.chorusFrameBufferPtr(), chorusPointer);

const clock = new CassiopeiaPerformanceClock(0n, 0n);
clock.resume(0n);
assert.equal(clock.timeMicros(1_000_000n), 1_000_000n);
clock.setRate(1_000_000n, 2_000_000);
assert.equal(clock.timeMicros(1_500_000n), 2_000_000n);
clock.pause(1_500_000n);
assert.equal(clock.timeMicros(9_000_000n), 2_000_000n);

clock.free();
chorusTimeline.free();
timeline.free();
console.log(
  "WASM boundary: package export, resolver, clock, score camera, and chorus camera buffers passed",
);
