<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import {
  RenderFrameBuilder,
  type RenderSettings,
} from "../adapter/renderFrame";
import type { OurNotesAssetManifest } from "../assets/manifest";
import { MediaClock } from "../audio/MediaClock";
import {
  MusicTimeAnchor,
  normalizeEventRealtimeMs,
} from "../audio/MusicTimeAnchor";
import { NoteSoundPlayer } from "../audio/NoteSoundPlayer";
import { normalizePlaybackRate } from "../audio/playbackRate";
import type { ChartMode } from "../core/enums";
import { LANE_COUNT } from "../core/geometry";
import { ChartSession } from "../core/session";
import type { ChartDocument, LaneInputEffectEvent } from "../core/types";
import { OurNotesInput, type InputPoint } from "../input/OurNotesInput";
import {
  TitleIntroductionPresentation,
  type TitleIntroductionContent,
  type TitleIntroductionSnapshot,
} from "../presentation/TitleIntroduction";
import { OurNotesRenderer } from "../render/OurNotesRenderer";
import { ChartPerfProbe } from "../render/PerfProbe";
import { nativeRenderPixelRatio } from "../render/pixelRatio";
import type {
  RenderFrame,
  RenderTitleIntroductionTheme,
} from "../render/types";
import {
  normalizeExternalTimeMs,
  shouldResetExternalTimeline,
} from "./externalClock";
import type { ChartPlayerEvents, ChartPlayerExpose } from "./types";

const props = withDefaults(
  defineProps<{
    chart: ChartDocument;
    assets: OurNotesAssetManifest;
    audioUrl?: string;
    /**
     * Owner-controlled media time. Providing this disables the internal
     * MediaClock and note-audio graph; owner updates drive one render each.
     */
    externalTimeMs?: number;
    /** Visual playing state for a controlled clock. It never starts an rAF loop. */
    externalPlaying?: boolean;
    /** Sonolus/USC BGM offset: media time = chart time + offset. */
    bgmOffsetMs?: number;
    /** Lightweight live-stage texture composited inside the WebGL base camera. */
    backgroundUrl?: string;
    /** Optional muted stage video synchronized to the music clock. */
    backgroundVideoUrl?: string;
    mode?: ChartMode;
    settings?: Partial<RenderSettings> & {
      judgementOffsetMs?: number;
      __perf?: boolean;
    };
    volume?: number;
    rate?: number;
    loop?: boolean;
    noteSoundEnabled?: boolean;
    noteSoundVolume?: number;
    /** Optional deterministic visual seed for replay and golden capture. */
    effectSeed?: number;
    /** Song metadata shown by the opening title presentation. */
    titleIntroduction?: TitleIntroductionContent;
    /** The opening presentation can be disabled without changing chart time. */
    titleIntroductionEnabled?: boolean;
    titleIntroductionTheme?: Partial<RenderTitleIntroductionTheme>;
    ariaLabel?: string;
    pauseLabel?: string;
    loadingLabel?: string;
  }>(),
  {
    audioUrl: "",
    bgmOffsetMs: 0,
    backgroundUrl: "",
    backgroundVideoUrl: "",
    mode: "watch",
    settings: () => ({}),
    volume: 0.8,
    rate: 1,
    loop: false,
    noteSoundEnabled: true,
    noteSoundVolume: 0.7,
    titleIntroductionEnabled: true,
    ariaLabel: "Chart player",
    pauseLabel: "Pause",
    loadingLabel: "Loading",
  },
);

const emit = defineEmits<ChartPlayerEvents>();
const root = ref<HTMLDivElement | null>(null);
const canvas = ref<HTMLCanvasElement | null>(null);
const hudCanvas = ref<HTMLCanvasElement | null>(null);
const backgroundVideo = ref<HTMLVideoElement | null>(null);
const ready = ref(false);
const failed = ref<Error | null>(null);
let renderer: OurNotesRenderer | undefined;
let clock: MediaClock | undefined;
let session: ChartSession | undefined;
let frameBuilder: RenderFrameBuilder | undefined;
let noteSounds: NoteSoundPlayer | undefined;
let titleIntroduction: TitleIntroductionPresentation | undefined;
let titleIntroductionSnapshot: TitleIntroductionSnapshot | undefined;
let titleIntroductionElapsedMs = 0;
let titleIntroductionRunning = false;
let titleIntroductionStartPending = false;
let titleIntroductionMediaPrimed = false;
let titleIntroductionMediaResumeAtMs = 0;
let titleIntroductionMediaPreviousVolume = 0.8;
let titleIntroductionUnlock: Promise<void> | undefined;
let suppressInternalMediaEvents = false;
let input: OurNotesInput | undefined;
let resizeObserver: ResizeObserver | undefined;
let animationFrame = 0;
let lastTimeEmit = -1;
let lastRenderedTimeMs = Number.NaN;
let suppressEffects = false;
let destroyed = false;
let dirty = true;
let timelineFinished = false;
let perfProbe: ChartPerfProbe | undefined;
let backgroundVideoRevision = 0;
let boundBackgroundVideoUrl = "";
let failedBackgroundVideoUrl = "";
const inputMusicTime = new MusicTimeAnchor();
const activePointerIds = new Set<number>();
const inputFeedbackClaimedPointerIds = new Set<number>();
const lastLaneInputEffect = new Map<number, number>();
const LANE_INPUT_EFFECT_WIDTH = 2;
const externalClockControlled = computed(
  () => props.externalTimeMs !== undefined,
);
const presentationTimeMs = () => {
  if (externalClockControlled.value)
    return normalizeExternalTimeMs(props.externalTimeMs);
  // The authorized media element runs silently behind the title presentation,
  // but gameplay remains exactly at chart time zero until the presentation is
  // complete. The media is rewound before it becomes audible.
  if (titleIntroductionInFlight()) return props.bgmOffsetMs;
  return clock?.timeMs ?? 0;
};
const chartTimeMs = () => presentationTimeMs() - props.bgmOffsetMs;
const titleIntroductionInFlight = () =>
  titleIntroductionRunning || titleIntroductionStartPending;
const gameplayIsPlaying = () =>
  externalClockControlled.value
    ? props.externalPlaying === true
    : clock?.playing === true && !titleIntroductionMediaPrimed;
const playerIsPlaying = () =>
  externalClockControlled.value
    ? props.externalPlaying === true
    : gameplayIsPlaying() || titleIntroductionInFlight();
const presentationDurationMs = () =>
  Math.max(
    clock?.durationMs ?? 0,
    props.chart.durationMs + props.bgmOffsetMs,
    0,
  );
const playbackRate = () => clock?.rate ?? normalizePlaybackRate(props.rate);

function renderPixelRatio(width: number, height: number): number {
  const quality = Math.max(
    0.5,
    Math.min(2, props.settings.graphicsQuality ?? 1),
  );
  return Math.max(
    0.5,
    nativeRenderPixelRatio(width, height, window.devicePixelRatio || 1) *
      quality,
  );
}

function errorOf(reason: unknown): Error {
  return reason instanceof Error ? reason : new Error(String(reason));
}

function reportError(reason: unknown): void {
  failed.value = errorOf(reason);
  emit("error", failed.value);
}

async function applyBackgroundTexture(
  target: OurNotesRenderer,
  url: string,
): Promise<void> {
  try {
    await target.setBackgroundTexture(url);
  } catch (reason) {
    // TextureLoader failures keep the previous WebGL stage (or the opaque
    // black fallback on first load). Treat them like the other prepared runtime
    // asset fallbacks instead of replacing the entire player with an error UI.
    target.reportAssetError(reason);
  }
}

function clearBackgroundVideo(): void {
  backgroundVideoRevision += 1;
  boundBackgroundVideoUrl = "";
  const video = backgroundVideo.value;
  if (!video) return;
  video.onerror = null;
  video.pause();
  video.removeAttribute("src");
  video.load();
}

async function applyBackgroundMedia(target: OurNotesRenderer): Promise<void> {
  const video = backgroundVideo.value;
  const url = props.backgroundVideoUrl;
  if (url && video && failedBackgroundVideoUrl !== url) {
    video.muted = true;
    video.loop = props.loop;
    video.playbackRate = playbackRate();
    if (boundBackgroundVideoUrl === url && video.getAttribute("src") === url) {
      if (video.readyState >= HTMLMediaElement.HAVE_METADATA)
        target.refreshBackgroundLayout();
      return;
    }
    const revision = ++backgroundVideoRevision;
    failedBackgroundVideoUrl = "";
    boundBackgroundVideoUrl = url;
    video.onerror = () => reportBackgroundVideoError(revision, url);
    if (video.getAttribute("src") !== url) {
      video.pause();
      video.src = url;
      video.load();
    }
    target.setBackgroundVideo(video);
    if (video.readyState >= HTMLMediaElement.HAVE_METADATA)
      target.refreshBackgroundLayout();
    return;
  }
  if (url !== failedBackgroundVideoUrl) failedBackgroundVideoUrl = "";
  clearBackgroundVideo();
  target.setBackgroundVideo(undefined);
  await applyBackgroundTexture(target, props.backgroundUrl);
}

function syncBackgroundVideo(
  force: boolean,
  targetPresentationTimeMs = presentationTimeMs(),
): void {
  const video = backgroundVideo.value;
  if (
    !props.backgroundVideoUrl ||
    failedBackgroundVideoUrl === props.backgroundVideoUrl ||
    !video ||
    video.readyState < HTMLMediaElement.HAVE_METADATA ||
    !Number.isFinite(targetPresentationTimeMs)
  )
    return;
  const maximum = Number.isFinite(video.duration)
    ? video.duration
    : Number.POSITIVE_INFINITY;
  const targetSeconds = Math.max(
    0,
    Math.min(maximum, targetPresentationTimeMs / 1000),
  );
  if (force || Math.abs(video.currentTime - targetSeconds) > 0.05)
    video.currentTime = targetSeconds;
}

function refreshBackgroundVideo(): void {
  renderer?.refreshBackgroundLayout();
  syncBackgroundVideo(true);
  dirty = true;
  requestFrame();
}

function reportBackgroundVideoError(revision: number, url: string): void {
  const target = renderer;
  if (
    !target ||
    revision !== backgroundVideoRevision ||
    url !== props.backgroundVideoUrl
  )
    return;
  failedBackgroundVideoUrl = url;
  boundBackgroundVideoUrl = "";
  const video = backgroundVideo.value;
  const message = video?.error?.message || "Stage video could not be loaded";
  if (video) {
    video.onerror = null;
    video.pause();
    video.removeAttribute("src");
    video.load();
  }
  target.reportAssetError(message);
  target.setBackgroundVideo(undefined);
  void applyBackgroundTexture(target, props.backgroundUrl).then(() => {
    if (destroyed || renderer !== target) return;
    dirty = true;
    requestFrame();
  });
}

function attachSession(): void {
  activePointerIds.clear();
  inputFeedbackClaimedPointerIds.clear();
  lastLaneInputEffect.clear();
  timelineFinished = false;
  session = new ChartSession(props.chart, {
    mode: props.mode,
    judgementOffsetMs: props.settings.judgementOffsetMs ?? 0,
  });
  frameBuilder = new RenderFrameBuilder(props.chart, {
    particleSeed: props.effectSeed,
  });
  session.on("judgement", (event) => {
    if (!suppressEffects) {
      frameBuilder?.addJudgement(event, chartTimeMs());
      if (props.noteSoundEnabled) noteSounds?.queue(event);
    }
    dirty = true;
    requestFrame();
    emit("judgement", event);
  });
  session.on("skill", (event) => emit("skill", event));
  session.on("fever", (event) => emit("fever", event));
  session.on("callChange", (event) => emit("callchange", event));
  attachTitleIntroduction();
}

function attachTitleIntroduction(): void {
  const content = props.titleIntroduction;
  titleIntroductionRunning = false;
  titleIntroductionStartPending = false;
  titleIntroductionElapsedMs = 0;
  if (!content?.title) {
    titleIntroduction = undefined;
    titleIntroductionSnapshot = undefined;
    return;
  }
  titleIntroduction = new TitleIntroductionPresentation({
    content,
    enabled: props.titleIntroductionEnabled,
  });
  titleIntroductionSnapshot = titleIntroduction.reset();
}

function resumeTitleIntroduction(): void {
  if (!titleIntroduction || titleIntroductionSnapshot?.state === "complete")
    return;
  titleIntroductionSnapshot = titleIntroduction.start(
    performance.now() - titleIntroductionElapsedMs,
  );
  titleIntroductionRunning = true;
}

function pauseTitleIntroduction(): void {
  if (
    !titleIntroductionRunning ||
    !titleIntroduction ||
    titleIntroductionSnapshot?.state === "complete"
  )
    return;
  titleIntroductionSnapshot = titleIntroduction.update(performance.now());
  titleIntroductionElapsedMs = titleIntroductionSnapshot.elapsedMs;
  titleIntroductionRunning = false;
}

function skipTitleIntroduction(): void {
  if (!titleIntroduction) return;
  titleIntroductionRunning = false;
  titleIntroductionStartPending = false;
  titleIntroductionElapsedMs = titleIntroduction.timing.totalDurationMs;
  titleIntroductionSnapshot = titleIntroduction.atElapsed(
    titleIntroductionElapsedMs,
  );
}

function shouldPlayTitleIntroduction(): boolean {
  return (
    !externalClockControlled.value &&
    Boolean(titleIntroduction) &&
    titleIntroductionSnapshot?.enabled === true &&
    titleIntroductionSnapshot.state !== "complete" &&
    chartTimeMs() <= 0
  );
}

function applyTitleIntroduction(
  frame: RenderFrame,
  realtimeMs: number,
): RenderFrame {
  const presentation = titleIntroduction;
  const hud = frame.hud;
  if (!presentation || !hud) return frame;
  if (titleIntroductionRunning) {
    titleIntroductionSnapshot = presentation.update(realtimeMs);
    titleIntroductionElapsedMs = titleIntroductionSnapshot.elapsedMs;
  }
  const snapshot = titleIntroductionSnapshot;
  hud.titleIntroduction =
    snapshot && snapshot.enabled && snapshot.state !== "complete"
      ? {
          ...snapshot.content,
          alpha: snapshot.alpha,
          contentAlpha: snapshot.contentAlpha,
          theme: props.titleIntroductionTheme,
        }
      : undefined;
  return frame;
}

function addEmptyLaneInputEffect(
  point: InputPoint,
  phase: LaneInputEffectEvent["phase"],
): void {
  if (
    !frameBuilder ||
    !Number.isFinite(point.lane) ||
    point.lane < 0 ||
    point.lane > LANE_COUNT - 1
  )
    return;
  // SetInVainLane addresses the twelve physical lanes through odd chart-lane
  // centres (1, 3, ... 23), so blank feedback occupies one discrete pair.
  const lane = Math.max(
    0,
    Math.min(
      LANE_COUNT - LANE_INPUT_EFFECT_WIDTH,
      Math.floor(point.lane / 2) * 2,
    ),
  );
  if (phase === "move" && lastLaneInputEffect.get(point.pointerId) === lane)
    return;
  frameBuilder.addLaneInput({
    pointerId: point.pointerId,
    lane,
    width: LANE_INPUT_EFFECT_WIDTH,
    timeMs: point.timeMs,
    phase,
  });
  lastLaneInputEffect.set(point.pointerId, lane);
  dirty = true;
  requestFrame();
}

function attachInput(): void {
  if (!root.value || !renderer) return;
  input?.destroy();
  activePointerIds.clear();
  const canJudge = () => props.mode === "play" && gameplayIsPlaying();
  input = new OurNotesInput(
    root.value,
    {
      tap: (point) => {
        if (!canJudge()) return;
        activePointerIds.add(point.pointerId);
        const judgement = session?.tap(
          point.lane,
          point.timeMs,
          point.pointerId,
        );
        if (judgement) inputFeedbackClaimedPointerIds.add(point.pointerId);
        else if (
          !session?.hasInputCandidate(point.lane, point.timeMs, point.pointerId)
        )
          addEmptyLaneInputEffect(point, "tap");
      },
      move: (point) => {
        if (!canJudge()) return;
        const judgement = session?.trace(
          point.lane,
          point.timeMs,
          point.pointerId,
        );
        if (judgement) inputFeedbackClaimedPointerIds.add(point.pointerId);
        else if (
          !inputFeedbackClaimedPointerIds.has(point.pointerId) &&
          !session?.hasInputCandidate(point.lane, point.timeMs, point.pointerId)
        )
          addEmptyLaneInputEffect(point, "move");
      },
      release: (point) => {
        activePointerIds.delete(point.pointerId);
        if (canJudge())
          session?.release(point.lane, point.timeMs, point.pointerId);
        else session?.cancel(point.pointerId);
        inputFeedbackClaimedPointerIds.delete(point.pointerId);
        lastLaneInputEffect.delete(point.pointerId);
      },
      flick: (point) => {
        if (!canJudge()) return;
        const judgement = session?.flick(
          point.previousLane,
          { dx: point.dx, dy: point.dy },
          point.timeMs,
          point.pointerId,
        );
        if (judgement) inputFeedbackClaimedPointerIds.add(point.pointerId);
      },
      cancel: (pointerId) => {
        activePointerIds.delete(pointerId);
        inputFeedbackClaimedPointerIds.delete(pointerId);
        lastLaneInputEffect.delete(pointerId);
        session?.cancel(pointerId);
      },
    },
    {
      now: chartTimeMs,
      eventTime: (event) => {
        const fallback = chartTimeMs();
        return inputMusicTime.timeAt(
          normalizeEventRealtimeMs(event.timeStamp),
          fallback,
        );
      },
      laneAtClientPoint: (clientX, clientY) =>
        renderer?.clientPointToLane(clientX, clientY) ?? 12,
      // PointerEvent coordinates are CSS pixels; CSS defines one inch as 96px.
      screenDpi: 96,
      flickDistanceCm: 0.1,
    },
  );
}

function renderFrame(): void {
  if (!renderer || !session || !frameBuilder) return;
  const playing = playerIsPlaying();
  if (!dirty && !playing) return;
  const timeMs = chartTimeMs();
  const simulationTimeMs = Math.floor(timeMs);
  inputMusicTime.sample(simulationTimeMs, performance.now(), playbackRate());
  if (playing) syncBackgroundVideo(false, timeMs + props.bgmOffsetMs);
  // A playing media element can report the same clock value for several rAFs
  // while buffering or while the platform audio clock advances at a lower
  // cadence. No simulator or visual state changes in those duplicate ticks.
  if (!dirty && timeMs === lastRenderedTimeMs && !titleIntroductionRunning)
    return;
  const frameStarted = perfProbe ? performance.now() : 0;
  const sessionStarted = frameStarted;
  if (props.mode === "play" && playing && activePointerIds.size > 0) {
    for (const point of input?.activePoints ?? []) {
      if (session.trace(point.lane, simulationTimeMs, point.pointerId))
        inputFeedbackClaimedPointerIds.add(point.pointerId);
    }
  }
  const snapshot =
    timelineFinished && !playing
      ? session.snapshot()
      : session.updateReusable(simulationTimeMs);
  const sessionFinished = perfProbe ? performance.now() : 0;
  if (props.noteSoundEnabled) {
    noteSounds?.flush(props.noteSoundVolume);
    noteSounds?.setLongLineActive(
      playing && snapshot.activeLongLine,
      props.noteSoundVolume,
    );
  } else {
    noteSounds?.clearQueue();
    noteSounds?.stopLongLine();
  }
  if (perfProbe) {
    const buildStarted = performance.now();
    const frame = applyTitleIntroduction(
      frameBuilder.buildReusable(timeMs, snapshot, props.settings),
      performance.now(),
    );
    const renderStarted = performance.now();
    renderer.render(frame);
    const renderedAt = performance.now();
    perfProbe.record(
      sessionFinished - sessionStarted,
      renderStarted - buildStarted,
      renderedAt - renderStarted,
      renderedAt - frameStarted,
      renderer.stats,
    );
    const summary = perfProbe.takeSummary(renderedAt);
    if (summary) emit("perf", summary);
  } else {
    renderer.render(
      applyTitleIntroduction(
        frameBuilder.buildReusable(timeMs, snapshot, props.settings),
        performance.now(),
      ),
    );
  }
  lastRenderedTimeMs = timeMs;
  dirty = false;
  const timeSeconds = presentationTimeMs() / 1000;
  if (lastTimeEmit < 0 || Math.abs(timeSeconds - lastTimeEmit) >= 0.03) {
    lastTimeEmit = timeSeconds;
    emit("timeupdate", timeSeconds);
  }
}

function animate(): void {
  animationFrame = 0;
  renderFrame();
  if (
    titleIntroductionRunning &&
    titleIntroductionSnapshot?.state === "complete"
  ) {
    titleIntroductionRunning = false;
    titleIntroductionStartPending = true;
    requestFrame();
    return;
  }
  if (titleIntroductionStartPending) {
    titleIntroductionStartPending = false;
    void startMediaPlayback(false);
    return;
  }
  if (clock?.playing || titleIntroductionRunning) requestFrame();
}

function requestFrame(): void {
  if (!destroyed && !animationFrame)
    animationFrame = requestAnimationFrame(animate);
}

function resize(): void {
  if (!renderer || !root.value) return;
  // CSS rotation changes getBoundingClientRect() to the transformed axis-
  // aligned box. The renderer needs the element's logical layout size so a
  // 90° transition cannot leave its canvas stuck at an intermediate ratio.
  const width = root.value.clientWidth;
  const height = root.value.clientHeight;
  renderer.resize(width, height, renderPixelRatio(width, height));
  dirty = true;
  requestFrame();
}

function resetTimeline(timeMs: number): void {
  if (!session || !frameBuilder) return;
  suppressEffects = true;
  activePointerIds.clear();
  inputFeedbackClaimedPointerIds.clear();
  lastLaneInputEffect.clear();
  noteSounds?.clearQueue();
  noteSounds?.stopLongLine();
  timelineFinished = false;
  if (timeMs > 0) skipTitleIntroduction();
  try {
    frameBuilder.reset();
    session.reset(timeMs);
  } finally {
    suppressEffects = false;
  }
  lastRenderedTimeMs = Number.NaN;
  inputMusicTime.sample(timeMs, performance.now(), playbackRate());
  syncBackgroundVideo(true, timeMs + props.bgmOffsetMs);
  dirty = true;
  requestFrame();
}

function finishTimeline(timeMs = chartTimeMs()): void {
  if (!session || timelineFinished) return;
  for (const pointerId of activePointerIds) session.cancel(pointerId);
  activePointerIds.clear();
  inputFeedbackClaimedPointerIds.clear();
  lastLaneInputEffect.clear();
  noteSounds?.stopLongLine();
  session.finish(Math.max(timeMs, props.chart.durationMs));
  timelineFinished = true;
  lastRenderedTimeMs = Number.NaN;
  dirty = true;
  requestFrame();
}

async function primeMediaForTitleIntroduction(): Promise<void> {
  const target = clock;
  if (!target?.source) return;
  titleIntroductionMediaResumeAtMs = target.timeMs;
  titleIntroductionMediaPreviousVolume = target.volume;
  suppressInternalMediaEvents = true;
  target.volume = 0;
  try {
    // Keep this same authorized playback alive. Pausing here and calling play
    // again after the introduction is rejected by strict autoplay policies.
    if (!target.playing) await target.play();
    titleIntroductionMediaPrimed = true;
  } catch (reason) {
    target.volume = titleIntroductionMediaPreviousVolume;
    titleIntroductionMediaPrimed = false;
    throw reason;
  } finally {
    suppressInternalMediaEvents = false;
  }
}

function restoreTitleIntroductionMedia(pauseMedia: boolean): void {
  const target = clock;
  if (!titleIntroductionMediaPrimed || !target) return;
  suppressInternalMediaEvents = true;
  try {
    if (pauseMedia) target.pause();
    target.seek(titleIntroductionMediaResumeAtMs);
    target.volume = titleIntroductionMediaPreviousVolume;
    titleIntroductionMediaPrimed = false;
    titleIntroductionUnlock = undefined;
  } finally {
    suppressInternalMediaEvents = false;
  }
}

async function startMediaPlayback(unlockNoteSounds = true): Promise<void> {
  if (externalClockControlled.value) return;
  try {
    if (titleIntroductionUnlock) await titleIntroductionUnlock;
    const noteSoundUnlock =
      unlockNoteSounds && props.noteSoundEnabled
        ? noteSounds?.unlock()
        : undefined;
    restoreTitleIntroductionMedia(false);
    const musicPlay = clock?.playing ? undefined : clock?.play();
    syncBackgroundVideo(true);
    const videoPlay =
      props.backgroundVideoUrl &&
      failedBackgroundVideoUrl !== props.backgroundVideoUrl
        ? backgroundVideo.value
            ?.play()
            .catch((reason) => renderer?.reportAssetError(reason))
        : undefined;
    await Promise.all([noteSoundUnlock, musicPlay, videoPlay]);
    requestFrame();
  } catch (reason) {
    if (!destroyed) {
      titleIntroductionRunning = false;
      titleIntroductionStartPending = false;
      restoreTitleIntroductionMedia(true);
      backgroundVideo.value?.pause();
      clock?.pause();
      emit("playing", false);
      reportError(reason);
    }
  }
}

async function play(): Promise<void> {
  if (externalClockControlled.value || titleIntroductionInFlight()) return;
  if (!shouldPlayTitleIntroduction()) {
    await startMediaPlayback();
    return;
  }

  // Note-audio and media priming are both invoked synchronously from the
  // click. The title clock then runs independently while chart/music time is 0.
  const noteSoundUnlock = props.noteSoundEnabled
    ? noteSounds?.unlock()
    : undefined;
  const mediaPrime = titleIntroductionMediaPrimed
    ? undefined
    : primeMediaForTitleIntroduction();
  titleIntroductionUnlock = Promise.all([noteSoundUnlock, mediaPrime]).then(
    () => undefined,
  );
  resumeTitleIntroduction();
  dirty = true;
  requestFrame();
  emit("playing", true);
  try {
    await titleIntroductionUnlock;
  } catch (reason) {
    titleIntroductionRunning = false;
    restoreTitleIntroductionMedia(true);
    emit("playing", false);
    if (!destroyed) reportError(reason);
  }
}

function pause(): void {
  const introductionWasPlaying = titleIntroductionInFlight();
  if (activePointerIds.size > 0) {
    for (const point of input?.activePoints ?? [])
      session?.cancel(point.pointerId);
  }
  activePointerIds.clear();
  inputFeedbackClaimedPointerIds.clear();
  lastLaneInputEffect.clear();
  noteSounds?.clearQueue();
  noteSounds?.stopLongLine();
  pauseTitleIntroduction();
  titleIntroductionStartPending = false;
  backgroundVideo.value?.pause();
  if (titleIntroductionMediaPrimed) restoreTitleIntroductionMedia(true);
  else clock?.pause();
  if (introductionWasPlaying && !clock?.playing) emit("playing", false);
}

function seek(seconds: number, skipIntroduction = true): void {
  if (externalClockControlled.value || !clock || !session || !frameBuilder)
    return;
  const introductionWasPlaying = titleIntroductionInFlight();
  if (titleIntroductionMediaPrimed) restoreTitleIntroductionMedia(true);
  if (skipIntroduction) skipTitleIntroduction();
  clock.seek(seconds * 1000);
  resetTimeline(chartTimeMs());
  if (introductionWasPlaying) emit("playing", false);
}

function attachInternalAudio(): void {
  if (externalClockControlled.value || clock) return;
  const candidate = new MediaClock(props.audioUrl, {
    volume: props.volume,
    playbackRate: props.rate,
    loop: props.loop,
  });
  clock = candidate;
  noteSounds = new NoteSoundPlayer(props.assets.noteSounds);
  void noteSounds.load();
  const active = () => !destroyed && clock === candidate;
  candidate.audio.addEventListener("play", () => {
    if (
      active() &&
      !suppressInternalMediaEvents &&
      !titleIntroductionInFlight()
    ) {
      if (candidate.timeMs + props.bgmOffsetMs < presentationDurationMs())
        timelineFinished = false;
      requestFrame();
      emit("playing", true);
    }
  });
  candidate.audio.addEventListener("pause", () => {
    if (
      active() &&
      !suppressInternalMediaEvents &&
      !titleIntroductionInFlight()
    ) {
      pauseTitleIntroduction();
      dirty = true;
      requestFrame();
      emit("playing", false);
    }
  });
  candidate.audio.addEventListener("ended", () => {
    if (active()) {
      finishTimeline();
      emit("playing", false);
    }
  });
  candidate.audio.addEventListener("durationchange", () => {
    if (active()) emit("duration", presentationDurationMs() / 1000);
  });
  candidate.audio.addEventListener("error", () => {
    if (active())
      reportError(
        candidate.audio.error?.message || "Audio could not be loaded",
      );
  });
}

function detachInternalAudio(): void {
  const previousClock = clock;
  clock = undefined;
  previousClock?.destroy();
  noteSounds?.dispose();
  noteSounds = undefined;
}

async function initialize(): Promise<void> {
  if (!canvas.value || !hudCanvas.value || !root.value) return;
  try {
    perfProbe = props.settings.__perf ? new ChartPerfProbe() : undefined;
    const initialWidth = root.value.clientWidth;
    const initialHeight = root.value.clientHeight;
    const candidate = new OurNotesRenderer({
      canvas: canvas.value,
      hudCanvas: hudCanvas.value,
      alpha: true,
      antialias: true,
      pixelRatio: renderPixelRatio(initialWidth, initialHeight),
      assets: props.assets,
    });
    renderer = candidate;
    await Promise.all([candidate.load(), applyBackgroundMedia(candidate)]);
    if (destroyed || renderer !== candidate) {
      candidate.dispose();
      return;
    }
    attachInternalAudio();
    attachSession();
    attachInput();
    resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(root.value);
    ready.value = true;
    resize();
    if (externalClockControlled.value) {
      skipTitleIntroduction();
      resetTimeline(chartTimeMs());
    } else requestFrame();
    if (
      externalClockControlled.value &&
      props.externalPlaying &&
      props.backgroundVideoUrl
    ) {
      syncBackgroundVideo(true);
      void backgroundVideo.value
        ?.play()
        .catch((reason) => candidate.reportAssetError(reason));
    }
    emit("duration", presentationDurationMs() / 1000);
    emit("ready");
  } catch (reason) {
    if (!destroyed) reportError(reason);
  }
}

watch([() => props.backgroundUrl, () => props.backgroundVideoUrl], async () => {
  const target = renderer;
  if (!target) return;
  await applyBackgroundMedia(target);
  if (destroyed || renderer !== target) return;
  if (playerIsPlaying() && props.backgroundVideoUrl) {
    syncBackgroundVideo(true);
    void backgroundVideo.value
      ?.play()
      .catch((reason) => target.reportAssetError(reason));
  }
  dirty = true;
  requestFrame();
});
watch(
  () => props.mode,
  (mode) => {
    session?.setMode(mode);
    pause();
    attachTitleIntroduction();
    if (externalClockControlled.value) {
      skipTitleIntroduction();
      resetTimeline(chartTimeMs());
    } else seek(0, false);
  },
);
watch([() => props.chart, () => props.effectSeed], () => {
  pause();
  attachSession();
  if (externalClockControlled.value) {
    skipTitleIntroduction();
    resetTimeline(chartTimeMs());
  } else seek(0, false);
  emit("duration", presentationDurationMs() / 1000);
});
watch(
  [() => props.titleIntroduction, () => props.titleIntroductionEnabled],
  () => {
    const introductionWasInFlight = titleIntroductionInFlight();
    attachTitleIntroduction();
    if (
      externalClockControlled.value ||
      chartTimeMs() > 0 ||
      gameplayIsPlaying()
    )
      skipTitleIntroduction();
    else if (introductionWasInFlight && props.titleIntroductionEnabled)
      resumeTitleIntroduction();
    else if (introductionWasInFlight) titleIntroductionStartPending = true;
    dirty = true;
    requestFrame();
  },
  { deep: true },
);
watch(
  () => props.titleIntroductionTheme,
  () => {
    dirty = true;
    requestFrame();
  },
  { deep: true },
);
watch(
  () => props.bgmOffsetMs,
  () => {
    pause();
    if (externalClockControlled.value) resetTimeline(chartTimeMs());
    else seek(0);
    emit("duration", presentationDurationMs() / 1000);
  },
);
watch(externalClockControlled, (controlled) => {
  if (!renderer) return;
  if (controlled) detachInternalAudio();
  else attachInternalAudio();
  if (controlled) skipTitleIntroduction();
  resetTimeline(chartTimeMs());
  emit("duration", presentationDurationMs() / 1000);
});
watch(
  () => props.externalTimeMs,
  (value, previous) => {
    if (!externalClockControlled.value) return;
    if (previous === undefined) return;
    const nextTimeMs = normalizeExternalTimeMs(value);
    const previousTimeMs = normalizeExternalTimeMs(previous);
    if (
      shouldResetExternalTimeline(
        previousTimeMs,
        nextTimeMs,
        props.externalPlaying === true,
      )
    ) {
      resetTimeline(nextTimeMs - props.bgmOffsetMs);
      return;
    }
    if (props.externalPlaying !== true) syncBackgroundVideo(true, nextTimeMs);
    dirty = true;
    requestFrame();
  },
);
watch(
  () => props.externalPlaying,
  (playing, previous) => {
    if (!externalClockControlled.value) return;
    if (!playing) {
      pause();
      const externalChartTimeMs =
        normalizeExternalTimeMs(props.externalTimeMs) - props.bgmOffsetMs;
      if (previous === true && externalChartTimeMs >= props.chart.durationMs)
        finishTimeline(externalChartTimeMs);
    } else {
      timelineFinished = false;
      skipTitleIntroduction();
      if (props.backgroundVideoUrl) {
        syncBackgroundVideo(true);
        void backgroundVideo.value
          ?.play()
          .catch((reason) => renderer?.reportAssetError(reason));
      }
    }
    dirty = true;
    requestFrame();
  },
);
watch(
  () => props.settings,
  (value) => {
    perfProbe = value.__perf ? (perfProbe ?? new ChartPerfProbe()) : undefined;
    session?.setOffset(value.judgementOffsetMs ?? 0);
    resize();
    dirty = true;
    requestFrame();
  },
  { deep: true },
);
watch(
  () => props.audioUrl,
  (value) => {
    if (!clock || clock.source === value) return;
    clock.source = value;
    if (value) seek(0);
    else {
      pause();
      resetTimeline(chartTimeMs());
    }
  },
);
watch(
  () => props.volume,
  (value) => {
    if (!clock) return;
    if (titleIntroductionMediaPrimed) titleIntroductionMediaPreviousVolume = value;
    else clock.volume = value;
  },
);
watch(
  () => props.rate,
  (value) => {
    const normalized = normalizePlaybackRate(value, clock?.rate);
    if (clock) clock.rate = normalized;
    if (backgroundVideo.value) backgroundVideo.value.playbackRate = normalized;
  },
);
watch(
  () => props.loop,
  (value) => {
    if (clock) clock.loop = value;
    if (backgroundVideo.value) backgroundVideo.value.loop = value;
  },
);
watch(
  () => props.noteSoundEnabled,
  (enabled) => {
    if (!enabled) {
      noteSounds?.clearQueue();
      noteSounds?.stopLongLine();
    }
    dirty = true;
    requestFrame();
  },
);
defineExpose<ChartPlayerExpose>({ play, pause, seek, resize });

onMounted(() => nextTick(initialize));
onBeforeUnmount(() => {
  destroyed = true;
  activePointerIds.clear();
  inputFeedbackClaimedPointerIds.clear();
  lastLaneInputEffect.clear();
  cancelAnimationFrame(animationFrame);
  resizeObserver?.disconnect();
  input?.destroy();
  clearBackgroundVideo();
  clock?.destroy();
  noteSounds?.dispose();
  renderer?.dispose();
  input = undefined;
  clock = undefined;
  inputMusicTime.reset();
  noteSounds = undefined;
  renderer = undefined;
});
</script>

<template>
  <div ref="root" class="our-notes-player" :aria-label="ariaLabel">
    <video
      ref="backgroundVideo"
      hidden
      style="display: none !important"
      muted
      playsinline
      preload="auto"
      crossorigin="anonymous"
      aria-hidden="true"
      @loadedmetadata="refreshBackgroundVideo"
      @resize="refreshBackgroundVideo"
    ></video>
    <div class="our-notes-player__stage" aria-hidden="true"></div>
    <canvas ref="canvas" class="our-notes-player__canvas"></canvas>
    <canvas ref="hudCanvas" class="our-notes-player__hud"></canvas>
    <button
      v-if="ready && !externalClockControlled"
      class="our-notes-player__pause-hit"
      type="button"
      :aria-label="pauseLabel"
      @pointerdown.stop
      @click.stop="pause"
    ></button>
    <div v-if="!ready && !failed" class="our-notes-player__status">
      <slot name="loading">{{ loadingLabel }}</slot>
    </div>
    <div v-else-if="failed" class="our-notes-player__status is-error">
      {{ failed.message }}
    </div>
  </div>
</template>
