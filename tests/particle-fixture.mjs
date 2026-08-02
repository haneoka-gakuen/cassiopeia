import { ParticleLayer, StageProjector } from "../dist/index.js";

export const constantCurve = (scalar) => ({ minMaxState: 0, scalar });

export function unityStringHash(value) {
  let crc = 0xffffffff;
  for (const byte of new TextEncoder().encode(value)) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

export function createSystem({
  delay = 0,
  speed = 1,
  rate = 60,
  distanceRate = 0,
  length = 1,
  looping = true,
  maxParticles = 64,
  lifetime = 1,
  animationPath,
  name = "rate-only",
} = {}) {
  const pathHash = animationPath ? unityStringHash(animationPath) : undefined;
  return {
    raw: {
      startDelay: constantCurve(delay),
      InitialModule: {
        startLifetime: constantCurve(lifetime),
        startSize: constantCurve(1),
        startRotation: constantCurve(0),
      },
      EmissionModule: {
        enabled: true,
        m_Bursts: [],
        rateOverTime: constantCurve(rate),
        rateOverDistance: constantCurve(distanceRate),
      },
    },
    length,
    simulationSpeed: speed,
    looping,
    maxParticles,
    simulationSpace: 0,
    scalingMode: 1,
    ref: {
      name,
      metadataUrl: "",
      texture: "star",
      localPosition: [0, 0, 0],
      rendererMaxParticleSize: 1,
      ...(animationPath ? { animationPath } : {}),
    },
    animationPathHash: pathHash,
    animationPathHierarchy: pathHash === undefined ? [] : [pathHash],
    nameHash: unityStringHash(name),
    emissionBuffer: [],
    emissionPool: [],
  };
}

export function createAnimationWithSimulationSpeed(animationPath, speed, duration = 10) {
  const path = unityStringHash(animationPath);
  const attribute = unityStringHash("simulationSpeed");
  const binding = { path, attribute, curveStart: 0, componentCount: 1 };
  return {
    duration,
    streamedCurveCount: 0,
    constantValues: [speed],
    frames: [],
    bindings: [binding],
    bindingLookup: new Map([[path, new Map([[attribute, binding]])]]),
  };
}

export function createParticleHarness(system, { animation, loopAnimation = true } = {}) {
  const manifest = {
    id: 8,
    rootScaleX: 1,
    loopAnimation,
    animationClipUrl: "",
    particleSystems: [],
    sprites: [],
  };
  const assets = { particles: { laneEffects: {}, effect001Prefabs: { SlideLoop: manifest } } };
  const layer = new ParticleLayer(new StageProjector(), assets, 1, 160);
  const prefab = {
    manifest,
    systems: [system],
    animation,
    animations: new Map(),
  };
  layer.loadedPrefabs.set(manifest.id, prefab);
  return { layer, manifest, prefab, system };
}

export function slideLoopEffect(age, seed = 123) {
  return [{ id: "loop", kind: "slide-loop", lane: 10, width: 4, age, seed }];
}

export function emissionSnapshot(emissions) {
  return emissions.map(({ simulationTime, originX, originY, originZ, index }) => ({
    simulationTime,
    originX,
    originY,
    originZ,
    index,
  }));
}

export function directEmissions(harness, impact, age, seed, incremental) {
  harness.layer.incrementalEmissions = incremental;
  return emissionSnapshot(
    harness.layer.emissionEventsFor(impact, harness.system, harness.prefab, age, seed, harness.prefab.animation),
  );
}

