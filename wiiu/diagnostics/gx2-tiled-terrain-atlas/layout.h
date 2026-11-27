#pragma once

enum {
   COLUMNS = 31, ROWS = 42, SPRITE = 64, CELL = SPRITE + 2,
   WIDTH = COLUMNS * CELL, HEIGHT = ROWS * CELL, PITCH = 2048,
   TILED_HEIGHT = 2776, LINEAR_BYTES = PITCH * HEIGHT * 4,
   TILED_BYTES = PITCH * TILED_HEIGHT * 4,
};

typedef enum {
   AtlasLinear, AtlasTile1D,
} AtlasLayout;

static AtlasLayout fixture_atlas_layout;
