# Cassiopeia

**CASSIOPEIA** — **C**ross-platform **A**udio-**S**ynchronized **S**imulation
**I**nfrastructure for **O**pen **P**layback, **E**ffects, **I**nterchange, and
**A**daptation.

Cassiopeia is an embeddable, multi-platform rhythm-game engine. Its Rust core
provides deterministic timing, judgement, validation, and live-performance
evaluation; the web package provides parsing, input, rendering, and Vue player
components.

Main entry points:

- `@haneoka/cassiopeia/parser`: chart parsing and normalization
- `@haneoka/cassiopeia/player`: interactive player
- `@haneoka/cassiopeia/overview`: chart overview
- `@haneoka/cassiopeia/assets`: host-provided runtime assets
- `@haneoka/cassiopeia/wasm`: lazy browser WebAssembly runtime

```ts
import { buildChart, parseScore } from "@haneoka/cassiopeia/parser";

const chart = buildChart(parseScore(scorePayload));
```

Cassiopeia Chart Format is canonical. SS, SUS, USC, and Sonolus `LevelData`
are supported through adapters with explicit loss reports. Applications supply
music, textures, effects, fonts, and other runtime media.

```sh
pnpm install --frozen-lockfile
pnpm check
```

License: MPL-2.0. Third-party components retain their own licenses.
