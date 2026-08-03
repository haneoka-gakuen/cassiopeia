/**
 * Invalidates asynchronous playback work when a newer transport operation wins.
 *
 * A title-introduction handoff remains presentation playback until the matching
 * media request settles. Pause, seek, and restart cancel that handoff atomically.
 */
export class PlayerPlaybackGate {
  private serial = 0;
  private activeGeneration: number | undefined;
  private handoffGeneration: number | undefined;
  private requested = false;

  get handoffPending(): boolean {
    return this.handoffGeneration !== undefined;
  }

  get playbackRequested(): boolean {
    return this.requested;
  }

  begin(): number {
    this.serial += 1;
    this.activeGeneration = this.serial;
    this.handoffGeneration = undefined;
    this.requested = true;
    return this.serial;
  }

  beginHandoff(generation: number): boolean {
    if (!this.isCurrent(generation)) return false;
    this.handoffGeneration = generation;
    return true;
  }

  isCurrent(generation: number): boolean {
    return this.activeGeneration === generation;
  }

  finish(generation: number): boolean {
    if (!this.isCurrent(generation)) return false;
    this.activeGeneration = undefined;
    this.handoffGeneration = undefined;
    return true;
  }

  cancel(): void {
    this.serial += 1;
    this.activeGeneration = undefined;
    this.handoffGeneration = undefined;
    this.requested = false;
  }
}

export function shouldStartPlayerIntroduction(
  _externalClockControlled: boolean,
  enabled: boolean,
  hasDisplayableTitle: boolean,
  complete: boolean,
): boolean {
  // An owner-controlled media clock cannot be mutated here, but the opening
  // presentation is still locally clocked. Completion is handed back through
  // the explicit external-playback-requested event.
  return enabled && hasDisplayableTitle && !complete;
}
