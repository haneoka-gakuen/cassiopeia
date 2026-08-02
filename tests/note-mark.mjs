import assert from "node:assert/strict";
import { MeshBasicMaterial, Texture } from "three";

import {
  DEFAULT_RENDER_SETTINGS,
  NoteLayer,
  OurNotesRenderer,
  StageProjector,
  resolveEaseNoteMarkPresentation,
} from "../dist/index.js";

assert.equal(DEFAULT_RENDER_SETTINGS.showEaseNote, false);
const rendererToggles = [];
const rendererHarness = {
  disposed: false,
  notes: { setShowEaseNote: (enabled) => rendererToggles.push(enabled) },
};
OurNotesRenderer.prototype.setShowEaseNote.call(rendererHarness);
OurNotesRenderer.prototype.setShowEaseNote.call(rendererHarness, true);
rendererHarness.disposed = true;
OurNotesRenderer.prototype.setShowEaseNote.call(rendererHarness, false);
assert.deepEqual(rendererToggles, [false, true]);

const markedKinds = [
  "tap",
  "flick",
  "flick-left",
  "flick-right",
  "slide-start",
  "slide-node",
  "trace",
  "guide",
];
for (const kind of markedKinds) {
  assert.deepEqual(resolveEaseNoteMarkPresentation(kind, true, true), {
    hasMark: true,
    emphasized: true,
    scaleMultiplier: 2,
    color: 0x000000,
  });
}
assert.deepEqual(resolveEaseNoteMarkPresentation("slide-end", true, true), {
  hasMark: false,
  emphasized: false,
  scaleMultiplier: 1,
  color: 0xffffff,
});
assert.equal(resolveEaseNoteMarkPresentation("tap", true, false).emphasized, false);
assert.equal(resolveEaseNoteMarkPresentation("tap", false, true).emphasized, false);

const texture = new Texture();
const region = {
  name: "fixture",
  rect: { x: 0, y: 0, width: 100, height: 50 },
  sourceSize: { width: 100, height: 50 },
  offset: { x: 0, y: 0 },
  pivot: { x: 0.5, y: 0.5 },
  border: { left: 10, bottom: 0, right: 10, top: 0 },
  pixelsToUnits: 100,
  packingRotation: 0,
};
const atlas = {
  texture,
  region(name) {
    return { ...region, name };
  },
  regionUvTransform() {
    return { a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 };
  },
  createMaterial() {
    return new MeshBasicMaterial({ map: texture, color: 0xffffff, transparent: true });
  },
  releaseMaterial(material) {
    material.dispose();
  },
};
const assets = {
  tiltThresholds: [0, 2, 4, 6, 8, 10, 12].map((distance, value) => ({ distance, value })),
};
const layer = new NoteLayer(new StageProjector(), assets);
layer.setAtlas(atlas);

const renderNote = (id, kind, critical) => ({
  id,
  kind,
  lane: 10,
  width: 4,
  approach: 0,
  critical,
});
const currentRoot = () => layer.group.children[0];
const currentMark = () => currentRoot().getObjectByName("note-mark");

// The option is explicitly off by default, including for a critical note.
layer.update([renderNote("tap-a", "tap", true)]);
const tapRoot = currentRoot();
const tapMark = currentMark();
assert.ok(tapMark);
assert.deepEqual(tapMark.scale.toArray(), [1, 0.5, 1]);
assert.equal(tapMark.material.color.getHex(), 0xffffff);

// Changing critical state does not change the descriptor or allocate a view.
layer.setShowEaseNote(true);
layer.update([renderNote("tap-a", "tap", true)]);
assert.equal(currentRoot(), tapRoot);
assert.deepEqual(tapMark.scale.toArray(), [2, 1, 2]);
assert.equal(tapMark.material.color.getHex(), 0x000000);
assert.equal(layer.stats.visualAllocations, 1);

layer.update([renderNote("tap-a", "tap", false)]);
assert.equal(currentRoot(), tapRoot);
assert.deepEqual(tapMark.scale.toArray(), [1, 0.5, 1]);
assert.equal(tapMark.material.color.getHex(), 0xffffff);
assert.equal(layer.stats.visualAllocations, 1);

// A pooled critical mark must be restored before the reused view is shown.
layer.update([]);
layer.update([renderNote("tap-b", "tap", true)]);
assert.equal(currentRoot(), tapRoot);
assert.equal(currentMark().material.color.getHex(), 0x000000);
assert.deepEqual(currentMark().scale.toArray(), [2, 1, 2]);
layer.update([]);
layer.update([renderNote("tap-c", "tap", false)]);
assert.equal(currentRoot(), tapRoot);
assert.equal(currentMark().material.color.getHex(), 0xffffff);
assert.deepEqual(currentMark().scale.toArray(), [1, 0.5, 1]);
assert.equal(layer.stats.visualReuses, 2);

// Flick arrows and note bodies are separate renderers and retain their state.
layer.update([]);
layer.update([renderNote("flick-a", "flick", false)]);
const flickRoot = currentRoot();
const flickBody = flickRoot.children[0];
const flickMark = flickRoot.getObjectByName("note-mark");
const flickArrow = flickRoot.getObjectByName("flick-arrow");
assert.ok(flickMark && flickArrow);
const bodyColor = flickBody.material.color.getHex();
const arrowColor = flickArrow.material.color.getHex();
const arrowScale = flickArrow.scale.toArray();
layer.update([renderNote("flick-a", "flick", true)]);
assert.equal(flickBody.material.color.getHex(), bodyColor);
assert.equal(flickArrow.material.color.getHex(), arrowColor);
assert.deepEqual(flickArrow.scale.toArray(), arrowScale);
assert.equal(flickMark.material.color.getHex(), 0x000000);

// Slide-end has no mark renderer; critical presentation leaves its body alone.
layer.update([]);
layer.update([renderNote("end-a", "slide-end", false)]);
const slideEndRoot = currentRoot();
const slideEndBody = slideEndRoot.children[0];
assert.equal(slideEndRoot.getObjectByName("note-mark"), undefined);
const slideEndBodyColor = slideEndBody.material.color.getHex();
layer.update([renderNote("end-a", "slide-end", true)]);
assert.equal(slideEndRoot.getObjectByName("note-mark"), undefined);
assert.equal(slideEndBody.material.color.getHex(), slideEndBodyColor);

layer.dispose();
texture.dispose();

console.log("ease-note mark presentation and pooling tests passed");
