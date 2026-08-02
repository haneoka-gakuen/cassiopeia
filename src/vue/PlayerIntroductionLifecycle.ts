import { DEFAULT_TITLE_INTRODUCTION_TIMING } from "../presentation/TitleIntroduction";

export interface PlayerIntroductionTransition {
  started?: true;
  timeSeconds?: number;
  completed?: true;
}

const EMPTY_TRANSITION: Readonly<PlayerIntroductionTransition> = Object.freeze({});

const finiteRealtime = (value: number): number => {
  if (!Number.isFinite(value)) throw new RangeError("realtimeMs must be finite");
  return value;
};

/** One-shot realtime clock for the opening direction, independent of music time. */
export class PlayerIntroductionLifecycle {
  readonly durationMs = DEFAULT_TITLE_INTRODUCTION_TIMING.totalDurationMs;

  private startRealtimeMs = 0;
  private elapsed = 0;
  private active = false;
  private consumed = false;
  private startedEmitted = false;
  private completedEmitted = false;

  get elapsedMs(): number {
    return this.elapsed;
  }

  get running(): boolean {
    return this.active;
  }

  get complete(): boolean {
    return this.consumed;
  }

  reset(): void {
    this.startRealtimeMs = 0;
    this.elapsed = 0;
    this.active = false;
    this.consumed = false;
    this.startedEmitted = false;
    this.completedEmitted = false;
  }

  start(realtimeMs: number): PlayerIntroductionTransition {
    const now = finiteRealtime(realtimeMs);
    if (this.active || this.consumed) return EMPTY_TRANSITION;
    this.startRealtimeMs = now - this.elapsed;
    this.active = true;
    if (this.startedEmitted) return EMPTY_TRANSITION;
    this.startedEmitted = true;
    return { started: true, timeSeconds: 0 };
  }

  update(realtimeMs: number): PlayerIntroductionTransition {
    const now = finiteRealtime(realtimeMs);
    if (!this.active) return EMPTY_TRANSITION;
    const previous = this.elapsed;
    this.elapsed = Math.min(
      this.durationMs,
      Math.round(Math.max(previous, now - this.startRealtimeMs) * 10_000) /
        10_000,
    );
    if (this.elapsed < this.durationMs) {
      return this.elapsed === previous
        ? EMPTY_TRANSITION
        : { timeSeconds: this.elapsed / 1000 };
    }
    this.active = false;
    this.consumed = true;
    if (this.completedEmitted) return EMPTY_TRANSITION;
    this.completedEmitted = true;
    return {
      timeSeconds: this.durationMs / 1000,
      completed: true,
    };
  }

  pause(realtimeMs: number): PlayerIntroductionTransition {
    const transition = this.update(realtimeMs);
    this.active = false;
    return transition;
  }

  skip(): PlayerIntroductionTransition {
    if (this.consumed) return EMPTY_TRANSITION;
    this.active = false;
    this.consumed = true;
    this.elapsed = this.durationMs;
    if (!this.startedEmitted || this.completedEmitted) return EMPTY_TRANSITION;
    this.completedEmitted = true;
    return {
      timeSeconds: this.durationMs / 1000,
      completed: true,
    };
  }
}
