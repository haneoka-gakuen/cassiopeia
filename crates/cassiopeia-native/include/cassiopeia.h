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
#ifdef __cplusplus
}
#endif
#endif
