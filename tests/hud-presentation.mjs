import assert from "node:assert/strict";
import {
  resolveJudgementTimingPresentation,
  resolveTitleIntroductionLayout,
  sampleJudgementPunchScale,
} from "../dist/index.js";

const expectedAt60Fps = [
  1,
  1.1111111641,
  1.1777777672,
  1.2000000477,
  1.1388889551,
  1.0888888836,
  1.0500000715,
  1.0222222805,
  1.0055555105,
  1,
];

for (const [frame, expected] of expectedAt60Fps.entries()) {
  const actual = sampleJudgementPunchScale(frame / 60);
  assert.ok(Math.abs(actual - expected) < 0.000001, `frame ${frame}: ${actual} != ${expected}`);
}

assert.equal(sampleJudgementPunchScale(-1), 1);
assert.equal(sampleJudgementPunchScale(1), 1);

assert.deepEqual(resolveTitleIntroductionLayout(1920, 1080), {
  ribbonCenterX: 960,
  ribbonCenterY: 841,
  jacketLeft: 739.8399963378906,
  jacketTop: 232.83999633789062,
  jacketSize: 440.32000732421875,
  metadataTop: 687,
  metadataRight: 1180,
  normalJacketLeft: 71.02399826049805,
  normalJacketTop: 70.02399826049805,
  normalJacketSize: 125.9520034790039,
  normalDifficultyLeft: 221.05000180006027,
  normalDifficultyTop: 86.20000052452087,
  normalDifficultyWidth: 135.89999639987946,
  normalDifficultyHeight: 39.59999895095825,
  normalLevelCenterX: 387,
  normalLevelCenterY: 108,
  normalMetadataLeft: 221,
  normalMetadataTop: 142,
  normalMetadataRight: 510,
  gekisouPanelRight: 1871,
  gekisouPanelTop: 26,
  gekisouMissionRowLeft: 1494,
  gekisouMissionRowTop: 90,
});
assert.equal(resolveTitleIntroductionLayout(1920, 1440).ribbonCenterY, 1201);

const timingDefaults = {
  showFastSlow: true,
  showPerfectFastSlow: true,
  showJudgeOffsetMs: false,
  alwaysShowFastSlow: false,
};
const fastSprite = resolveJudgementTimingPresentation(timingDefaults, "great", "FAST", -17, 0);
assert.deepEqual(fastSprite, {
  mode: "sprite",
  timing: "fast",
  nativeWidth: 307,
  nativeHeight: 31,
  fontSize: 44,
  scale: 0.75,
  alpha: 1,
  localX: 0,
  localY: 0,
});
assert.equal(resolveJudgementTimingPresentation(timingDefaults, "perfect", "FAST", -17, 0)?.mode, "sprite");
assert.equal(
  resolveJudgementTimingPresentation({ ...timingDefaults, showPerfectFastSlow: false }, "perfect", "FAST", -17, 0),
  null,
);
assert.equal(
  resolveJudgementTimingPresentation({ ...timingDefaults, showFastSlow: false }, "great", "SLOW", 19, 0),
  null,
);
assert.equal(
  resolveJudgementTimingPresentation(
    { ...timingDefaults, showFastSlow: false, alwaysShowFastSlow: true },
    "bad",
    "SLOW",
    19,
    0,
  )?.timing,
  "late",
);

const fastMilliseconds = resolveJudgementTimingPresentation(
  { ...timingDefaults, showJudgeOffsetMs: true },
  "great",
  "FAST",
  -17.4,
  0,
);
assert.equal(fastMilliseconds?.mode, "milliseconds");
assert.equal(fastMilliseconds?.text, "17");
assert.equal(fastMilliseconds?.fontSize, 44);
assert.equal(fastMilliseconds?.color, "rgba(92.61791735887527, 193.99018496274948, 255, 1)");
assert.equal(fastMilliseconds?.localX, 0);
assert.equal(fastMilliseconds?.localY, 0);
assert.equal("sprite" in (fastMilliseconds ?? {}), false);

const lateMilliseconds = resolveJudgementTimingPresentation(
  { ...timingDefaults, showJudgeOffsetMs: true },
  "good",
  "SLOW",
  22.6,
  1 / 60,
);
assert.equal(lateMilliseconds?.text, "23");
assert.equal(lateMilliseconds?.color, "rgba(204.48113322257996, 76.19815483689308, 76.19815483689308, 1)");
assert.equal(
  lateMilliseconds?.scale,
  0.75 * sampleJudgementPunchScale(1 / 60),
  "timing sub tween must sample its own age",
);
assert.equal(resolveJudgementTimingPresentation(timingDefaults, "great", "FAST", -17, 0.30000001192092896), null);

console.log("HUD presentation: judgement punch and title layout checks passed");
