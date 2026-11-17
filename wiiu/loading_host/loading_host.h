#pragma once
#include <stddef.h>
#include <stdint.h>

typedef int (*HorizonLoadingDrawTarget)(void *context, uint32_t target,
                                       uint32_t width, uint32_t height);

/* Begin a WHB frame, bind/clear TV then DRC, call draw for each while its GX2
 * target is active, and finish both. This is synchronous on the GX2 thread. */
int horizon_loading_host_present(HorizonLoadingDrawTarget draw, void *context);
int horizon_loading_host_present_with_clear(HorizonLoadingDrawTarget draw, void *context,
                                            const float rgba[4]);

/* Return actual WHB colour-buffer dimensions as {TV width, TV height,
 * DRC width, DRC height}. Returns nonzero until both buffers are available. */
int horizon_loading_host_dimensions(uint32_t dimensions[4]);

/* Explicit synchronous TV-target readback for screenshot callers. Call only
 * on the GX2 thread while the TV target is active, after the frame has drawn.
 * `out` must hold at least TV_width * TV_height * 4 bytes. On success it gets
 * tightly packed RGBA8 rows; returns nonzero if the surface is unsupported or
 * allocation/readback fails. This allocates only for this call. */
int horizon_loading_host_read_tv_rgba(uint8_t *out, size_t length);

const uint8_t *horizon_loading_host_hud_gfd(size_t *length);
const uint8_t *horizon_loading_host_panorama_gfd(size_t *length);
const uint8_t *horizon_loading_host_terrain_gfd(size_t *length);
