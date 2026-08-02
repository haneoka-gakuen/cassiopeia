import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import init, {
  CameraFrameWord,
  CassiopeiaCameraTimeline,
  CassiopeiaComboCutInSequencer,
  CassiopeiaPerformanceClock,
  ComboCutInEndReason,
  ComboCutInEventKind,
  ComboCutInEventWord,
  ComboCutInRole,
  ComboCutInSource,
  cassiopeiaMemory,
  initSync,
  resolveComboCutInMilestone,
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
    noiseProfiles: {
      handheld: {
        positionOctaves: [],
        orientationOctaves: [
          [
            [4, 0.2, false],
            [2, 0.15, false],
            [0, 0, false],
          ],
          [
            [2, 0.4, false],
            [2, 0.5, false],
            [0, 0, false],
          ],
          [
            [1, 0.7, false],
            [1, 0.6, false],
            [0, 0, false],
          ],
        ],
      },
    },
    profiles: {
      low: {
        cameraNames: ["a", "b"],
        customLookAtTarget: [],
        perlin: {
          enabled: true,
          noiseProfile: "handheld",
          pivotOffset: [0, 0, 0],
          noiseOffsets: [347.368896484375, 731.6524658203125, -17.897705078125],
          defaultGain: [0.1, 1],
          gainOverrides: { b: [0.2, 2] },
        },
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
assert.equal(chorusTimeline.effectsSamplingSupported(), true);
assert.equal(chorusTimeline.effectsStandbyRoundRobinSupported(), false);
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

assert.equal(chorusTimeline.evaluateWithEffects(0, 1.5, 1), true);
const effectFrame = Array.from(scoreFrame);
assert.equal(effectFrame[CameraFrameWord.PositionX], 5);
assert.notEqual(effectFrame[CameraFrameWord.RotationX], 0);
assert.equal(chorusTimeline.evaluateWithEffects(0, 0.5, 1), true);
const effectAfterSeek = Array.from(scoreFrame);
assert.equal(chorusTimeline.evaluateWithEffects(0, 0.5, 1), true);
assert.deepEqual(Array.from(scoreFrame), effectAfterSeek);
assert.equal(chorusTimeline.evaluateWithEffects(0, 0.5, 0), true);
assert.notDeepEqual(Array.from(scoreFrame), effectAfterSeek);
assert.throws(() => chorusTimeline.evaluateWithEffects(0, 0.5, Number.NaN));
assert.equal(scoreFrame[CameraFrameWord.Available], 0);

const clock = new CassiopeiaPerformanceClock(0n, 0n);
clock.resume(0n);
assert.equal(clock.timeMicros(1_000_000n), 1_000_000n);
clock.setRate(1_000_000n, 2_000_000);
assert.equal(clock.timeMicros(1_500_000n), 2_000_000n);
clock.pause(1_500_000n);
assert.equal(clock.timeMicros(9_000_000n), 2_000_000n);

assert.equal(resolveComboCutInMilestone(true, 100, 1, 200, 1_000, 0), 100n);
assert.equal(resolveComboCutInMilestone(false, 100, 1, 200, 1_000, 0), -1n);
assert.equal(resolveComboCutInMilestone(true, 100, 1, 995, 1_000, 5), -1n);

const comboCutIn = new CassiopeiaComboCutInSequencer(
  0,
  ComboCutInSource.Fixed,
  ComboCutInRole.Call,
  101n,
  1,
  1_001n,
  ComboCutInRole.Response,
  202n,
  4,
  2_002n,
);
const comboCutInPointer = comboCutIn.eventBufferPtr();
assert.equal(comboCutIn.eventBufferLen(), 72);
assert.equal(comboCutIn.eventBufferStride(), 18);
const comboCutInEvents = new BigInt64Array(
  cassiopeiaMemory().buffer,
  comboCutInPointer,
  comboCutIn.eventBufferLen(),
);
const comboCutInEvent = (index) =>
  comboCutInEvents.subarray(
    index * comboCutIn.eventBufferStride(),
    (index + 1) * comboCutIn.eventBufferStride(),
  );

assert.equal(comboCutIn.observeFrame(true, 100, 1, 200, 1_000, true), 2);
assert.equal(comboCutIn.eventBufferPtr(), comboCutInPointer);
assert.equal(comboCutIn.eventCount(), 2);
const comboStart = comboCutInEvent(0);
assert.equal(comboStart[ComboCutInEventWord.AbiVersion], 1n);
assert.equal(comboStart[ComboCutInEventWord.Kind], BigInt(ComboCutInEventKind.Started));
assert.equal(comboStart[ComboCutInEventWord.SequenceId], 1n);
assert.equal(comboStart[ComboCutInEventWord.MilestoneCombo], 100n);
assert.equal(comboStart[ComboCutInEventWord.DurationMicros], 4_000_000n);
assert.equal(comboStart[ComboCutInEventWord.ActorCharacterId], 101n);
assert.equal(comboStart[ComboCutInEventWord.PartnerCharacterId], 202n);
const firstComboCue = comboCutInEvent(1);
assert.equal(
  firstComboCue[ComboCutInEventWord.Kind],
  BigInt(ComboCutInEventKind.CueDispatched),
);
assert.equal(firstComboCue[ComboCutInEventWord.CutInIndex], 0n);
assert.equal(firstComboCue[ComboCutInEventWord.DispatchedAtMicros], 0n);

assert.equal(comboCutIn.advance(1_500_000n, true), 0);
assert.equal(comboCutIn.hasPendingCue(), true);
assert.ok(comboCutInEvents.every((word) => word === -1n));
assert.equal(comboCutIn.advance(2_500_000n, true), 1);
const comboEnd = comboCutInEvent(0);
assert.equal(comboEnd[ComboCutInEventWord.Kind], BigInt(ComboCutInEventKind.Ended));
assert.equal(
  comboEnd[ComboCutInEventWord.EndReason],
  BigInt(ComboCutInEndReason.Completed),
);
assert.equal(comboCutIn.isActive(), false);
assert.equal(comboCutIn.hasPendingCue(), true);

assert.equal(comboCutIn.advance(0n, false), 1);
const lateComboCue = comboCutInEvent(0);
assert.equal(lateComboCue[ComboCutInEventWord.CutInIndex], 1n);
assert.equal(lateComboCue[ComboCutInEventWord.AuthoredAtMicros], 1_500_000n);
assert.equal(lateComboCue[ComboCutInEventWord.DispatchedAtMicros], 4_000_000n);
assert.equal(lateComboCue[ComboCutInEventWord.ActorCharacterId], 202n);

assert.equal(comboCutIn.observeFrame(true, 200, 1, 300, 1_000, false), 2);
assert.equal(comboCutIn.advance(1_000_000n, false), 0);
comboCutIn.pause();
assert.equal(comboCutIn.advance(9_000_000n, false), 0);
assert.equal(comboCutIn.isActive(), true);
comboCutIn.resume();
assert.equal(comboCutIn.advance(500_000n, false), 1);

assert.equal(comboCutIn.observeFrame(true, 300, 1, 400, 1_000, false), 3);
assert.equal(comboCutIn.advance(1_500_000n, true), 0);
assert.equal(comboCutIn.hasPendingCue(), true);
assert.equal(comboCutIn.cancel(), 1);
assert.equal(
  comboCutInEvent(0)[ComboCutInEventWord.EndReason],
  BigInt(ComboCutInEndReason.Cancelled),
);
assert.equal(comboCutIn.hasPendingCue(), false);
assert.equal(comboCutIn.advance(0n, false), 0);
assert.equal(comboCutIn.eventBufferPtr(), comboCutInPointer);

comboCutIn.free();
clock.free();
chorusTimeline.free();
timeline.free();
console.log(
  "WASM boundary: package export, resolver, clock, camera, chorus, and combo cut-in buffers passed",
);
