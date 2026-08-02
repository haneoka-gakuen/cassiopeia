import {
  createParticleHarness,
  createSystem,
  slideLoopEffect,
} from "../tests/particle-fixture.mjs";

const seed = 123;
const age = 300;
const harness = createParticleHarness(
  createSystem({ delay: 0.25, speed: 2, rate: 120, maxParticles: 1_000, lifetime: 5 }),
);

// The first large seek intentionally builds the bounded ring. Measurements
// cover steady rendering at the same media time, the common paused/30Hz effect
// camera case that formerly replayed five minutes from zero every frame.
harness.layer.update(slideLoopEffect(age, seed));
const samples = [];
let checksum = 0;
for (let sample = 0; sample < 21; sample += 1) {
  const started = process.hrtime.bigint();
  harness.layer.update(slideLoopEffect(age, seed));
  samples.push(Number(process.hrtime.bigint() - started));
  checksum += harness.layer.stats.renderedParticles;
}
samples.sort((left, right) => left - right);

const state = harness.layer.impacts.get("loop")?.emissionStates.get(harness.system);
if (!state) throw new Error("effect benchmark did not use the incremental rate ring");
if (state.persistent.records.length > harness.system.maxParticles) throw new Error("effect ring exceeded maxParticles");

console.log(
  JSON.stringify(
    {
      schema: "org.haneoka.cassiopeia.effect-benchmark",
      schemaVersion: 1,
      workload: {
        ageSeconds: age,
        simulationSpeed: harness.system.simulationSpeed,
        ratePerSecond: 120,
        maxParticles: harness.system.maxParticles,
        samples: samples.length,
      },
      median: { steadyStateNanosecondsPerFrame: Math.round(samples[Math.floor(samples.length / 2)]) },
      state: {
        stepEvaluations: state.stepEvaluations,
        rebuilds: state.rebuilds,
        allocatedRingRecords: state.persistent.records.length + state.transient.records.length,
      },
      checksum,
    },
    null,
    2,
  ),
);

harness.layer.dispose();
