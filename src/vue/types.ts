import type { ChartCallChangeEvent, ChartFeverTransitionEvent, ChartSkillEvent, JudgementEvent } from "../core/types";
import type { ChartPerfSummary } from "../render/PerfProbe";

export interface ChartPlayerExpose {
  play(): Promise<void>;
  pause(): void;
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
