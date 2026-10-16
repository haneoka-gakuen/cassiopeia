import 'dart:convert';
import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'contracts.dart';

typedef _VersionN = Uint32 Function();
typedef _CreateN = Pointer<Void> Function();
typedef _DestroyN = Void Function(Pointer<Void>);
typedef _DispatchN = Size Function(Pointer<Void>, Pointer<Uint8>, Size);
typedef _BytesN = Pointer<Uint8> Function(Pointer<Void>);
typedef _LengthN = Size Function(Pointer<Void>);
typedef _CountN = Size Function();
typedef _WordsN = Pointer<Int64> Function(Pointer<Void>);
typedef _StepN = Int32 Function(Pointer<Void>, Int64);
typedef _ResetN = Int32 Function(
  Pointer<Void>,
  Int64,
  Pointer<Uint32>,
  Pointer<Int64>,
  Size,
);

/// Load the host-selected .so/.dylib/.dll; paths and platform packaging belong to the host.
final class CassiopeiaNativeSession
    implements CassiopeiaSessionPort, CassiopeiaWatchProjectionPort {
  CassiopeiaNativeSession({required DynamicLibrary library})
    : _api = _Bindings(library) {
    if (_api.abi() != 1 ||
        _api.fixedAbi() != 1 ||
        _api.snapshotCount() != 16 ||
        _api.eventStride() != 12) {
      throw StateError('cassiopeia_native_abi');
    }
    _handle = _api.create();
    if (_handle == nullptr) throw StateError('cassiopeia_native_create');
  }
  final _Bindings _api;
  late Pointer<Void> _handle;
  CassiopeiaSessionSnapshot? _snapshot;
  int? _lastObservation;
  bool _closed = false;

  void loadNotes(
    Iterable<CassiopeiaWatchNote> notes, {
    CassiopeiaMode mode = CassiopeiaMode.watch,
    CassiopeiaScoringComboPolicy scoringComboPolicy =
        CassiopeiaScoringComboPolicy.sequential,
    int assistLevel = 0,
    int offsetMicros = 0,
    CassiopeiaAutomaticPolicy? policy,
  }) {
    _dispatch({
      'command': 'loadNotes',
      'notes': [for (final n in notes) n.toJson()],
      'mode': mode.name,
      if (scoringComboPolicy != CassiopeiaScoringComboPolicy.sequential)
        'scoringComboPolicy': scoringComboPolicy.wireName,
      'assistLevel': assistLevel,
      'offsetMicros': offsetMicros,
      'automaticPolicy': (policy ?? CassiopeiaAutomaticPolicy()).toJson(),
    });
    _snapshot = _readSnapshot();
    _lastObservation = null;
  }

  @override
  CassiopeiaSessionSnapshot get snapshot {
    _check();
    return _snapshot ?? (throw StateError('cassiopeia_not_loaded'));
  }

  @override
  CassiopeiaObservation observe(int mediaMicros) {
    _check();
    if (_lastObservation == mediaMicros)
      return CassiopeiaObservation(
        snapshot: snapshot,
        events: const [],
        historical: false,
      );
    if (_lastObservation != null && mediaMicros < _lastObservation!)
      return reset(mediaMicros);
    return _collect(_api.update(_handle, mediaMicros), mediaMicros, false);
  }

  @override
  CassiopeiaObservation reset(
    int mediaMicros, {
    Map<int, int> observedByNote = const {},
  }) {
    _check();
    if (observedByNote.isEmpty)
      return _collect(_api.reset(_handle, mediaMicros), mediaMicros, true);
    final ids = calloc<Uint32>(observedByNote.length),
        times = calloc<Int64>(observedByNote.length);
    try {
      var i = 0;
      for (final e in observedByNote.entries) {
        if (e.key < 0 || e.key > 0xffffffff)
          throw RangeError.value(e.key, 'noteId');
        ids[i] = e.key;
        times[i] = e.value;
        i++;
      }
      return _collect(
        _api.resetObserved(
          _handle,
          mediaMicros,
          ids,
          times,
          observedByNote.length,
        ),
        mediaMicros,
        true,
      );
    } finally {
      calloc.free(ids);
      calloc.free(times);
    }
  }

  @override
  bool get supportsWatchProjection => _api.project != null;

  @override
  CassiopeiaWatchVisualProjection projectWatchObserved(
    int mediaMicros, {
    required Map<int, int> observedByNote,
  }) {
    _check();
    final project = _api.project;
    if (project == null)
      throw UnsupportedError('cassiopeia_watch_projection_unsupported');
    final ids = calloc<Uint32>(observedByNote.length),
        times = calloc<Int64>(observedByNote.length);
    try {
      var i = 0;
      for (final entry in observedByNote.entries) {
        if (entry.key < 0 || entry.key > 0xffffffff)
          throw RangeError.value(entry.key, 'noteId');
        ids[i] = entry.key;
        times[i] = entry.value;
        i++;
      }
      final count = project(
        _handle,
        mediaMicros,
        ids,
        times,
        observedByNote.length,
      );
      if (count < 0) _nativeError();
      final snapshot = CassiopeiaSessionSnapshot.fromWords(
        _api.projectionSnapshot!(_handle).asTypedList(16),
      );
      final words = _api.projectionEvents!(_handle).asTypedList(count * 12);
      return CassiopeiaWatchVisualProjection(
        timeMicros: snapshot.timeMicros,
        combo: snapshot.combo,
        fullCombo: snapshot.fullCombo,
        allPerfect: snapshot.allPerfect,
        perfectCombo: snapshot.perfectCombo,
        events: [
          for (var i = 0; i < count; i++)
            CassiopeiaWatchVisualEvent.fromEvent(
              CassiopeiaJudgementEvent.fromWords(words, i * 12),
            ),
        ],
      );
    } finally {
      calloc.free(ids);
      calloc.free(times);
    }
  }

  Never _nativeError() {
    final length = _api.errorLength(_handle);
    throw StateError(
      length == 0
          ? 'cassiopeia_native_operation'
          : utf8.decode(_api.error(_handle).asTypedList(length)),
    );
  }

  @override
  CassiopeiaObservation finish(int mediaMicros) {
    _check();
    return _collect(_api.finish(_handle, mediaMicros), mediaMicros, false);
  }

  @override
  void configureAutomaticPolicy(CassiopeiaAutomaticPolicy policy) {
    _dispatch({'command': 'setAutomaticPolicy', 'policy': policy.toJson()});
    _snapshot = _readSnapshot();
  }

  CassiopeiaObservation _collect(int count, int mediaMicros, bool historical) {
    if (count < 0) _nativeError();
    final next = _readSnapshot();
    final words = _api.events(_handle).asTypedList(count * 12);
    final events = [
      for (var i = 0; i < count; i++)
        CassiopeiaJudgementEvent.fromWords(words, i * 12),
    ];
    _snapshot = next;
    _lastObservation = mediaMicros;
    return CassiopeiaObservation(
      snapshot: next,
      events: events,
      historical: historical,
    );
  }

  CassiopeiaSessionSnapshot _readSnapshot() =>
      CassiopeiaSessionSnapshot.fromWords(
        _api.snapshot(_handle).asTypedList(16),
      );
  void _dispatch(Map<String, Object> request) {
    _check();
    final bytes = utf8.encode(jsonEncode(request));
    final pointer = calloc<Uint8>(bytes.length);
    try {
      pointer.asTypedList(bytes.length).setAll(0, bytes);
      final length = _api.dispatch(_handle, pointer, bytes.length);
      if (length == 0) throw StateError('cassiopeia_native_dispatch');
      final result = jsonDecode(
        utf8.decode(_api.reply(_handle).asTypedList(length)),
      ) as Map;
      if (result['ok'] != true)
        throw StateError(
          result['error'] as String? ?? 'cassiopeia_native_request',
        );
    } finally {
      calloc.free(pointer);
    }
  }

  void _check() {
    if (_closed) throw StateError('cassiopeia_session_closed');
  }

  @override
  void close() {
    if (_closed) return;
    _closed = true;
    _api.destroy(_handle);
    _handle = nullptr;
    _snapshot = null;
  }
}

final class _Bindings {
  _Bindings(DynamicLibrary l)
    : abi = l.lookupFunction<_VersionN, int Function()>(
        'cassiopeia_abi_version',
      ),
      fixedAbi = l.lookupFunction<_VersionN, int Function()>(
        'cassiopeia_fixed_abi_version',
      ),
      create = l.lookupFunction<_CreateN, Pointer<Void> Function()>(
        'cassiopeia_create',
      ),
      destroy = l.lookupFunction<_DestroyN, void Function(Pointer<Void>)>(
        'cassiopeia_destroy',
      ),
      dispatch = l
          .lookupFunction<
            _DispatchN,
            int Function(Pointer<Void>, Pointer<Uint8>, int)
          >('cassiopeia_dispatch'),
      reply = l.lookupFunction<_BytesN, Pointer<Uint8> Function(Pointer<Void>)>(
        'cassiopeia_reply',
      ),
      update = l.lookupFunction<_StepN, int Function(Pointer<Void>, int)>(
        'cassiopeia_update',
      ),
      reset = l.lookupFunction<_StepN, int Function(Pointer<Void>, int)>(
        'cassiopeia_reset',
      ),
      finish = l.lookupFunction<_StepN, int Function(Pointer<Void>, int)>(
        'cassiopeia_finish',
      ),
      resetObserved = l
          .lookupFunction<
            _ResetN,
            int Function(
              Pointer<Void>,
              int,
              Pointer<Uint32>,
              Pointer<Int64>,
              int,
            )
          >('cassiopeia_reset_observed'),
      snapshot = l
          .lookupFunction<_WordsN, Pointer<Int64> Function(Pointer<Void>)>(
            'cassiopeia_snapshot_words',
          ),
      events = l
          .lookupFunction<_WordsN, Pointer<Int64> Function(Pointer<Void>)>(
            'cassiopeia_event_words',
          ),
      snapshotCount = l.lookupFunction<_CountN, int Function()>(
        'cassiopeia_snapshot_word_count',
      ),
      eventStride = l.lookupFunction<_CountN, int Function()>(
        'cassiopeia_event_stride',
      ),
      error = l.lookupFunction<_BytesN, Pointer<Uint8> Function(Pointer<Void>)>(
        'cassiopeia_error',
      ),
      errorLength = l.lookupFunction<_LengthN, int Function(Pointer<Void>)>(
        'cassiopeia_error_length',
      ) {
    const symbols = [
      'cassiopeia_project_watch_observed',
      'cassiopeia_projection_snapshot_words',
      'cassiopeia_projection_event_words',
    ];
    final present = symbols.where(l.providesSymbol).length;
    if (present != 0 && present != symbols.length)
      throw StateError('cassiopeia_projection_abi');
    project = present == 0
        ? null
        : l.lookupFunction<
            _ResetN,
            int Function(
              Pointer<Void>,
              int,
              Pointer<Uint32>,
              Pointer<Int64>,
              int,
            )
          >(symbols[0]);
    projectionSnapshot = present == 0
        ? null
        : l.lookupFunction<_WordsN, Pointer<Int64> Function(Pointer<Void>)>(
            symbols[1],
          );
    projectionEvents = present == 0
        ? null
        : l.lookupFunction<_WordsN, Pointer<Int64> Function(Pointer<Void>)>(
            symbols[2],
          );
  }
  late final int Function(
    Pointer<Void>,
    int,
    Pointer<Uint32>,
    Pointer<Int64>,
    int,
  )?
  project;
  late final Pointer<Int64> Function(Pointer<Void>)? projectionSnapshot,
      projectionEvents;
  final int Function() abi, fixedAbi, snapshotCount, eventStride;
  final Pointer<Void> Function() create;
  final void Function(Pointer<Void>) destroy;
  final int Function(Pointer<Void>, Pointer<Uint8>, int) dispatch;
  final Pointer<Uint8> Function(Pointer<Void>) reply, error;
  final int Function(Pointer<Void>) errorLength;
  final int Function(Pointer<Void>, int) update, reset, finish;
  final int Function(Pointer<Void>, int, Pointer<Uint32>, Pointer<Int64>, int)
  resetObserved;
  final Pointer<Int64> Function(Pointer<Void>) snapshot, events;
}
