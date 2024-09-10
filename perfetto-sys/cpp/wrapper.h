#pragma once

#include <cstdint>

#include "interface_wrapper.h"

extern "C" {
void *init_perfetto(uint32_t);
void deinit_perfetto(void *);
bool is_category_enabled(char *);
}
