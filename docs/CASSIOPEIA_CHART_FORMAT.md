# Cassiopeia Chart Format 1

Status: experimental, additive changes allowed before 1.0.

The canonical media type is
`application/vnd.haneoka.cassiopeia-chart+json;version=1`. The root object has
the fixed identifier `org.haneoka.cassiopeia.chart` and numeric `version: 1`.
The Rust types in `crates/cassiopeia-core/src/chart.rs` are normative for the
current draft.

## Design rules

- Time is authored in integer ticks with an explicit PPQ.
- Tempo uses integer microseconds per quarter note; no locale or binary float
  is needed for interchange.
- Lanes and widths are integer units in the chart-declared lane basis.
- Stable string IDs make hold/guide relationships independent of array order.
- Rendering time scale is separate from judgement/audio time.
- Source-only values live in a namespaced extension during import and must
  produce an explicit warning if an exporter cannot represent them.
- Unknown root or note fields are rejected in version 1. A future extension
  must use a new version or an agreed namespaced extension point.

## Compatibility profiles

`header.compatibilityProfile` selects observable game-specific semantics
without changing the neutral document model. `our-notes-2026` selects the
versioned Our Notes timing, judgement, lane, slide, score, life, and effect
behavior implemented by the runtime. A missing profile uses Cassiopeia's
documented neutral defaults.

Sonolus `LevelData`, SS, SUS, and USC are adapters, not aliases of this schema.
Round trips must report every dropped or approximated capability.
