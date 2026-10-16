# Cassiopeia runtime for Dart

`cassiopeia_runtime.dart` exports pure Dart session ports and DTOs. `native.dart`
adds the optional FFI implementation. Neither entry imports a renderer, Flutter,
a game profile or Vega. The host supplies its platform library explicitly:

```dart
final session = CassiopeiaNativeSession(
  library: DynamicLibrary.open(hostSelectedLibraryPath),
);
session.loadNotes(sourceWatchNotes, policy: sourceAutomaticPolicy);
final observation = session.observe(sourceObservedMicros);
```

`CassiopeiaWatchNote` contains the actual stable source identifier, canonical
microsecond time, operation and judgement type. Source order is retained for equal
times. The notes-only constructor supports Watch and Chart; interactive Play uses
the complete RuntimeChartV1 native ABI, with actual positions and line ownership.
No coordinates are synthesized for Watch.

Feed each real media observation once. `observe` returns no new events when the
time is repeated. A backward observation produces a historical reset. A force
render reads `snapshot`; it does not synthesize a tick. This package accepts
integer microseconds; any game's integer-ms or other clock projection belongs to
its source adapter. Keep the raw audio timestamp separate from that projection.

Automatic rules are ordered, first-match half-open observation intervals,
filtered by judgement type. They carry terminal judgement codes; the default is
Perfect. Game code owns construction of mission/range/type data. Changing policy
does not retroactively change results: call `reset` explicitly with real recorded
`observedByNote` times for a mode change or seek. Missing observations use canonical
note time + 1 ms, matching the existing source Watch reconstruction contract.
Future observations beyond a rewind target are ignored during that reset.

```dart
session.configureAutomaticPolicy(nextSourcePolicy);
final rebuilt = session.reset(currentSourceMicros,
    observedByNote: actualPreviouslyAdmittedObservations);
```

`rebuilt.historical` is true and its events rebuild presentation counters. Do not
send them to live sound, vibration or fresh hit spawning. Live `observe`/`finish`
events carry actual observed judgedAt, automatic timing and zero timing error.
`finish` preserves the existing Rust terminal settling behavior; it does not
invent the missed observation cadence of a whole skipped section.

Snapshot and event DTOs own their values. Native arrays are consumed before the
next operation; no borrowed typed list escapes the adapter. Calls for one session
are synchronous on its owning isolate. The host calls `close` at teardown;
repeated close is safe. Library discovery/packaging and platform paths belong to
the host. The runtime never selects a renderer or audio device.

ABI1 JSON is used for loading and changing policy. Per-observation update/reset/
finish use the shared 16-word snapshot and 12-word event regions, with no JSON.
`CassiopeiaObservation.historical` distinguishes rebuilding from fresh emissions.
The snapshot is a render value, not a complete persistent Play checkpoint.
