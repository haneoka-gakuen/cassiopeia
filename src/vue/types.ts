import type { ChartCallChangeEvent, ChartFeverTransitionEvent, ChartSkillEvent, JudgementEvent } from "../core/types";
import type { ChartPerfSummary } from "../render/PerfProbe";

export interface ChartPlayerExpose {
  /**
   * Starts or resumes this performance.
   *
   * With an external clock, the player runs the optional title introduction
   * locally, then emits `external-playback-requested`. The owner must begin
   * advancing `externalTimeMs` and set `externalPlaying`; no wall clock is
   * guessed and owner-controlled time is never mutated by the player.
   */
  play(): Promise<void>;
  pause(): void;
  /**
   * Consumes a pending title introduction before seeking. With an external
   * clock, the owner remains responsible for changing `externalTimeMs`.
   */
  seek(seconds: number): void;
  /**
   * Starts a distinct performance from the beginning.
   *
   * With an external clock, the owner must move `externalTimeMs` to its
   * beginning in the same update; the player cannot mutate owner state.
   */
  restart(): void;
  resize(): void;
}

export interface ChartPlayerEvents {
  ready: [];
  playing: [value: boolean];
  /** True only while chart media advances; title-introduction playback stays false. */
  "media-playing": [value: boolean];
  "introduction-started": [];
  "introduction-timeupdate": [seconds: number];
  "introduction-completed": [];
  /**
   * The locally clocked opening has released an owner-controlled transport.
   * The owner should start advancing `externalTimeMs` from this presentation
   * time and set `externalPlaying`; the player never mutates owner state.
   */
  "external-playback-requested": [presentationTimeMs: number];
  "finish-direction-started": [];
  "finish-direction-timeupdate": [seconds: number];
  "finish-direction-completed": [];
  "finish-direction-cancelled": [];
  /**
   * Allocation-free presentation clock emitted after every rendered frame.
   * Times use milliseconds; `performanceEpoch` changes only for a distinct
   * performance, never for transport seeks or visual refreshes. Combo deltas
   * aggregate all judgements handled since the preceding rendered frame.
   */
  frame: [
    presentationTimeMs: number,
    chartTimeMs: number,
    performanceEpoch: number,
    combo: number,
    processed: number,
    total: number,
    comboUpdated: boolean,
    addedCombo: number,
  ];
  timeupdate: [seconds: number];
  duration: [seconds: number];
  judgement: [event: JudgementEvent];
  skill: [event: ChartSkillEvent];
  fever: [event: ChartFeverTransitionEvent];
  callchange: [event: ChartCallChangeEvent];
  error: [error: Error];
  perf: [summary: ChartPerfSummary];
}
