export const DEFAULT_FINISH_DIRECTION_DURATION_MS = 3_000;

export interface PlayerFinishDirectionTransition {
  started?: true;
  timeSeconds?: number;
  completed?: true;
  cancelled?: true;
}

const EMPTY_TRANSITION: Readonly<PlayerFinishDirectionTransition> = Object.freeze({});

const finiteRealtime = (value: number): number => {
  if (!Number.isFinite(value)) throw new RangeError("realtimeMs must be finite");
  return value;
};

/** Pauseable one-shot realtime clock for the post-score camera direction. */
export class PlayerFinishDirectionLifecycle {
  readonly durationMs = DEFAULT_FINISH_DIRECTION_DURATION_MS;

  private startRealtimeMs = 0;
  private elapsed = 0;
  private active = false;
  private startedEmitted = false;
  private completedEmitted = false;

  get elapsedMs(): number {
    return this.elapsed;
  }

  get running(): boolean {
    return this.active;
  }

  get pending(): boolean {
    return this.startedEmitted && !this.completedEmitted;
  }

  get complete(): boolean {
    return this.completedEmitted;
  }

  reset(): PlayerFinishDirectionTransition {
    const cancelled = this.startedEmitted && !this.completedEmitted;
    this.startRealtimeMs = 0;
    this.elapsed = 0;
    this.active = false;
    this.startedEmitted = false;
    this.completedEmitted = false;
    return cancelled ? { cancelled: true } : EMPTY_TRANSITION;
  }

  start(realtimeMs: number): PlayerFinishDirectionTransition {
    const now = finiteRealtime(realtimeMs);
    if (this.active || this.completedEmitted) return EMPTY_TRANSITION;
    this.startRealtimeMs = now - this.elapsed;
    this.active = true;
    if (this.startedEmitted) return EMPTY_TRANSITION;
    this.startedEmitted = true;
    return { started: true, timeSeconds: 0 };
  }

  update(realtimeMs: number): PlayerFinishDirectionTransition {
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
        : { timeSeconds: this.elapsed / 1_000 };
    }
    this.active = false;
    this.completedEmitted = true;
    return {
      timeSeconds: this.durationMs / 1_000,
      completed: true,
    };
  }

  pause(realtimeMs: number): PlayerFinishDirectionTransition {
    const transition = this.update(realtimeMs);
    this.active = false;
    return transition;
  }
}
