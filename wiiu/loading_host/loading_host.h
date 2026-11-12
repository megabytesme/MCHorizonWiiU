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
const uint8_t *horizon_loading_host_hud_gfd(size_t *length);
const uint8_t *horizon_loading_host_panorama_gfd(size_t *length);
const uint8_t *horizon_loading_host_terrain_gfd(size_t *length);
