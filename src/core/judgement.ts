import { JudgeTiming, NoteDirection, NoteJudgementType, NoteSimulateJudgement } from "./enums.js";
import { DEFAULT_ASSIST_LEVEL, getAssistTimingTable, type AssistLevel } from "./assist.js";

export interface JudgeWindow {
  judgement: NoteSimulateJudgement;
  before: number;
  after: number;
}

const J = NoteSimulateJudgement;

/**
 * MasterLiveSettings.note_direction_flick_angle.
 *
 * This value looks surprising, but it is passed to `atan(angle * Deg2Rad)` by
 * the native function rather than to `cos`. At 180 degrees the resulting
 * threshold is greater than one, so a left/right candidate cannot satisfy it.
 */
export const DIRECTION_FLICK_ANGLE = 180;

export interface FlickMoveVector {
  /** Browser screen-space delta: positive X points right. */
  dx: number;
  /** Browser screen-space delta: positive Y points down. */
  dy: number;
}

/**
 * `FTLiveSimulator.NoteJudgementLogic.IsTargetDirectionFlick`.
 *
 * Direction 0 is unconditional. Direction 1/2 creates a left/right unit
 * vector, normalizes both operands, takes their dot product and compares it to
 * `atan(directionFlickAngle * Deg2Rad)`. Unity screen Y points up, hence the
 * browser Y delta is negated before normalization (the target has Y=0, but the
 * conversion is kept explicit to preserve the native coordinate convention).
 */
export function isTargetDirectionFlick(
  direction: NoteDirection,
  move: FlickMoveVector,
  directionFlickAngle = DIRECTION_FLICK_ANGLE,
): boolean {
  if (direction === NoteDirection.Normal) return true;

  const unityX = move.dx;
  const unityY = -move.dy;
  const moveLength = Math.hypot(unityX, unityY);
  const normalizedMoveX = moveLength > 0 ? unityX / moveLength : 0;
  const normalizedMoveY = moveLength > 0 ? unityY / moveLength : 0;

  const targetX = direction === NoteDirection.Left ? -1 : direction === NoteDirection.Right ? 1 : 0;
  const targetY = 0;
  const targetLength = Math.hypot(targetX, targetY);
  const normalizedTargetX = targetLength > 0 ? targetX / targetLength : 0;
  const normalizedTargetY = targetLength > 0 ? targetY / targetLength : 0;
  const dot = normalizedMoveX * normalizedTargetX + normalizedMoveY * normalizedTargetY;
  const threshold = Math.atan(directionFlickAngle * (Math.PI / 180));
  return dot >= threshold;
}

/** Level-zero compatibility view. Use the assist-aware helpers for new code. */
export const JUDGE_WINDOWS: Readonly<Partial<Record<number, readonly JudgeWindow[]>>> =
  getAssistTimingTable(DEFAULT_ASSIST_LEVEL);

export const MAXIMUM_EARLY_WINDOW = Math.max(
  0,
  ...Object.values(JUDGE_WINDOWS).flatMap((windows) => (windows ?? []).map((item) => item.before)),
);

export interface JudgeResult {
  judgement: NoteSimulateJudgement;
  timing: JudgeTiming;
}

export function judge(
  noteType: NoteJudgementType,
  diffMs: number,
  assistLevel: AssistLevel = DEFAULT_ASSIST_LEVEL,
): JudgeResult {
  const windows = getAssistTimingTable(assistLevel)[noteType];
  if (!windows?.length) return { judgement: J.Miss, timing: JudgeTiming.OutOfTime };
  for (const item of windows) {
    if (diffMs < -item.before) continue;
    if (diffMs <= item.after) {
      const timing = Math.abs(diffMs) <= 1 ? JudgeTiming.None : diffMs > 1 ? JudgeTiming.Late : JudgeTiming.Fast;
      return { judgement: item.judgement, timing };
    }
  }
  return { judgement: J.Miss, timing: JudgeTiming.OutOfTime };
}

export function maximumLateWindow(
  noteType: NoteJudgementType,
  assistLevel: AssistLevel = DEFAULT_ASSIST_LEVEL,
): number {
  return Math.max(0, ...(getAssistTimingTable(assistLevel)[noteType] ?? []).map((item) => item.after));
}

export function maximumEarlyWindow(
  noteType: NoteJudgementType,
  assistLevel: AssistLevel = DEFAULT_ASSIST_LEVEL,
): number {
  return Math.max(0, ...(getAssistTimingTable(assistLevel)[noteType] ?? []).map((item) => item.before));
}
