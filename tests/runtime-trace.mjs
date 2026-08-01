import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  ChartSession,
  MusicTimeAnchor,
  MusicSyncTimeCache,
  normalizeEventRealtimeMs,
  NoteSimulateJudgement,
  OurNotesInput,
} from "../dist/index.js";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const fixtureDirectory = join(root, "fixtures", "runtime");
const capture = process.argv.includes("--capture");
const requested = process.argv.slice(2).filter((argument) => argument !== "--capture");

assert.deepStrictEqual(
  {
    None: NoteSimulateJudgement.None,
    Wait: NoteSimulateJudgement.Wait,
    Miss: NoteSimulateJudgement.Miss,
    Bad: NoteSimulateJudgement.Bad,
    Good: NoteSimulateJudgement.Good,
    Great: NoteSimulateJudgement.Great,
    Perfect: NoteSimulateJudgement.Perfect,
    Just: NoteSimulateJudgement.Just,
    Pass: NoteSimulateJudgement.Pass,
  },
  { None: -1, Wait: 0, Miss: 1, Bad: 2, Good: 3, Great: 4, Perfect: 5, Just: 6, Pass: 7 },
  "NoteSimulateJudgement numeric contract changed",
);

function normalizeJudgement(event) {
  return {
    noteId: event.note.id,
    judgement: event.judgement,
    timing: event.timing,
    diffMs: event.diffMs,
    judgedAtMs: event.judgedAtMs,
    combo: event.combo,
    maxCombo: event.maxCombo,
    score: event.score,
    scoreDelta: event.scoreDelta,
    life: event.life,
  };
}

function normalizeSnapshot(snapshot) {
  return {
    timeMs: snapshot.timeMs,
    durationMs: snapshot.durationMs,
    activeLongLine: snapshot.activeLongLine,
    combo: snapshot.combo,
    fullCombo: snapshot.fullCombo ?? null,
    allPerfect: snapshot.allPerfect ?? null,
    perfectCombo: snapshot.perfectCombo,
    maxCombo: snapshot.maxCombo,
    score: snapshot.score,
    life: snapshot.life,
    processed: snapshot.processed,
    total: snapshot.total,
    lastJudgementNoteId: snapshot.lastJudgement?.note.id ?? null,
    lastSkillIndex: snapshot.lastSkill?.index ?? null,
    callChangeIndex: snapshot.callChange?.index ?? null,
    feverState: snapshot.feverState,
    feverSectionIndex: snapshot.feverSection?.index ?? null,
  };
}

function canceledPointerEvents() {
  const listeners = new Map();
  const callbacks = [];
  const element = {
    style: { touchAction: "auto" },
    addEventListener(type, listener) {
      listeners.set(type, listener);
    },
    removeEventListener(type) {
      listeners.delete(type);
    },
    getBoundingClientRect() {
      return { left: 0, top: 0 };
    },
    setPointerCapture() {},
  };
  const windowStub = { addEventListener() {}, removeEventListener() {} };
  const documentStub = { visibilityState: "visible", addEventListener() {}, removeEventListener() {} };
  const hadWindow = Object.hasOwn(globalThis, "window");
  const hadDocument = Object.hasOwn(globalThis, "document");
  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;
  globalThis.window = windowStub;
  globalThis.document = documentStub;
  let input;
  try {
    input = new OurNotesInput(
      element,
      {
        tap: (point) => callbacks.push({ type: "tap", timeMs: point.timeMs }),
        move: (point) => callbacks.push({ type: "move", timeMs: point.timeMs }),
        release: (point) => callbacks.push({ type: "release", timeMs: point.timeMs }),
        flick: (point) => callbacks.push({ type: "flick", timeMs: point.timeMs }),
        cancel: (pointerId) => callbacks.push({ type: "cancel", pointerId }),
      },
      {
        eventTime: (event) => event.timeStamp,
        laneAtClientPoint: () => 12,
        screenDpi: 96,
        flickDistanceCm: 0.1,
      },
    );
    const dispatch = (type, clientX, timeStamp) => {
      listeners.get(type)?.({
        type,
        pointerId: 7,
        pointerType: "touch",
        clientX,
        clientY: 0,
        timeStamp,
        cancelable: true,
        preventDefault() {},
      });
    };
    dispatch("pointerdown", 0, 10);
    // This displacement exceeds the flick threshold. Cancellation still must
    // release without taking a final movement sample.
    dispatch("pointercancel", 100, 11);
  } finally {
    input?.destroy();
    if (hadWindow) globalThis.window = previousWindow;
    else Reflect.deleteProperty(globalThis, "window");
    if (hadDocument) globalThis.document = previousDocument;
    else Reflect.deleteProperty(globalThis, "document");
  }
  return callbacks;
}

function listen(session, trace) {
  session.on("judgement", (event) => trace.events.push({ type: "judgement", ...normalizeJudgement(event) }));
  session.on("skill", (event) =>
    trace.events.push({ type: "skill", index: event.index, tick: event.tick, timeMs: event.timeMs }),
  );
  session.on("fever", (event) =>
    trace.events.push({ type: "fever", sectionIndex: event.section.index, state: event.state, timeMs: event.timeMs }),
  );
  session.on("callChange", (event) =>
    trace.events.push({
      type: "callChange",
      index: event.index,
      tick: event.tick,
      timeMs: event.timeMs,
      rhythms: [...event.rhythms],
    }),
  );
  session.on("update", () => trace.events.push({ type: "update" }));
  session.on("reset", () => trace.events.push({ type: "reset" }));
}

function perform(session, action, trace) {
  switch (action.op) {
    case "update":
      session.update(action.timeMs);
      break;
    case "tap":
      session.tap(action.lane, action.timeMs, action.pointerId);
      break;
    case "flick":
      session.flick(action.lane, action.vector, action.timeMs, action.pointerId);
      break;
    case "release":
      session.release(action.lane, action.timeMs, action.pointerId);
      break;
    case "trace":
      session.trace(action.lane, action.timeMs, action.pointerId);
      break;
    case "simulate": {
      const note = session.chart.notes.find((candidate) => candidate.id === action.noteId);
      assert.ok(note, `Runtime trace references unknown note ${String(action.noteId)}`);
      const apply = Reflect.get(session, "apply");
      assert.equal(typeof apply, "function", "ChartSession test hook apply must remain callable");
      assert.equal(apply.length, 4, "ChartSession test hook apply signature changed");
      Reflect.apply(apply, session, [
        note,
        action.judgement,
        action.timing ?? 0,
        action.diffMs ?? 0,
        action.timeMs ?? note.timeMs,
      ]);
      break;
    }
    case "clockSample":
      trace.clock.sample(action.musicTimeMs, action.realtimeMs);
      break;
    case "clockTime":
      trace.events.push({
        type: "clockTime",
        timeMs: trace.clock.timeAt(action.realtimeMs, action.fallbackMusicTimeMs),
      });
      break;
    case "normalizeRealtime":
      trace.events.push({
        type: "normalizedRealtime",
        realtimeMs: normalizeEventRealtimeMs(action.timeStamp, action.currentRealtimeMs, action.timeOriginMs),
      });
      break;
    case "cancelInput":
      trace.events.push(...canceledPointerEvents().map((event) => ({ type: "input", ...event })));
      break;
    case "syncSoundInfo":
      trace.syncTime.setSoundInfo(action.available);
      break;
    case "syncRead":
      trace.events.push({
        type: "syncTime",
        timeMs: trace.syncTime.read(action.playbackUsable, action.sampledTimeMs),
      });
      break;
    case "syncSeek":
      trace.events.push({ type: "syncTime", timeMs: trace.syncTime.seek(action.timeMs) });
      break;
    case "seek":
      session.reset(action.timeMs);
      break;
    case "reset":
      session.reset(action.timeMs ?? 0);
      break;
    case "setOffset":
      session.setOffset(action.milliseconds);
      break;
    case "cancel":
      session.cancel(action.pointerId);
      break;
    default:
      throw new TypeError(`Unsupported runtime trace action: ${String(action.op)}`);
  }
}

function validateFixture(fixture, path) {
  assert.equal(fixture.format, "RuntimeTraceV1", `${path}: unsupported format`);
  assert.equal(fixture.version, 1, `${path}: unsupported version`);
  assert.ok(Array.isArray(fixture.cases) && fixture.cases.length > 0, `${path}: cases must be a non-empty array`);
  const ids = new Set();
  for (const testCase of fixture.cases) {
    assert.equal(typeof testCase.id, "string", `${path}: every case needs an id`);
    assert.ok(!ids.has(testCase.id), `${path}: duplicate case id ${testCase.id}`);
    ids.add(testCase.id);
    assert.ok(testCase.chart && typeof testCase.chart === "object", `${path}#${testCase.id}: chart is required`);
    assert.ok(Array.isArray(testCase.steps) && testCase.steps.length > 0, `${path}#${testCase.id}: steps are required`);
  }
}

async function fixturePaths() {
  if (requested.length) return requested.map((path) => resolve(root, path));
  return (await readdir(fixtureDirectory))
    .filter((name) => name.endsWith(".json") && !name.endsWith(".schema.json"))
    .sort()
    .map((name) => join(fixtureDirectory, name));
}

let caseCount = 0;
let stepCount = 0;
for (const path of await fixturePaths()) {
  const fixture = JSON.parse(await readFile(path, "utf8"));
  validateFixture(fixture, path);
  const capturedCases = [];
  for (const testCase of fixture.cases) {
    const session = new ChartSession(testCase.chart, { mode: testCase.mode, ...testCase.options });
    const trace = { events: [], clock: new MusicTimeAnchor(), syncTime: new MusicSyncTimeCache() };
    listen(session, trace);
    const capturedSteps = [];
    for (const [index, step] of testCase.steps.entries()) {
      trace.events.length = 0;
      perform(session, step.action, trace);
      const actual = { events: [...trace.events], snapshot: normalizeSnapshot(session.snapshot()) };
      const location = `${path}#${testCase.id}/steps/${index}${step.label ? ` (${step.label})` : ""}`;
      if (!capture) assert.deepStrictEqual(actual, step.expect, location);
      capturedSteps.push({ ...step, expect: actual });
      stepCount++;
    }
    capturedCases.push({ ...testCase, steps: capturedSteps });
    caseCount++;
  }
  if (capture) process.stdout.write(`${JSON.stringify({ ...fixture, cases: capturedCases }, null, 2)}\n`);
}

if (!capture) console.log(`RuntimeTraceV1: ${caseCount} cases, ${stepCount} steps passed.`);
