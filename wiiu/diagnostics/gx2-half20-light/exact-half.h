#pragma once

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

static inline bool exact_half_position(float value, uint16_t *encoded)
{
    uint32_t bits;
    memcpy(&bits, &value, sizeof(bits));
    const uint16_t sign = (uint16_t)((bits >> 16) & 0x8000u);
    if ((bits & 0x7fffffffu) == 0) {
        *encoded = sign;
        return true;
    }
    const unsigned exponent = (bits >> 23) & 0xffu;
    // Normal finite half or signed zero. Treat half subnormals as a
    // full-precision fallback so the GPU need not preserve them.
    if (exponent < 113u || exponent > 142u || (bits & 0x1fffu))
        return false;
    *encoded = (uint16_t)(sign | ((exponent - 112u) << 10) | ((bits >> 13) & 0x3ffu));
    return true;
}
