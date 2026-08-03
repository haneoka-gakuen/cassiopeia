import assert from "node:assert/strict";
import {
  DEFAULT_TITLE_INTRODUCTION_TIMING,
  TitleIntroductionPresentation,
  sampleTitleIntroduction,
} from "../dist/index.js";
import {
  DEFAULT_FINISH_DIRECTION_DURATION_MS,
  PlayerFinishDirectionLifecycle,
  PlayerIntroductionHandoff,
  PlayerIntroductionLifecycle,
  PlayerPlaybackGate,
  shouldStartPlayerIntroduction,
} from "../dist/player.js";

const content = {
  title: "Starlight",
  artist: "Cassiopeia",
  lyricist: "Lyricist",
  composer: "Composer",
  arranger: "Arranger",
};
const timing = DEFAULT_TITLE_INTRODUCTION_TIMING;
const epsilon = 0.0001;

const at = (elapsedMs) => sampleTitleIntroduction(content, elapsedMs);

assert.deepEqual(at(0), {
  enabled: true,
  state: "hidden",
  alpha: 0,
  rootAlpha: 0,
  centerAlpha: 0,
  simpleAlpha: 0,
  leftAlpha: 0,
  rightAlpha: 0,
  contentAlpha: 0,
  elapsedMs: 0,
  content,
});
assert.equal(at(timing.displayStartMs - epsilon).state, "hidden");
assert.equal(at(timing.displayStartMs).state, "showing");
assert.equal(at(timing.displayStartMs).alpha, 0);
assert.equal(
  at((timing.displayStartMs + timing.holdStartMs) / 2).alpha,
  0.5,
);
assert.equal(at(timing.holdStartMs - epsilon).state, "showing");
assert.ok(at(timing.holdStartMs - epsilon).alpha > 0.99);
assert.equal(at(timing.holdStartMs).state, "holding");
assert.equal(at(timing.holdStartMs).alpha, 1);
assert.equal(at(timing.holdStartMs).contentAlpha, 0);
assert.equal(at((timing.holdStartMs + timing.contentShowEndMs) / 2).contentAlpha, 0.5);
assert.equal(at(timing.contentShowEndMs).contentAlpha, 1);
assert.equal(at(timing.normalShowStartMs).leftAlpha, 0);
assert.equal(
  at((timing.normalShowStartMs + timing.normalShowEndMs) / 2).leftAlpha,
  0.5,
);
assert.equal(at(timing.normalShowEndMs).leftAlpha, 1);
assert.equal(at(timing.normalShowEndMs).rightAlpha, 1);
assert.equal(at(timing.normalShowEndMs).centerAlpha, 1);
assert.equal(at(timing.normalShowEndMs).simpleAlpha, 1);
assert.equal(at(timing.showClipEndMs - epsilon).state, "holding");
assert.equal(at(timing.showClipEndMs).state, "hiding");
assert.equal(at(timing.showClipEndMs).alpha, 1);
assert.equal(at(timing.hideEndMs - epsilon).state, "hiding");
assert.ok(at(timing.hideEndMs - epsilon).alpha < 0.001);
assert.equal(at(timing.hideEndMs).state, "hidden");
assert.equal(at(timing.totalDurationMs - epsilon).state, "hidden");
assert.equal(at(timing.totalDurationMs).state, "complete");
assert.equal(at(timing.totalDurationMs + 1000).elapsedMs, timing.totalDurationMs);

const disabled = sampleTitleIntroduction(content, timing.displayStartMs, false);
assert.equal(disabled.state, "complete");
assert.equal(disabled.alpha, 0);
assert.equal(disabled.contentAlpha, 0);

const presentation = new TitleIntroductionPresentation({ content });
assert.equal(presentation.update(500).state, "hidden");
presentation.start(10_000);
assert.equal(presentation.update(10_000 + timing.showClipEndMs).state, "hiding");
assert.equal(presentation.reset().state, "hidden");
assert.equal(presentation.update(50_000).elapsedMs, 0);
assert.equal(presentation.retry().state, "complete");
assert.equal(presentation.setEnabled(false).state, "complete");
assert.equal(presentation.update(20_000 + timing.holdStartMs).alpha, 0);
assert.equal(presentation.setEnabled(true).state, "hidden");

assert.equal(shouldStartPlayerIntroduction(false, true, true, false), true);
assert.equal(shouldStartPlayerIntroduction(true, true, true, false), true);
assert.equal(shouldStartPlayerIntroduction(false, false, true, false), false);
assert.equal(shouldStartPlayerIntroduction(false, true, false, false), false);
assert.equal(shouldStartPlayerIntroduction(false, true, true, true), false);

const introductionLifecycle = new PlayerIntroductionLifecycle();
assert.deepEqual(introductionLifecycle.start(1_000), {
  started: true,
  timeSeconds: 0,
});
introductionLifecycle.update(1_000 + introductionLifecycle.durationMs);
assert.equal(introductionLifecycle.complete, true);
introductionLifecycle.reset();
assert.equal(introductionLifecycle.complete, false);
assert.deepEqual(introductionLifecycle.start(10_000), {
  started: true,
  timeSeconds: 0,
});

const playbackGate = new PlayerPlaybackGate();
const handoff = new PlayerIntroductionHandoff();
assert.equal(handoff.phase, "idle");
handoff.request();
assert.equal(handoff.phase, "update-frame");
assert.equal(handoff.afterAnimationFrame(), false);
assert.equal(handoff.phase, "next-frame");
assert.equal(handoff.afterAnimationFrame(), true);
assert.equal(handoff.phase, "idle");
handoff.request();
handoff.cancel();
assert.equal(handoff.afterAnimationFrame(), false);

const cancelledBetweenFramesGeneration = playbackGate.begin();
handoff.request();
assert.equal(handoff.afterAnimationFrame(), false);
assert.equal(handoff.phase, "next-frame");
playbackGate.cancel();
handoff.cancel();
assert.equal(handoff.afterAnimationFrame(), false);
assert.equal(playbackGate.beginHandoff(cancelledBetweenFramesGeneration), false);

const staleGeneration = playbackGate.begin();
assert.equal(playbackGate.playbackRequested, true);
assert.equal(playbackGate.beginHandoff(staleGeneration), true);
assert.equal(playbackGate.handoffPending, true);
let releaseHandoff;
const deferredHandoff = new Promise((resolve) => {
  releaseHandoff = resolve;
});
let lateMediaStarts = 0;
const staleContinuation = (async () => {
  await deferredHandoff;
  if (!playbackGate.isCurrent(staleGeneration)) return;
  lateMediaStarts += 1;
  playbackGate.finish(staleGeneration);
})();
playbackGate.cancel();
assert.equal(playbackGate.handoffPending, false);
assert.equal(playbackGate.playbackRequested, false);
releaseHandoff();
await staleContinuation;
assert.equal(lateMediaStarts, 0);

const currentGeneration = playbackGate.begin();
assert.notEqual(currentGeneration, staleGeneration);
assert.equal(playbackGate.beginHandoff(staleGeneration), false);
assert.equal(playbackGate.beginHandoff(currentGeneration), true);
assert.equal(playbackGate.finish(currentGeneration), true);
assert.equal(playbackGate.handoffPending, false);
assert.equal(playbackGate.playbackRequested, true);

let customSamplerCalls = 0;
const custom = new TitleIntroductionPresentation({
  content,
  alphaSampler(sample) {
    customSamplerCalls += 1;
    return sample.state === "holding" ? 0.25 : 0;
  },
});
assert.equal(custom.atElapsed(timing.holdStartMs).alpha, 0.25);
assert.equal(customSamplerCalls, 1);

assert.throws(
  () =>
    new TitleIntroductionPresentation({
      content,
      timing: { ...timing, hideEndMs: timing.showClipEndMs },
    }),
  RangeError,
);

const finishDirection = new PlayerFinishDirectionLifecycle();
assert.equal(finishDirection.durationMs, DEFAULT_FINISH_DIRECTION_DURATION_MS);
assert.deepEqual(finishDirection.start(10_000), {
  started: true,
  timeSeconds: 0,
});
assert.equal(finishDirection.running, true);
assert.equal(finishDirection.pending, true);
assert.deepEqual(finishDirection.update(11_250), { timeSeconds: 1.25 });
assert.deepEqual(finishDirection.pause(11_500), { timeSeconds: 1.5 });
assert.equal(finishDirection.running, false);
assert.equal(finishDirection.elapsedMs, 1_500);
assert.deepEqual(finishDirection.update(50_000), {});
assert.deepEqual(finishDirection.start(20_000), {});
assert.deepEqual(finishDirection.update(21_499), { timeSeconds: 2.999 });
assert.deepEqual(finishDirection.update(21_500), {
  timeSeconds: 3,
  completed: true,
});
assert.equal(finishDirection.running, false);
assert.equal(finishDirection.pending, false);
assert.equal(finishDirection.complete, true);
assert.deepEqual(finishDirection.start(30_000), {});
assert.deepEqual(finishDirection.reset(), {});
assert.equal(finishDirection.elapsedMs, 0);
assert.equal(finishDirection.complete, false);

assert.deepEqual(finishDirection.start(40_000), {
  started: true,
  timeSeconds: 0,
});
assert.deepEqual(finishDirection.reset(), { cancelled: true });
assert.equal(finishDirection.running, false);
assert.equal(finishDirection.pending, false);
assert.throws(() => finishDirection.start(Number.NaN), RangeError);
assert.throws(() => finishDirection.update(Number.POSITIVE_INFINITY), RangeError);

console.log(
  "Title introduction and finish direction: boundary, pause, reset, disable, and sampler checks passed",
);
