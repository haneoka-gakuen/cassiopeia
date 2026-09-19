# Cassiopeia

Portable rhythm-game kernel and typed plugin contracts. This repository owns
integer timing, candidate selection, judgement, scoring, session state, native
C ABI and WASM bindings. It has no Three, Vue or Sonolus dependency.

Feature implementations live in independent repositories:

| Package | Responsibility |
| --- | --- |
| `@haneoka/cassiopeia-plugin-our-notes` | SS normalization, presentation frames, extracted skin/effect metadata and haptic policy |
| `@haneoka/cassiopeia-renderer-three` | WebGL stage, note/effect rendering, HUD and sprite atlases |
| `@haneoka/cassiopeia-host-web` | Browser audio clock, note sounds and pointer input |
| `@haneoka/cassiopeia-ui-vue` | Vue player and overview, composed from the preceding plugins |
| `@haneoka/cassiopeia-plugin-sonolus` | Sonolus conversion, play/watch/preview/tutorial targets and offline effect capture |

```ts
import { CassiopeiaRuntime, createKernelPlugin, CASSIOPEIA_SESSION } from '@haneoka/cassiopeia/plugin';
import { createOurNotesPlugin, OUR_NOTES_RULES } from '@haneoka/cassiopeia-plugin-our-notes';

const runtime = new CassiopeiaRuntime([createKernelPlugin(), createOurNotesPlugin()]);
const chart = runtime.require(OUR_NOTES_RULES).parse(sourceScore);
const session = runtime.require(CASSIOPEIA_SESSION).create(chart, { mode: 'play' });
// Supply timestamped input from the host audio clock.
runtime.dispose();
```

Plugins declare API version, dependencies and provided services. Installation
sorts dependencies, rejects duplicates/missing dependencies/cycles, and rolls
back cleanup callbacks on failure. Every runtime owns its services; there is no
global singleton or implicit discovery. A factory's caller owns the objects it
creates and must dispose them. Plugin-scoped subscriptions use `context.defer`.

The website uses `.dependencies` checkouts, `workspace:*` links and an exact Git
compatibility lock, following Vega. There is no tarball fallback. Unpublished
local commits require `HANEOKA_DEPENDENCY_SOURCE_ROOT=/path/to/local/repositories`
when running the website's `pnpm dependencies:checkout`.

```sh
pnpm install --frozen-lockfile
CARGO_BUILD_JOBS=1 NODE_OPTIONS=--max-old-space-size=1536 pnpm check
```

The native library exposes the same kernel boundary through C, Android JNI and
Swift wrappers. GPU rendering, audio devices and platform haptics remain host
responsibilities.

License: MPL-2.0. Third-party components retain their notices.
