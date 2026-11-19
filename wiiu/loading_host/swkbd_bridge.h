#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Narrow Rust/C ABI for the Wii U system software keyboard.
 *
 * Status returned by horizon_swkbd_take_result:
 *   0: no completed request
 *   1: accepted text copied to output
 *   2: cancelled
 *  <0: invalid argument, setup failure, or output buffer too small
 */
int32_t horizon_swkbd_request(const uint16_t *initial_text,
                              int32_t max_utf16_units);
int32_t horizon_swkbd_is_open(void);
int32_t horizon_swkbd_calc(const void *vpad_status);
int32_t horizon_swkbd_draw(uint32_t target);
int32_t horizon_swkbd_take_result(char *utf8, size_t capacity,
                                  size_t *length);
void horizon_swkbd_close(void);

#ifdef __cplusplus
}
#endif
