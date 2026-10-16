#ifndef CASSIOPEIA_H
#define CASSIOPEIA_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct NativeHost CassiopeiaHost;
uint32_t cassiopeia_abi_version(void);
CassiopeiaHost *cassiopeia_create(void);
/* One caller at a time per handle. UTF-8 JSON in; UTF-8 JSON out. */
size_t cassiopeia_dispatch(CassiopeiaHost *host, const uint8_t *request, size_t length);
/* Copy `dispatch`'s returned byte count before next dispatch or destruction. */
const uint8_t *cassiopeia_reply(const CassiopeiaHost *host);
void cassiopeia_destroy(CassiopeiaHost *host);
/* Additive fixed ABI1: 16 int64 snapshot words / 12 int64 words per event,
   identical to WASM. Successful operations return event count, -1 on error.
   Reset events are historical; route them to state projection, not live sound/FX.
   Pointers are borrowed until the next operation or handle destruction. */
uint32_t cassiopeia_fixed_abi_version(void);
int32_t cassiopeia_update(CassiopeiaHost *host, int64_t time_micros);
int32_t cassiopeia_reset(CassiopeiaHost *host, int64_t time_micros);
int32_t cassiopeia_reset_observed(CassiopeiaHost *host, int64_t time_micros,
    const uint32_t *note_ids, const int64_t *observed_times, size_t count);
/* Seek-only visual prefix. Allows observed time == note time. Does not change
   gameplay, main snapshot, main events, or the real observation clock.
   Read the separate projection buffers with the existing 16/12 layouts. */
int32_t cassiopeia_project_watch_observed(CassiopeiaHost *host, int64_t time_micros,
    const uint32_t *note_ids, const int64_t *observed_times, size_t count);
const int64_t *cassiopeia_projection_snapshot_words(const CassiopeiaHost *host);
const int64_t *cassiopeia_projection_event_words(const CassiopeiaHost *host);
int32_t cassiopeia_finish(CassiopeiaHost *host, int64_t time_micros);
const int64_t *cassiopeia_snapshot_words(const CassiopeiaHost *host);
size_t cassiopeia_snapshot_word_count(void);
const int64_t *cassiopeia_event_words(const CassiopeiaHost *host);
size_t cassiopeia_event_stride(void);
const uint8_t *cassiopeia_error(const CassiopeiaHost *host);
size_t cassiopeia_error_length(const CassiopeiaHost *host);
#ifdef __cplusplus
}
#endif
#endif
