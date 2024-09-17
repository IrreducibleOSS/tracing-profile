#pragma once

#include <cstdint>
#include <cstddef>

#include "interface_wrapper.h"

extern "C" {
void *init_perfetto(uint32_t, const char* output_file, size_t buffer_size_kb);
void deinit_perfetto(void *);
}
