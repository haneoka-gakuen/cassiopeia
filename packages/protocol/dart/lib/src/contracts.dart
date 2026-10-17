/// Stable values from the Cassiopeia runtime contract.
enum CassiopeiaScoringComboPolicy {
  sequential('sequential'),
  strictPriorNoteTime('strict-prior-note-time');

  const CassiopeiaScoringComboPolicy(this.wireName);
  final String wireName;
}

enum CassiopeiaMode { chart, watch, play }

enum CassiopeiaOperation {
  none(0),
  normal(1),
  slideBegin(20),
  slideConnection(21),
  slideEnd(22),
  flick(40),
  slideBeginFlick(41),
  slideEndFlick(42),
  trace(60),
  slideBeginTrace(61),
  slideEndTrace(62),
  slideConnectionTrace(63),
  hiddenSlideBegin(80),
  hiddenSlideEnd(82),
  guideBegin(100),
  guideBeginNormal(101),
  guideBeginFlick(102),
  guideEnd(103),
  guideBeginTrace(104),
  guideEndTrace(105),
  combo(120),
  comboSkip(121),
  hidden(122),
  invalidHidden(123);

  const CassiopeiaOperation(this.code);
  final int code;
  static CassiopeiaOperation fromCode(int code) =>
      values.firstWhere((v) => v.code == code);
}

enum CassiopeiaJudgementType {
  none(0),
  normal(1),
  easyNormal(2),
  flick(5),
  slideBegin(10),
  slideEnd(11),
  slideEndFlick(12),
  slideBeginEasy(15),
  trace(21),
  slideEndTrace(22);

  const CassiopeiaJudgementType(this.code);
  final int code;
  static CassiopeiaJudgementType fromCode(int code) =>
      values.firstWhere((v) => v.code == code);
}

final class CassiopeiaWatchNote {
  const CassiopeiaWatchNote({
    required this.id,
    required this.timeMicros,
    required this.operateType,
    required this.judgementType,
  });
  final int id, timeMicros;
  final CassiopeiaOperation operateType;
  final CassiopeiaJudgementType judgementType;
  Map<String, Object> toJson() => {
    'id': id.toString(),
    'time': timeMicros,
    'operateType': operateType.name,
    'judgementType': judgementType.name,
  };
}

/// Ordered first-match data. The host's game plugin constructs these rules.
final class CassiopeiaAutomaticRule {
  CassiopeiaAutomaticRule({
    required this.startMicros,
    required this.endMicros,
    required Iterable<CassiopeiaJudgementType> judgementTypes,
    required this.judgement,
  }) : judgementTypes = List.unmodifiable(judgementTypes);
  final int startMicros, endMicros, judgement;
  final List<CassiopeiaJudgementType> judgementTypes;
  Map<String, Object> toJson() => {
    'startMicros': startMicros,
    'endMicros': endMicros,
    'judgementTypes': [for (final v in judgementTypes) v.name],
    'judgement': judgement,
  };
}

final class CassiopeiaAutomaticPolicy {
  CassiopeiaAutomaticPolicy([
    Iterable<CassiopeiaAutomaticRule> rules = const [],
  ]) : rules = List.unmodifiable(rules);
  final List<CassiopeiaAutomaticRule> rules;
  Map<String, Object> toJson() => {
    'rules': [for (final r in rules) r.toJson()],
  };
}

final class CassiopeiaSessionSnapshot {
  const CassiopeiaSessionSnapshot({
    required this.mode,
    required this.assistLevel,
    required this.timeMicros,
    required this.offsetMicros,
    required this.combo,
    required this.perfectCombo,
    required this.allPerfect,
    required this.fullCombo,
    required this.maxCombo,
    required this.score,
    required this.life,
    required this.processed,
    required this.total,
    required this.lastInputSequence,
  });
  factory CassiopeiaSessionSnapshot.fromWords(List<int> w) {
    if (w.length != 16 || w[0] != 1)
      throw const FormatException('cassiopeia_snapshot_abi');
    return CassiopeiaSessionSnapshot(
      mode: CassiopeiaMode.values[w[1]],
      assistLevel: w[2],
      timeMicros: w[3],
      offsetMicros: w[4],
      combo: w[5],
      perfectCombo: w[6] != 0,
      allPerfect: w[7] != 0,
      fullCombo: w[8] != 0,
      maxCombo: w[9],
      score: w[10],
      life: w[11],
      processed: w[12],
      total: w[13],
      lastInputSequence: w[14] < 0 ? null : w[14],
    );
  }
  final CassiopeiaMode mode;
  final int assistLevel,
      timeMicros,
      offsetMicros,
      combo,
      maxCombo,
      score,
      life,
      processed,
      total;
  final int? lastInputSequence;
  final bool perfectCombo, allPerfect, fullCombo;
}

/// Fields needed by render-only judgement feedback, independent from score state.
abstract interface class CassiopeiaJudgementView {
  int get noteId;
  int get judgement;
  int get timing;
  int get differenceMicros;
  int get judgedAtMicros;
  int get combo;
}

final class CassiopeiaJudgementEvent implements CassiopeiaJudgementView {
  const CassiopeiaJudgementEvent({
    required this.noteId,
    required this.judgement,
    required this.timing,
    required this.differenceMicros,
    required this.judgedAtMicros,
    required this.inputSequence,
    required this.combo,
    required this.maxCombo,
    required this.score,
    required this.scoreDelta,
    required this.life,
  });
  factory CassiopeiaJudgementEvent.fromWords(List<int> w, int offset) =>
      CassiopeiaJudgementEvent(
        noteId: w[offset],
        judgement: w[offset + 1],
        timing: w[offset + 2],
        differenceMicros: w[offset + 3],
        judgedAtMicros: w[offset + 4],
        inputSequence: w[offset + 5] < 0 ? null : w[offset + 5],
        combo: w[offset + 6],
        maxCombo: w[offset + 7],
        score: w[offset + 8],
        scoreDelta: w[offset + 9],
        life: w[offset + 10],
      );
  final int noteId,
      judgement,
      timing,
      differenceMicros,
      judgedAtMicros,
      combo,
      maxCombo,
      score,
      scoreDelta,
      life;
  final int? inputSequence;
}

final class CassiopeiaObservation {
  CassiopeiaObservation({
    required this.snapshot,
    required Iterable<CassiopeiaJudgementEvent> events,
    required this.historical,
  }) : events = List.unmodifiable(events);
  final CassiopeiaSessionSnapshot snapshot;
  final List<CassiopeiaJudgementEvent> events;

  /// Historical events rebuild presentation state; they are not new sound/hit emissions.
  final bool historical;
}

abstract interface class CassiopeiaSessionPort {
  CassiopeiaSessionSnapshot get snapshot;
  CassiopeiaObservation observe(int mediaMicros);
  CassiopeiaObservation reset(
    int mediaMicros, {
    Map<int, int> observedByNote = const {},
  });
  CassiopeiaObservation finish(int mediaMicros);
  void configureAutomaticPolicy(CassiopeiaAutomaticPolicy policy);
  void close();
}

/// Seek-only visual results have no business score, life, or processed fields.
final class CassiopeiaWatchVisualEvent implements CassiopeiaJudgementView {
  const CassiopeiaWatchVisualEvent({
    required this.noteId,
    required this.judgement,
    required this.timing,
    required this.differenceMicros,
    required this.judgedAtMicros,
    required this.combo,
  });
  factory CassiopeiaWatchVisualEvent.fromEvent(CassiopeiaJudgementEvent e) =>
      CassiopeiaWatchVisualEvent(
        noteId: e.noteId,
        judgement: e.judgement,
        timing: e.timing,
        differenceMicros: e.differenceMicros,
        judgedAtMicros: e.judgedAtMicros,
        combo: e.combo,
      );
  final int noteId, judgement, timing, differenceMicros, judgedAtMicros, combo;
}

final class CassiopeiaWatchVisualProjection {
  CassiopeiaWatchVisualProjection({
    required this.timeMicros,
    required this.combo,
    required this.fullCombo,
    required this.allPerfect,
    required this.perfectCombo,
    required Iterable<CassiopeiaWatchVisualEvent> events,
  }) : events = List.unmodifiable(events);
  final int timeMicros, combo;
  final bool fullCombo, allPerfect, perfectCombo;
  final List<CassiopeiaWatchVisualEvent> events;
  CassiopeiaWatchVisualEvent? get lastEvent => events.lastOrNull;
}

/// Optional capability; older native libraries keep ordinary session support.
abstract interface class CassiopeiaWatchProjectionPort {
  bool get supportsWatchProjection;
  CassiopeiaWatchVisualProjection projectWatchObserved(
    int mediaMicros, {
    required Map<int, int> observedByNote,
  });
}
