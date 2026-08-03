export type PlayerIntroductionHandoffPhase =
  | "idle"
  | "update-frame"
  | "next-frame";

/**
 * Preserves the authored boundary between the completed opening timeline and
 * music start: publish one UpdateFrame(0), then cross one browser frame before
 * releasing playback. The generation gate remains responsible for async
 * cancellation after release.
 */
export class PlayerIntroductionHandoff {
  private currentPhase: PlayerIntroductionHandoffPhase = "idle";

  get phase(): PlayerIntroductionHandoffPhase {
    return this.currentPhase;
  }

  get pending(): boolean {
    return this.currentPhase !== "idle";
  }

  request(): void {
    if (this.currentPhase === "idle") this.currentPhase = "update-frame";
  }

  /** Call once at the end of each animation-frame callback. */
  afterAnimationFrame(): boolean {
    if (this.currentPhase === "update-frame") {
      this.currentPhase = "next-frame";
      return false;
    }
    if (this.currentPhase === "next-frame") {
      this.currentPhase = "idle";
      return true;
    }
    return false;
  }

  cancel(): void {
    this.currentPhase = "idle";
  }
}
