export type TitleIntroductionState =
  | "hidden"
  | "showing"
  | "holding"
  | "hiding"
  | "complete";

export interface TitleIntroductionContent {
  title: string;
  artist: string;
  lyricist?: string;
  composer?: string;
  arranger?: string;
}

export interface TitleIntroductionTiming {
  totalDurationMs: number;
  displayStartMs: number;
  /** Root CanvasGroup reaches alpha 1 at this boundary. */
  holdStartMs: number;
  /** Center title/artist/credit group reaches alpha 1 at this boundary. */
  contentShowEndMs: number;
  showClipEndMs: number;
  hideEndMs: number;
}

export interface TitleIntroductionAlphaSample {
  state: TitleIntroductionState;
  elapsedMs: number;
  phaseElapsedMs: number;
  phaseDurationMs: number;
}

export type TitleIntroductionAlphaSampler = (sample: TitleIntroductionAlphaSample) => number;

export interface TitleIntroductionSnapshot {
  enabled: boolean;
  state: TitleIntroductionState;
  alpha: number;
  contentAlpha: number;
  elapsedMs: number;
  content: Readonly<TitleIntroductionContent>;
}

export interface TitleIntroductionOptions {
  content: TitleIntroductionContent;
  enabled?: boolean;
  timing?: TitleIntroductionTiming;
  alphaSampler?: TitleIntroductionAlphaSampler;
}

/** Stable clip boundaries from the authored 60 Hz opening timeline. */
export const DEFAULT_TITLE_INTRODUCTION_TIMING: Readonly<TitleIntroductionTiming> = Object.freeze({
  totalDurationMs: 5916.666666666667,
  displayStartMs: 1483.3333333333333,
  holdStartMs: 1650.0000049670537,
  contentShowEndMs: 1983.3333333333333,
  showClipEndMs: 3766.6666666666665,
  hideEndMs: 4433.333353201548,
});

const clampUnit = (value: number): number => {
  if (!Number.isFinite(value)) return 0;
  return Math.min(1, Math.max(0, value));
};

/**
 * Conservative piecewise alpha used until a product skin supplies its own
 * sampler. Layout, translation and scaling remain the renderer's concern.
 */
export const sampleDefaultTitleIntroductionAlpha: TitleIntroductionAlphaSampler = ({
  state,
  phaseElapsedMs,
  phaseDurationMs,
}) => {
  if (state === "holding") return 1;
  const progress = clampUnit(phaseElapsedMs / phaseDurationMs);
  const smooth = progress * progress * (3 - 2 * progress);
  if (state === "showing") return smooth;
  if (state === "hiding") return 1 - smooth;
  return 0;
};

function validateTiming(timing: TitleIntroductionTiming): TitleIntroductionTiming {
  const values = [
    timing.totalDurationMs,
    timing.displayStartMs,
    timing.holdStartMs,
    timing.contentShowEndMs,
    timing.showClipEndMs,
    timing.hideEndMs,
  ];
  if (values.some((value) => !Number.isFinite(value))) {
    throw new RangeError("Title introduction timing values must be finite");
  }
  if (
    timing.displayStartMs < 0 ||
    timing.displayStartMs >= timing.holdStartMs ||
    timing.holdStartMs >= timing.contentShowEndMs ||
    timing.contentShowEndMs > timing.showClipEndMs ||
    timing.showClipEndMs >= timing.hideEndMs ||
    timing.hideEndMs > timing.totalDurationMs
  ) {
    throw new RangeError("Title introduction timing boundaries are out of order");
  }
  return { ...timing };
}

function finiteRealtime(value: number, label: string): number {
  if (!Number.isFinite(value)) throw new RangeError(`${label} must be finite`);
  return value;
}

function resolveState(elapsedMs: number, timing: TitleIntroductionTiming): TitleIntroductionState {
  if (elapsedMs >= timing.totalDurationMs) return "complete";
  if (elapsedMs < timing.displayStartMs || elapsedMs >= timing.hideEndMs) return "hidden";
  if (elapsedMs < timing.holdStartMs) return "showing";
  if (elapsedMs < timing.showClipEndMs) return "holding";
  return "hiding";
}

function phaseBounds(
  state: TitleIntroductionState,
  timing: TitleIntroductionTiming,
): readonly [number, number] {
  if (state === "showing") return [timing.displayStartMs, timing.holdStartMs];
  if (state === "holding") return [timing.holdStartMs, timing.showClipEndMs];
  if (state === "hiding") return [timing.showClipEndMs, timing.hideEndMs];
  if (state === "complete") return [timing.totalDurationMs, timing.totalDurationMs];
  return [0, timing.displayStartMs];
}

/** Pure elapsed-time sampler for external clocks and deterministic replays. */
export function sampleTitleIntroduction(
  content: Readonly<TitleIntroductionContent>,
  elapsedMs: number,
  enabled = true,
  timing: TitleIntroductionTiming = DEFAULT_TITLE_INTRODUCTION_TIMING,
  alphaSampler: TitleIntroductionAlphaSampler = sampleDefaultTitleIntroductionAlpha,
): TitleIntroductionSnapshot {
  const checkedTiming = validateTiming(timing);
  const checkedElapsed = Math.min(
    checkedTiming.totalDurationMs,
    Math.max(0, finiteRealtime(elapsedMs, "elapsedMs")),
  );
  if (!enabled) {
    return { enabled: false, state: "complete", alpha: 0, contentAlpha: 0, elapsedMs: checkedElapsed, content };
  }

  const state = resolveState(checkedElapsed, checkedTiming);
  const [phaseStartMs, phaseEndMs] = phaseBounds(state, checkedTiming);
  const alpha = clampUnit(
    alphaSampler({
      state,
      elapsedMs: checkedElapsed,
      phaseElapsedMs: Math.max(0, checkedElapsed - phaseStartMs),
      phaseDurationMs: Math.max(Number.EPSILON, phaseEndMs - phaseStartMs),
    }),
  );
  const contentProgress = clampUnit(
    (checkedElapsed - checkedTiming.holdStartMs) /
      Math.max(Number.EPSILON, checkedTiming.contentShowEndMs - checkedTiming.holdStartMs),
  );
  const contentSmooth = contentProgress * contentProgress * (3 - 2 * contentProgress);
  const contentAlpha = checkedElapsed >= checkedTiming.hideEndMs ? 0 : clampUnit(alpha * contentSmooth);
  return { enabled: true, state, alpha, contentAlpha, elapsedMs: checkedElapsed, content };
}

/** Realtime-driven title introduction with explicit reset and retry behavior. */
export class TitleIntroductionPresentation {
  readonly content: Readonly<TitleIntroductionContent>;
  readonly timing: Readonly<TitleIntroductionTiming>;

  private enabled: boolean;
  private startRealtimeMs: number | undefined;
  private readonly alphaSampler: TitleIntroductionAlphaSampler;

  constructor(options: TitleIntroductionOptions) {
    this.content = Object.freeze({ ...options.content });
    this.timing = Object.freeze(
      validateTiming(options.timing ?? DEFAULT_TITLE_INTRODUCTION_TIMING),
    );
    this.enabled = options.enabled ?? true;
    this.alphaSampler = options.alphaSampler ?? sampleDefaultTitleIntroductionAlpha;
  }

  start(realtimeMs: number): TitleIntroductionSnapshot {
    this.startRealtimeMs = finiteRealtime(realtimeMs, "realtimeMs");
    return this.atElapsed(0);
  }

  update(realtimeMs: number): TitleIntroductionSnapshot {
    const now = finiteRealtime(realtimeMs, "realtimeMs");
    const elapsedMs =
      this.startRealtimeMs === undefined
        ? 0
        : Math.round(Math.max(0, now - this.startRealtimeMs) * 10_000) / 10_000;
    return this.atElapsed(elapsedMs);
  }

  atElapsed(elapsedMs: number): TitleIntroductionSnapshot {
    return sampleTitleIntroduction(
      this.content,
      elapsedMs,
      this.enabled,
      this.timing,
      this.alphaSampler,
    );
  }

  setEnabled(enabled: boolean): TitleIntroductionSnapshot {
    this.enabled = enabled;
    return this.atElapsed(0);
  }

  reset(): TitleIntroductionSnapshot {
    this.startRealtimeMs = undefined;
    return this.atElapsed(0);
  }

  /** Retry bypasses the opening state and must not replay this presentation. */
  retry(): TitleIntroductionSnapshot {
    this.startRealtimeMs = undefined;
    return this.atElapsed(this.timing.totalDurationMs);
  }
}
