import assert from "node:assert/strict";

import {
  refreshDynamicCanvasTexture,
  selectBackgroundMediaSource,
} from "../dist/index.js";

assert.equal(
  selectBackgroundMediaSource(true, true, true),
  "video",
  "a playable video has priority over dynamic and static backgrounds",
);
assert.equal(
  selectBackgroundMediaSource(false, true, true),
  "canvas",
  "a dynamic canvas is the first fallback after video",
);
assert.equal(
  selectBackgroundMediaSource(false, false, true),
  "static",
  "the prepared texture remains the final visual fallback",
);
assert.equal(selectBackgroundMediaSource(false, false, false), "none");

const dynamicTexture = { needsUpdate: false };
assert.equal(
  refreshDynamicCanvasTexture(
    dynamicTexture,
    { width: 960, height: 540 },
    960,
    540,
  ),
  false,
  "content-only updates do not invalidate the cover layout",
);
assert.equal(
  dynamicTexture.needsUpdate,
  true,
  "each produced canvas frame is uploaded",
);
dynamicTexture.needsUpdate = false;
assert.equal(
  refreshDynamicCanvasTexture(
    dynamicTexture,
    { width: 1920, height: 1080 },
    960,
    540,
  ),
  true,
  "backing-size changes invalidate the cover layout",
);
assert.equal(dynamicTexture.needsUpdate, true);

console.log("background media tests passed");
