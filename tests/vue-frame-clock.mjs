import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const playerUrl = new URL("../src/vue/ChartPlayer.vue", import.meta.url);
const typesUrl = new URL("../src/vue/types.ts", import.meta.url);
const [player, types] = await Promise.all([
  readFile(playerUrl, "utf8"),
  readFile(typesUrl, "utf8"),
]);

function functionBody(source, name) {
  const signature = `function ${name}(`;
  const start = source.indexOf(signature);
  assert.notEqual(start, -1, `${name} must exist`);
  const open = source.indexOf("{", start);
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] !== "}") continue;
    depth -= 1;
    if (depth === 0) return source.slice(open + 1, index);
  }
  assert.fail(`${name} must have a complete body`);
}

const renderFrame = functionBody(player, "renderFrame");
assert.match(
  renderFrame,
  /emit\(\s*"frame",\s*framePresentationTimeMs,\s*timeMs,\s*performanceEpoch,\s*snapshot\.combo,\s*snapshot\.processed,\s*snapshot\.total,\s*frameComboUpdated,\s*frameAddedCombo,/s,
  "every completed render must publish its clocks, epoch, and aggregated session state",
);
assert.ok(
  renderFrame.indexOf('"frame",') > renderFrame.lastIndexOf("renderer.render("),
  "the high-frequency clock describes a frame that was actually rendered",
);
assert.match(
  renderFrame,
  /Math\.abs\(timeSeconds - lastTimeEmit\) >= 0\.03/,
  "the ordinary UI timeupdate must remain throttled",
);

const restart = functionBody(player, "restart");
assert.match(restart, /beginPerformanceEpoch\(\)/);
assert.match(restart, /seek\(0\)/);
for (const transportOrVisualOperation of ["seek", "pause", "resize"]) {
  assert.doesNotMatch(
    functionBody(player, transportOrVisualOperation),
    /beginPerformanceEpoch\(\)/,
    `${transportOrVisualOperation} must preserve the current performance epoch`,
  );
}
assert.match(
  functionBody(player, "play"),
  /timelineFinished && finishDirectionLifecycle\.complete\) restart\(\)/,
  "replaying a completed performance must begin a new epoch",
);

assert.match(types, /restart\(\): void/);
assert.match(
  types,
  /frame:\s*\[\s*presentationTimeMs: number,\s*chartTimeMs: number,\s*performanceEpoch: number,\s*combo: number,\s*processed: number,\s*total: number,\s*comboUpdated: boolean,\s*addedCombo: number,?\s*\]/s,
  "the public frame event must retain its positional millisecond ABI",
);

const attachSession = functionBody(player, "attachSession");
assert.match(attachSession, /incrementsCombo\(event\.judgement\)/);
assert.match(attachSession, /frameAddedCombo \+= 1/);
assert.match(
  attachSession,
  /breaksCombo\(event\.judgement\)[\s\S]*frameAddedCombo = 0/,
  "a combo break discards additions from the preceding combo in the same frame",
);
assert.match(
  renderFrame,
  /emit\([\s\S]*frameComboUpdated = false;\s*frameAddedCombo = 0;/,
  "the per-frame aggregate must be cleared only after its single frame event",
);

console.log("vue frame clock contract passed");
