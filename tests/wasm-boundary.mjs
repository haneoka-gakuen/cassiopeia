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

const clock = new CassiopeiaPerformanceClock(0n, 0n);
clock.resume(0n);
assert.equal(clock.timeMicros(1_000_000n), 1_000_000n);
clock.setRate(1_000_000n, 2_000_000);
assert.equal(clock.timeMicros(1_500_000n), 2_000_000n);
clock.pause(1_500_000n);
assert.equal(clock.timeMicros(9_000_000n), 2_000_000n);

clock.free();
timeline.free();
console.log("WASM boundary: package export, resolver, clock, and camera buffer passed");
