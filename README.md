# Cassiopeia

**CASSIOPEIA** — **C**ross-platform **A**udio-**S**ynchronized **S**imulation
**I**nfrastructure for **O**pen **P**layback, **E**ffects, **I**nterchange, and
**A**daptation.

Cassiopeia is an embeddable, multi-platform rhythm-game engine. It provides a
web parser, simulation, input system, Three.js renderer, Vue components, and a
portable Rust kernel for deterministic timing, judgement, chart validation,
and replay.

## Packages and entry points

The npm package is `@haneoka/cassiopeia`:

- `@haneoka/cassiopeia/parser` parses and normalizes source scores.
- `@haneoka/cassiopeia/player` embeds the interactive Vue player.
- `@haneoka/cassiopeia/overview` renders a virtualized chart overview.
- `@haneoka/cassiopeia/assets` resolves host-provided runtime assets.
- `haneoka-cassiopeia-core` is the platform-neutral Rust crate.

```ts
import { buildChart, parseScore } from "@haneoka/cassiopeia/parser";

const chart = buildChart(parseScore(scorePayload));
```

The engine does not ship runtime media. A host supplies chart, music, texture,
effect, and font resources through the public asset contracts.

Cassiopeia Chart Format is canonical. SS, SUS, USC, Sonolus `LevelData`, and
source-specific formats are adapters with explicit loss reports. See the
[format contract](docs/CASSIOPEIA_CHART_FORMAT.md).

## Development

```sh
pnpm install --frozen-lockfile
pnpm check
```

The Rust kernel can also be checked independently:

```sh
cargo test --workspace
```

## License

Cassiopeia-authored source is available under MPL-2.0. Third-party components
retain their own licenses. Runtime media is supplied by applications.
