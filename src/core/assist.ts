import { JudgementAreaOffsetType, NoteJudgementType, NoteSimulateJudgement } from "./enums.js";

export const ASSIST_LEVELS = [0, 1, 2, 3, 4, 5] as const;
export type AssistLevel = (typeof ASSIST_LEVELS)[number];

export const DEFAULT_ASSIST_LEVEL: AssistLevel = 0;
export const MIN_ASSIST_LEVEL: AssistLevel = 0;
export const MAX_ASSIST_LEVEL: AssistLevel = 5;

export type AssistJudgementPriority = 1 | 2 | 3 | 4 | 5 | 6;

export interface AssistJudgeWindow {
  readonly priority: AssistJudgementPriority;
  readonly judgement: NoteSimulateJudgement;
  /** Inclusive early window, in milliseconds. */
  readonly before: number;
  /** Inclusive late window, in milliseconds. */
  readonly after: number;
}

export interface AssistJudgementAreaOffset {
  readonly x: number;
  readonly y: number;
}

export type AssistTimingTable = Readonly<Partial<Record<NoteJudgementType, readonly AssistJudgeWindow[]>>>;
export type AssistAreaOffsetTable = Readonly<Partial<Record<JudgementAreaOffsetType, AssistJudgementAreaOffset>>>;

export const SLIDE_OFFSET_MIN_NOTE_WIDTH = 4;
export const SLIDE_OFFSET_MAX_NOTE_WIDTH = 5;

const N = NoteJudgementType;
const J = NoteSimulateJudgement;
const A = JudgementAreaOffsetType;

function window(
  priority: AssistJudgementPriority,
  judgement: NoteSimulateJudgement,
  before: number,
  after: number,
): AssistJudgeWindow {
  return Object.freeze({ priority, judgement, before, after });
}

function windows(...entries: AssistJudgeWindow[]): readonly AssistJudgeWindow[] {
  return Object.freeze(entries);
}

function normalWindows(good: number, bad: number, miss: number): readonly AssistJudgeWindow[] {
  return windows(
    window(1, J.Just, 1, 1),
    window(2, J.Perfect, 42, 42),
    window(3, J.Great, 83, 83),
    window(4, J.Good, good, good),
    window(5, J.Bad, bad, bad),
    window(6, J.Miss, miss, miss),
  );
}

function flickWindows(
  perfectBefore: number,
  goodAfter: number,
  badAfter: number,
  missAfter: number,
): readonly AssistJudgeWindow[] {
  return windows(
    window(1, J.Just, 1, 1),
    window(2, J.Perfect, perfectBefore, 58),
    window(3, J.Great, 0, 83),
    window(4, J.Good, 0, goodAfter),
    window(5, J.Bad, 0, badAfter),
    window(6, J.Miss, 0, missAfter),
  );
}

function slideEndWindows(
  goodBefore: number,
  goodAfter: number,
  badBefore: number,
  badAfter: number,
  miss: number,
): readonly AssistJudgeWindow[] {
  return windows(
    window(2, J.Perfect, 42, 66),
    window(3, J.Great, 99, 166),
    window(4, J.Good, goodBefore, goodAfter),
    window(5, J.Bad, badBefore, badAfter),
    window(6, J.Miss, miss, miss),
  );
}

function easyWindows(perfectBefore: number, perfectAfter: number): readonly AssistJudgeWindow[] {
  return windows(
    window(1, J.Just, 1, 1),
    window(2, J.Perfect, perfectBefore, perfectAfter),
    window(6, J.Miss, 58, 130),
  );
}

function traceWindows(perfectBefore: number, perfectAfter: number, missAfter: number): readonly AssistJudgeWindow[] {
  return windows(window(2, J.Perfect, perfectBefore, perfectAfter), window(6, J.Miss, 58, missAfter));
}

function timingTable(
  normal: readonly AssistJudgeWindow[],
  flick: readonly AssistJudgeWindow[],
  slideEnd: readonly AssistJudgeWindow[],
  easy: readonly AssistJudgeWindow[],
  trace: readonly AssistJudgeWindow[],
  slideBegin: readonly AssistJudgeWindow[] = normal,
): AssistTimingTable {
  return Object.freeze({
    [N.Normal]: normal,
    [N.EasyNormal]: easy,
    [N.Flick]: flick,
    [N.SlideBegin]: slideBegin,
    [N.SlideEnd]: slideEnd,
    [N.SlideEndFlick]: flick,
    [N.SlideBeginEasy]: easy,
    [N.Trace]: trace,
    [N.SlideEndTrace]: trace,
  });
}

const NORMAL_0 = normalWindows(108, 125, 130);
const NORMAL_1 = normalWindows(111, 128, 133);
const NORMAL_2 = normalWindows(114, 132, 137);
const NORMAL_3 = normalWindows(118, 137, 143);
const NORMAL_4 = normalWindows(124, 143, 149);
const NORMAL_5 = normalWindows(129, 150, 156);

const FLICK_0 = flickWindows(83, 108, 125, 130);
const FLICK_1 = flickWindows(85, 111, 128, 133);
const FLICK_2 = flickWindows(87, 114, 132, 137);
const FLICK_3 = flickWindows(89, 118, 137, 143);
const FLICK_4 = flickWindows(91, 124, 143, 149);
const FLICK_5 = flickWindows(93, 129, 150, 156);

const SLIDE_END_0 = slideEndWindows(124, 191, 141, 208, 150);
const SLIDE_END_1 = slideEndWindows(128, 196, 145, 214, 154);
const SLIDE_END_2 = slideEndWindows(132, 202, 150, 220, 159);
const SLIDE_END_3 = slideEndWindows(137, 210, 155, 229, 165);
const SLIDE_END_4 = slideEndWindows(143, 219, 162, 239, 172);
const SLIDE_END_5 = slideEndWindows(149, 229, 169, 249, 180);

const EASY_0 = easyWindows(58, 66);
const EASY_1 = easyWindows(60, 68);
const EASY_2 = easyWindows(62, 66);
const EASY_3 = easyWindows(64, 66);
const EASY_4 = easyWindows(66, 66);
const EASY_5 = easyWindows(68, 66);

const TRACE_0 = traceWindows(58, 66, 130);
const TRACE_1 = traceWindows(60, 68, 133);
const TRACE_2 = traceWindows(62, 66, 137);
const TRACE_3 = traceWindows(64, 66, 143);
const TRACE_4 = traceWindows(66, 66, 149);
const TRACE_5 = traceWindows(68, 66, 156);

/** Six immutable timing profiles, indexed by {@link AssistLevel}. */
export const ASSIST_TIMING_TABLES: readonly AssistTimingTable[] = Object.freeze([
  timingTable(NORMAL_0, FLICK_0, SLIDE_END_0, EASY_0, TRACE_0),
  timingTable(NORMAL_1, FLICK_1, SLIDE_END_1, EASY_1, TRACE_1),
  timingTable(NORMAL_2, FLICK_2, SLIDE_END_2, EASY_2, TRACE_2),
  timingTable(NORMAL_3, FLICK_3, SLIDE_END_3, EASY_3, TRACE_3),
  timingTable(NORMAL_4, FLICK_4, SLIDE_END_4, EASY_4, TRACE_4),
  timingTable(NORMAL_5, FLICK_5, SLIDE_END_5, EASY_5, TRACE_5, NORMAL_0),
]);

function offset(x: number, y = 9999): AssistJudgementAreaOffset {
  return Object.freeze({ x: Math.fround(x), y: Math.fround(y) });
}

function areaOffsetTable(
  defaultX: number,
  slideBeginX: number,
  slideEndX: number,
  traceX: number,
): AssistAreaOffsetTable {
  const defaultOffset = offset(defaultX);
  const slideBeginOffset = offset(slideBeginX);
  const slideEndOffset = offset(slideEndX);
  const traceOffset = offset(traceX);
  return Object.freeze({
    [A.Default]: defaultOffset,
    [A.SlideBegin]: slideBeginOffset,
    [A.SlideEnd]: slideEndOffset,
    [A.Flick]: slideEndOffset,
    [A.Trace]: traceOffset,
    [A.SlideMin]: slideBeginOffset,
    [A.SlideMax]: defaultOffset,
    [A.EasyDefault]: traceOffset,
    [A.EasySlideBegin]: traceOffset,
  });
}

/** Six immutable hit-area profiles, indexed by {@link AssistLevel}. */
export const ASSIST_AREA_OFFSET_TABLES: readonly AssistAreaOffsetTable[] = Object.freeze([
  areaOffsetTable(1, 2, 3, 2.8),
  areaOffsetTable(1.1, 2.1, 3.1, 2.85),
  areaOffsetTable(1.2, 2.2, 3.2, 2.9),
  areaOffsetTable(1.3, 2.3, 3.3, 2.95),
  areaOffsetTable(1.4, 2.4, 3.4, 3),
  areaOffsetTable(1.5, 2.5, 3.5, 3.1),
]);

function requireInteger(value: number, name: string): void {
  if (!Number.isFinite(value) || !Number.isInteger(value)) {
    throw new TypeError(`${name} must be a finite integer.`);
  }
}

/**
 * Clamp an integer level to the supported range. Fractions and non-finite
 * values are rejected instead of being rounded implicitly.
 */
export function clampAssistLevel(level: number = DEFAULT_ASSIST_LEVEL): AssistLevel {
  requireInteger(level, "Assist level");
  return Math.max(MIN_ASSIST_LEVEL, Math.min(MAX_ASSIST_LEVEL, level)) as AssistLevel;
}

/** Validate an exact level. Invalid integers are rejected rather than clamped. */
export function requireAssistLevel(level: number = DEFAULT_ASSIST_LEVEL): AssistLevel {
  requireInteger(level, "Assist level");
  if (level < MIN_ASSIST_LEVEL || level > MAX_ASSIST_LEVEL) {
    throw new RangeError(`Assist level must be between ${MIN_ASSIST_LEVEL} and ${MAX_ASSIST_LEVEL}.`);
  }
  return level as AssistLevel;
}

/** Lookups reject invalid levels; callers may opt into clamping first. */
export function getAssistTimingTable(level: number = DEFAULT_ASSIST_LEVEL): AssistTimingTable {
  return ASSIST_TIMING_TABLES[requireAssistLevel(level)]!;
}

/** Return priority-ordered windows for a supported judgement type. */
export function getAssistJudgementWindows(
  judgementType: NoteJudgementType,
  level: number = DEFAULT_ASSIST_LEVEL,
): readonly AssistJudgeWindow[] {
  const result = getAssistTimingTable(level)[judgementType];
  if (!result) throw new RangeError(`Unsupported judgement type: ${judgementType}.`);
  return result;
}

/** Lookups reject invalid levels; callers may opt into clamping first. */
export function getAssistAreaOffsetTable(level: number = DEFAULT_ASSIST_LEVEL): AssistAreaOffsetTable {
  return ASSIST_AREA_OFFSET_TABLES[requireAssistLevel(level)]!;
}

/**
 * Resolve a hit-area offset. Note width is required to be finite. Slide widths
 * outside 4..5 are clamped to the nearest endpoint before interpolation.
 */
export function getAssistJudgementAreaOffset(
  offsetType: JudgementAreaOffsetType,
  noteWidth: number,
  level: number = DEFAULT_ASSIST_LEVEL,
): AssistJudgementAreaOffset {
  if (!Number.isFinite(noteWidth)) throw new TypeError("Note width must be finite.");
  const table = getAssistAreaOffsetTable(level);

  if (offsetType !== A.Slide) {
    const stored = table[offsetType];
    if (!stored) throw new RangeError(`Unsupported judgement area offset type: ${offsetType}.`);
    return stored;
  }

  const minimum = table[A.SlideMin]!;
  const maximum = table[A.SlideMax]!;
  const width = Math.fround(noteWidth);
  const span = Math.fround(SLIDE_OFFSET_MAX_NOTE_WIDTH - SLIDE_OFFSET_MIN_NOTE_WIDTH);
  const unclamped = span === 0 ? 0 : Math.fround(Math.fround(width - SLIDE_OFFSET_MIN_NOTE_WIDTH) / span);
  const factor = Math.max(0, Math.min(1, unclamped));
  const delta = Math.fround(maximum.x - minimum.x);
  const x = Math.fround(minimum.x + Math.fround(delta * factor));
  return { x, y: minimum.y };
}
