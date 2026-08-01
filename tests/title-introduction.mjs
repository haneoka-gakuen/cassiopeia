import assert from "node:assert/strict";
import {
  DEFAULT_TITLE_INTRODUCTION_TIMING,
  TitleIntroductionPresentation,
  sampleTitleIntroduction,
} from "../dist/index.js";

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
  elapsedMs: 0,
  content,
});
assert.equal(at(timing.displayStartMs - epsilon).state, "hidden");
assert.equal(at(timing.displayStartMs).state, "showing");
assert.equal(at(timing.displayStartMs).alpha, 0);
assert.equal(at(timing.holdStartMs - epsilon).state, "showing");
assert.ok(at(timing.holdStartMs - epsilon).alpha > 0.99);
assert.equal(at(timing.holdStartMs).state, "holding");
assert.equal(at(timing.holdStartMs).alpha, 1);
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

const presentation = new TitleIntroductionPresentation({ content });
assert.equal(presentation.update(500).state, "hidden");
presentation.start(10_000);
assert.equal(presentation.update(10_000 + timing.showClipEndMs).state, "hiding");
assert.equal(presentation.reset().state, "hidden");
assert.equal(presentation.update(50_000).elapsedMs, 0);
presentation.retry(20_000);
assert.equal(presentation.update(20_000 + timing.holdStartMs).state, "holding");
assert.equal(presentation.setEnabled(false).state, "complete");
assert.equal(presentation.update(20_000 + timing.holdStartMs).alpha, 0);
assert.equal(presentation.setEnabled(true).state, "hidden");

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

console.log("Title introduction: boundary, clock, reset, disable, and sampler checks passed");
