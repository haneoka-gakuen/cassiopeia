import assert from "node:assert/strict";
import { sampleJudgementPunchScale } from "../dist/index.js";

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

console.log("HUD presentation: judgement punch curve checks passed");
