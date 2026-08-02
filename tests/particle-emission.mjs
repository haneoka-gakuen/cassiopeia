import assert from "node:assert/strict";

import {
  createAnimationWithSimulationSpeed,
  createParticleHarness,
  createSystem,
  directEmissions,
  slideLoopEffect,
} from "./particle-fixture.mjs";

const seed = 123;

// Constant simulation speed preserves the authored start delay in simulated
// seconds: .25 simulated seconds at 2x is .125 real seconds.
{
  const harness = createParticleHarness(
    createSystem({ delay: 0.25, speed: 2, rate: 60, length: 10, looping: false, maxParticles: 256 }),
    { loopAnimation: false },
  );
  const impact = { emissionStates: new Map() };
  assert.deepStrictEqual(directEmissions(harness, impact, 0.124, seed, false), []);
  assert.equal(directEmissions(harness, impact, 0.625, seed, false).length, 59);
  harness.layer.dispose();
}

// Non-looping systems stop at lengthInSec; looping systems continue while
// retaining only the newest native maxParticles records.
{
  const stopped = createParticleHarness(createSystem({ rate: 60, length: 1, looping: false, maxParticles: 10 }));
  const looping = createParticleHarness(createSystem({ rate: 60, length: 1, looping: true, maxParticles: 10 }));
  const stoppedEvents = directEmissions(stopped, { emissionStates: new Map() }, 10, seed, false);
  const loopingEvents = directEmissions(looping, { emissionStates: new Map() }, 10, seed, false);
  assert.equal(stoppedEvents.length, 10);
  assert.equal(loopingEvents.length, 10);
  assert.ok(stoppedEvents.at(-1).simulationTime < 1.01);
  assert.ok(loopingEvents[0].simulationTime > 9.7);
  stopped.layer.dispose();
  looping.layer.dispose();
}

// A rate-only SlideLoop without animationPath now reaches the incremental
// ring. Advancing five minutes stays linear and memory remains maxParticles-
// bounded instead of retaining the entire emission history.
{
  const harness = createParticleHarness(createSystem({ delay: 0.25, speed: 2, rate: 120, maxParticles: 64 }));
  for (let frame = 0; frame <= 300 * 60; frame += 1) harness.layer.update(slideLoopEffect(frame / 60, seed));
  const impact = harness.layer.impacts.get("loop");
  const state = impact?.emissionStates.get(harness.system);
  assert.ok(state, "rate-only SlideLoop did not acquire incremental emission state");
  assert.equal(state.rebuilds, 1);
  // 36k complete simulated steps plus at most one transient tail per 18k
  // rendered frames. A replay-from-zero implementation would be quadratic.
  assert.ok(state.stepEvaluations < 54_100, `unexpected replay growth: ${state.stepEvaluations} evaluations`);
  assert.ok(state.persistent.records.length <= harness.system.maxParticles);
  assert.ok(state.transient.records.length <= harness.system.maxParticles);
  assert.ok(state.output.length <= harness.system.maxParticles);
  harness.layer.dispose();
}

// Backward seeks and seed changes rebuild the ring once and preserve exact
// parity with the complete replay path.
{
  const harness = createParticleHarness(createSystem({ delay: 0.2, speed: 1.5, rate: 73, maxParticles: 37 }));
  const incrementalImpact = { emissionStates: new Map() };
  const fullImpact = { emissionStates: new Map() };
  for (const age of [0, 0.1, 0.133333, 0.5, 1, 2.75, 5]) {
    const fast = directEmissions(harness, incrementalImpact, age, seed, true);
    const full = directEmissions(harness, fullImpact, age, seed, false);
    assert.deepStrictEqual(fast, full, `incremental/full mismatch at age=${age}`);
  }
  directEmissions(harness, incrementalImpact, 2, seed, true);
  let state = incrementalImpact.emissionStates.get(harness.system);
  assert.equal(state.rebuilds, 2, "backward seek must reset the ring exactly once");
  assert.deepStrictEqual(
    directEmissions(harness, incrementalImpact, 2, seed, true),
    directEmissions(harness, fullImpact, 2, seed, false),
  );
  directEmissions(harness, incrementalImpact, 2, seed + 1, true);
  state = incrementalImpact.emissionStates.get(harness.system);
  assert.equal(state.rebuilds, 3, "seed change must reset the deterministic emission stream");
  harness.layer.dispose();
}

// Distance emission and animated simulationSpeed stay on the complete clock
// path. The latter must integrate the selected clip's speed, not the serialized
// fallback speed.
{
  const distance = createParticleHarness(createSystem({ rate: 60, distanceRate: 2 }));
  distance.layer.update(slideLoopEffect(1, seed));
  assert.equal(distance.layer.impacts.get("loop")?.emissionStates.size, 0);
  distance.layer.dispose();

  const animationPath = "root/particles";
  const animated = createParticleHarness(
    createSystem({ speed: 1, rate: 10, maxParticles: 64, lifetime: 10, animationPath }),
    { animation: createAnimationWithSimulationSpeed(animationPath, 2) },
  );
  animated.layer.update(slideLoopEffect(1, seed));
  assert.equal(animated.layer.impacts.get("loop")?.emissionStates.size, 0);
  assert.equal(animated.layer.stats.renderedParticles, 20);
  animated.layer.dispose();
}

// The O(1) constant clock and the retained sampled clock render the same
// particles when an animation binding holds the same speed constant.
{
  const animationPath = "root/constant-speed";
  const fast = createParticleHarness(
    createSystem({ delay: 0.25, speed: 2, rate: 17, maxParticles: 128, lifetime: 20 }),
    { loopAnimation: false },
  );
  const sampled = createParticleHarness(
    createSystem({ delay: 0.25, speed: 2, rate: 17, maxParticles: 128, lifetime: 20, animationPath }),
    { animation: createAnimationWithSimulationSpeed(animationPath, 2, 20), loopAnimation: false },
  );
  for (const age of [0, 0.124, 0.125, 0.5, 1, 2.75, 5]) {
    fast.layer.update(slideLoopEffect(age, seed));
    sampled.layer.update(slideLoopEffect(age, seed));
    assert.equal(
      fast.layer.stats.renderedParticles,
      sampled.layer.stats.renderedParticles,
      `constant/sampled clock mismatch at age=${age}`,
    );
  }
  fast.layer.dispose();
  sampled.layer.dispose();
}

console.log("particle emission regression: ok");
