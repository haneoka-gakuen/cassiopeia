import assert from "node:assert/strict";
import { resolveTitleIntroductionLayout, sampleJudgementPunchScale } from "../dist/index.js";

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
});
assert.equal(resolveTitleIntroductionLayout(1920, 1440).ribbonCenterY, 1201);

console.log("HUD presentation: judgement punch and title layout checks passed");
