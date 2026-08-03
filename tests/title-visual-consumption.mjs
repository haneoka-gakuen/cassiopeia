import assert from "node:assert/strict";
import { HudLayer } from "../dist/index.js";

const requestedImages = [];

class FakeImage {
  decoding = "";
  onerror;
  onload;
}

Object.defineProperty(FakeImage.prototype, "src", {
  configurable: true,
  get() {
    return this._src ?? "";
  },
  set(value) {
    this._src = value;
    if (value) requestedImages.push(value);
    queueMicrotask(() => this.onload?.());
  },
});

function fakeContext(draws) {
  return {
    fillStyle: "",
    font: "",
    globalAlpha: 1,
    imageSmoothingEnabled: true,
    lineJoin: "round",
    lineWidth: 1,
    strokeStyle: "",
    textAlign: "center",
    textBaseline: "middle",
    clearRect() {},
    createLinearGradient() {
      return { addColorStop() {} };
    },
    drawImage(image, ...coordinates) {
      draws.push([image.src, ...coordinates]);
    },
    fillRect() {},
    fillText() {},
    measureText(text) {
      return { width: text.length * 12 };
    },
    restore() {},
    save() {},
    scale() {},
    setTransform() {},
    strokeRect() {},
  };
}

const draws = [];
const context = fakeContext(draws);
const scratchContext = fakeContext([]);
const canvas = {
  clientHeight: 1080,
  clientWidth: 1920,
  height: 1080,
  style: {},
  width: 1920,
  getContext: () => context,
};

globalThis.Image = FakeImage;
globalThis.document = {
  createElement: () => ({
    height: 1,
    width: 1,
    getContext: () => scratchContext,
  }),
};

const layer = new HudLayer(canvas, {});
layer.resize(1920, 1080, 1);
const state = {
  titleIntroduction: {
    alpha: 1,
    centerAlpha: 1,
    leftAlpha: 1,
    rightAlpha: 1,
    layoutMode: "normal",
    title: "Title",
    highScore: 123456,
    level: 27,
    gekisou: {
      enabled: true,
      missions: [
        { kind: "combo", label: "100" },
        { kind: "luck", label: "80" },
        { kind: "just", label: "20" },
      ],
    },
    theme: {
      assets: {
        activeRibbonUrl: "active.png",
        gekisouMissionUrls: {
          combo: "mission-combo.png",
          just: "mission-just.png",
          luck: "mission-luck.png",
        },
        gekisouTitleUrl: "gekisou-title.png",
        normalHighScoreBaseAlpha: 0.1921568661928177,
        normalHighScoreBaseUrl: "high-score-base.png",
        panelRibbonUrl: "panel.png",
      },
    },
  },
};

layer.draw(state);
await new Promise((resolve) => setImmediate(resolve));
draws.length = 0;
layer.draw(state);

const drawFor = (url) => draws.find(([source]) => source === url)?.slice(1);
assert.deepEqual(requestedImages.sort(), [
  "gekisou-title.png",
  "high-score-base.png",
  "mission-combo.png",
  "mission-just.png",
  "mission-luck.png",
  "panel.png",
]);
assert.deepEqual(drawFor("panel.png"), [277, 745, 1366, 192]);
assert.deepEqual(drawFor("high-score-base.png"), [221, 142, 289, 38]);
assert.deepEqual(
  drawFor("gekisou-title.png"),
  [
    1511.1999993026257, 26.399999529123306, 93.60000139474869,
    63.20000094175339,
  ],
);
assert.equal(drawFor("mission-combo.png")?.[2], 93);
assert.equal(drawFor("mission-luck.png")?.[2], 93);
assert.equal(drawFor("mission-just.png")?.[2], 93);

state.titleIntroduction.ribbonVariant = "active";
draws.length = 0;
layer.draw(state);
await new Promise((resolve) => setImmediate(resolve));
draws.length = 0;
layer.draw(state);
assert.deepEqual(drawFor("active.png"), [277, 730, 1366, 222]);

layer.dispose();
console.log(
  "title visual consumption: authored ribbon, normal, and Gekisou sprites passed",
);
