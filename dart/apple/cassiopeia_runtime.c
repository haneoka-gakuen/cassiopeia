#include <stdint.h>

extern uint32_t cassiopeia_abi_version(void);

uint32_t cassiopeia_runtime_link_anchor(void) {
  return cassiopeia_abi_version();
}
