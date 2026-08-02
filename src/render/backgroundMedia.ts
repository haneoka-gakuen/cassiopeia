export type BackgroundMediaSource = "video" | "canvas" | "static" | "none";

/**
 * Pick the single source that owns the stage background. Keeping this policy
 * independent from Vue and Three.js makes source changes deterministic while
 * video loading and canvas production happen asynchronously.
 */
export function selectBackgroundMediaSource(
  videoAvailable: boolean,
  canvasAvailable: boolean,
  staticAvailable: boolean,
): BackgroundMediaSource {
  if (videoAvailable) return "video";
  if (canvasAvailable) return "canvas";
  if (staticAvailable) return "static";
  return "none";
}

interface DynamicCanvasTexture {
  needsUpdate: boolean;
}

interface CanvasDimensions {
  readonly width: number;
  readonly height: number;
}

/** Unversioned producers preserve the legacy per-render upload behavior. */
export function dynamicCanvasFrameChanged(
  producerVersion: number | undefined,
  uploadedVersion: number | undefined,
): boolean {
  return producerVersion === undefined || producerVersion !== uploadedVersion;
}

/** Mark one produced canvas frame for upload and report a backing-size change. */
export function refreshDynamicCanvasTexture(
  texture: DynamicCanvasTexture,
  canvas: CanvasDimensions,
  previousWidth: number,
  previousHeight: number,
): boolean {
  texture.needsUpdate = true;
  return canvas.width !== previousWidth || canvas.height !== previousHeight;
}
