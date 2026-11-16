#include <stdint.h>
#include <stddef.h>
#include <string.h>

#include <coreinit/debug.h>
#include <gx2/clear.h>
#include <gx2/draw.h>
#include <gx2/event.h>
#include <gx2/registers.h>
#include <gx2/sampler.h>
#include <gx2/shaders.h>
#include <gx2/texture.h>
#include <gx2/utils.h>
#include <gx2r/buffer.h>
#include <gx2r/draw.h>
#include <gx2r/surface.h>
#include <vpad/input.h>
#include <whb/gfx.h>
#include <whb/proc.h>
#include "vpad_bridge.h"

extern void horizon_wiiu_frame_color(uint32_t frame, float rgba[4]);
extern const uint8_t _binary_horizon_block_gsh_start[];

#define WORLD_VERTEX_CAPACITY 66560
#define WORLD_TRIANGLE_CAPACITY ((WORLD_VERTEX_CAPACITY / 4) * 6)
#define GROUND_HEIGHT_COUNT (16 * 16)
#ifdef HORIZON_WIIU_WORLDGEN
#define ATLAS_TILE_SIZE 16
#define ATLAS_COLUMNS 4
#define ATLAS_ROWS 2
#else
#define ATLAS_TILE_SIZE 4
#define ATLAS_COLUMNS 2
#define ATLAS_ROWS 2
#endif
#define ATLAS_WIDTH (ATLAS_TILE_SIZE * ATLAS_COLUMNS)
#define ATLAS_HEIGHT (ATLAS_TILE_SIZE * ATLAS_ROWS)
#ifndef HORIZON_WIIU_WORLD_PROFILE_SUPERFLAT
#define HORIZON_WIIU_WORLD_PROFILE_SUPERFLAT 1
#endif
#ifndef HORIZON_WIIU_WORLD_REGION_RADIUS
#define HORIZON_WIIU_WORLD_REGION_RADIUS 1
#endif
#define HORIZON_WIIU_DIAGNOSTIC_CAMERA_MARGIN 4.0f
#define HORIZON_WIIU_WORLD_REGION_DIAMETER ((HORIZON_WIIU_WORLD_REGION_RADIUS * 2 + 1) * 16)

typedef struct Vertex {
   float position[4];
   float color[3];
   float uv[2];
   float alphaCutout;
   float lightUv[2];
} Vertex;

typedef struct HorizonTerrainVertex {
   float position[3];
   uint16_t uv[2];
   uint16_t layer;
   uint16_t light;
   uint32_t color;
} HorizonTerrainVertex;

_Static_assert(sizeof(HorizonTerrainVertex) == 24, "Horizon TerrainVertex ABI must stay 24 bytes");

extern uint32_t horizon_wiiu_build_runtime_chunk(HorizonTerrainVertex *vertices,
                                                  uint32_t capacity,
                                                  int16_t *heights,
                                                  uint32_t heightCapacity,
                                                  int64_t seed);
extern uint32_t horizon_wiiu_encode_terrain_quads(const HorizonTerrainVertex *source,
                                                   uint32_t source_vertex_count,
                                                   float *output,
                                                   uint32_t output_capacity,
                                                   float section_x,
                                                   float section_y,
                                                   float section_z,
                                                   const float camera[9],
                                                   uint32_t perspective);
extern void horizon_wiiu_camera_motion(float yaw_degrees,
                                       float forward,
                                       float strafe,
                                       float output[2]);
#ifdef HORIZON_WIIU_WORLDGEN
extern void horizon_wiiu_log_init(void);
extern int32_t horizon_wiiu_world_init(const char *resources, int64_t seed,
                                        uint32_t profile, int32_t spawn[2], char *error,
                                        uint32_t errorCapacity);
extern void *horizon_wiiu_world_generate_region(int32_t centerX, int32_t centerZ,
                                                 uint32_t radius, char *error,
                                                 uint32_t errorCapacity);
extern int32_t horizon_wiiu_world_request_region(int32_t centerX, int32_t centerZ,
                                                  uint32_t radius, char *error,
                                                  uint32_t errorCapacity);
extern int32_t horizon_wiiu_world_take_region_result(void **chunk, char *error,
                                                      uint32_t errorCapacity);
extern const HorizonTerrainVertex *horizon_wiiu_world_chunk_vertices(
   const void *chunk, uint32_t *vertexCount);
extern const uint8_t *horizon_wiiu_world_terrain_atlas(void);
extern uint32_t horizon_wiiu_world_terrain_atlas_width(void);
extern uint32_t horizon_wiiu_world_terrain_atlas_height(void);
extern int32_t horizon_wiiu_world_chunk_position(const void *chunk, int32_t output[2]);
extern int32_t horizon_wiiu_world_chunk_diagnostic_surface_y(const void *chunk,
                                                              int32_t *surfaceY);
extern void horizon_wiiu_world_free_chunk(void *chunk);
#define HORIZON_WIIU_RESOURCE_PATH "/vol/external01/wiiu/apps/HorizonWiiU/resources/26.3"
#endif

#ifdef HORIZON_WIIU_CAPTURE
extern const HorizonTerrainVertex horizon_wiiu_capture_vertices[];
extern const uint32_t horizon_wiiu_capture_vertex_count;
extern const float horizon_wiiu_capture_section_origin[3];
extern const uint8_t horizon_wiiu_capture_atlas_pixels[];
extern const uint32_t horizon_wiiu_capture_atlas_width;
extern const uint32_t horizon_wiiu_capture_atlas_height;
extern const uint8_t horizon_wiiu_capture_lightmap_pixels[16 * 16 * 4];
extern const float horizon_wiiu_capture_view[4];
#endif

static WHBGfxShaderGroup sBlockShader;
static GX2RBuffer sBlockBuffer;
static GX2Texture sBlockAtlas;
static GX2Sampler sBlockSampler;
static GX2Texture sLightmap;
static GX2Sampler sLightmapSampler;
static const HorizonTerrainVertex *sBlockSection;
static uint32_t sAtlasWidth;
static uint32_t sAtlasHeight;
#ifndef HORIZON_WIIU_CAPTURE
#ifdef HORIZON_WIIU_WORLDGEN
static void *sRuntimeWorldChunk;
#else
static HorizonTerrainVertex sRuntimeChunk[WORLD_VERTEX_CAPACITY];
#endif
#if !defined(HORIZON_WIIU_WORLDGEN)
static int16_t sGroundHeights[GROUND_HEIGHT_COUNT];
#endif
#endif
static float sSectionOrigin[3];
static uint32_t sBlockSectionVertexCount;
static uint32_t sBlockOutputCapacity;
static uint32_t sBlockVertexCount;
#ifdef HORIZON_WIIU_WORLDGEN
static int sBlockInitFailure;
static uint32_t sRegionReloadCooldown;
#if HORIZON_WIIU_WORLD_REGION_RADIUS > 0
static BOOL sRegionLoadPending;
static int32_t sPendingRegionCenterX;
static int32_t sPendingRegionCenterZ;
#endif
#endif
static float sCameraX;
static float sCameraY;
static float sCameraZ;
static float sCameraYaw;
static float sCameraPitch;
static float sFocalScale = 1.0f;
static float sAspect = 1.0f;
static BOOL sBlockMeshDirty = TRUE;

static void
set_atlas_pixel(uint8_t *pixels, uint32_t pitch, uint32_t x, uint32_t y,
                uint8_t red, uint8_t green, uint8_t blue)
{
   uint8_t *pixel = pixels + ((y * pitch + x) * 4);
   pixel[0] = red;
   pixel[1] = green;
   pixel[2] = blue;
   pixel[3] = 255;
}

static BOOL
init_block_atlas(void)
{
   memset(&sBlockAtlas, 0, sizeof(sBlockAtlas));
   sBlockAtlas.surface.use = GX2_SURFACE_USE_TEXTURE;
   sBlockAtlas.surface.dim = GX2_SURFACE_DIM_TEXTURE_2D;
   sAtlasWidth = ATLAS_WIDTH;
   sAtlasHeight = ATLAS_HEIGHT;
#ifdef HORIZON_WIIU_CAPTURE
   sAtlasWidth = horizon_wiiu_capture_atlas_width;
   sAtlasHeight = horizon_wiiu_capture_atlas_height;
   if (sAtlasWidth == 0 || sAtlasHeight == 0) {
      return FALSE;
   }
#elif defined(HORIZON_WIIU_WORLDGEN)
   const uint32_t preparedWidth = horizon_wiiu_world_terrain_atlas_width();
   const uint32_t preparedHeight = horizon_wiiu_world_terrain_atlas_height();
   if (preparedWidth != 0 && preparedHeight != 0) {
      sAtlasWidth = preparedWidth;
      sAtlasHeight = preparedHeight;
   }
#endif
   sBlockAtlas.surface.width = sAtlasWidth;
   sBlockAtlas.surface.height = sAtlasHeight;
   sBlockAtlas.surface.depth = 1;
   sBlockAtlas.surface.mipLevels = 1;
   sBlockAtlas.surface.format = GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8;
   sBlockAtlas.surface.aa = GX2_AA_MODE1X;
   sBlockAtlas.surface.tileMode = GX2_TILE_MODE_LINEAR_ALIGNED;
   sBlockAtlas.viewNumMips = 1;
   sBlockAtlas.viewNumSlices = 1;
   sBlockAtlas.compMap = GX2_COMP_MAP(GX2_SQ_SEL_R, GX2_SQ_SEL_G, GX2_SQ_SEL_B, GX2_SQ_SEL_A);

   const GX2RResourceFlags textureFlags = GX2R_RESOURCE_BIND_TEXTURE
                                          | GX2R_RESOURCE_USAGE_CPU_WRITE
                                          | GX2R_RESOURCE_USAGE_GPU_READ;
   if (!GX2RCreateSurface(&sBlockAtlas.surface, textureFlags)) {
      return FALSE;
   }

   uint8_t *pixels = (uint8_t *)GX2RLockSurfaceEx(&sBlockAtlas.surface, 0, textureFlags);
   if (!pixels) {
      GX2RDestroySurfaceEx(&sBlockAtlas.surface, textureFlags);
      memset(&sBlockAtlas, 0, sizeof(sBlockAtlas));
      return FALSE;
   }

   const uint32_t pitch = sBlockAtlas.surface.pitch;
#ifdef HORIZON_WIIU_CAPTURE
   for (uint32_t y = 0; y < sAtlasHeight; ++y) {
      memcpy(pixels + (y * pitch * 4),
             horizon_wiiu_capture_atlas_pixels + (y * sAtlasWidth * 4),
             sAtlasWidth * 4);
   }
#else
   const uint8_t *preparedAtlas = NULL;
#ifdef HORIZON_WIIU_WORLDGEN
   preparedAtlas = horizon_wiiu_world_terrain_atlas();
#endif
   if (preparedAtlas) {
      for (uint32_t y = 0; y < sAtlasHeight; ++y) {
         memcpy(pixels + (y * pitch * 4),
                preparedAtlas + (y * sAtlasWidth * 4), sAtlasWidth * 4);
      }
   } else {
      for (uint32_t y = 0; y < sAtlasHeight; ++y) {
         for (uint32_t x = 0; x < sAtlasWidth; ++x) {
            const uint32_t tileX = x / ATLAS_TILE_SIZE;
            const uint32_t tileY = y / ATLAS_TILE_SIZE;
            const uint32_t localX = x % ATLAS_TILE_SIZE;
            const uint32_t localY = y % ATLAS_TILE_SIZE;
            const BOOL speckle = ((localX + localY) & 1) != 0;
            uint8_t red, green, blue;

            if (tileX == 0 && tileY == 0) {
               red = speckle ? 72 : 55;
               green = speckle ? 220 : 199;
               blue = speckle ? 78 : 62;
            } else if (tileX == 1 && tileY == 0) {
               if (localY == 0) {
                  red = speckle ? 72 : 55;
                  green = speckle ? 192 : 171;
                  blue = speckle ? 66 : 52;
               } else {
                  red = speckle ? 177 : 154;
                  green = speckle ? 109 : 88;
                  blue = speckle ? 61 : 47;
               }
            } else if (tileX == 2 && tileY == 0) {
               red = speckle ? 137 : 119;
               green = speckle ? 137 : 119;
               blue = speckle ? 137 : 119;
            } else if (tileX == 0 && tileY == 1) {
               red = speckle ? 226 : 206;
               green = speckle ? 211 : 190;
               blue = speckle ? 160 : 140;
            } else if (tileX == 1 && tileY == 1) {
               red = speckle ? 119 : 101;
               green = speckle ? 110 : 94;
               blue = speckle ? 81 : 69;
            } else if (tileX == 2 && tileY == 1) {
               red = localX < 3 || localX > 12 ? 92 : (speckle ? 158 : 135);
               green = localX < 3 || localX > 12 ? 66 : (speckle ? 112 : 91);
               blue = localX < 3 || localX > 12 ? 40 : (speckle ? 65 : 49);
            } else {
               red = speckle ? 133 : 116;
               green = speckle ? 87 : 72;
               blue = speckle ? 49 : 40;
            }

            set_atlas_pixel(pixels, pitch, x, y, red, green, blue);
         }
      }
   }
#endif

   GX2RUnlockSurfaceEx(&sBlockAtlas.surface, 0, textureFlags);
   GX2InitTextureRegs(&sBlockAtlas);
   GX2InitSampler(&sBlockSampler, GX2_TEX_CLAMP_MODE_CLAMP, GX2_TEX_XY_FILTER_MODE_POINT);
   return TRUE;
}

static BOOL
init_lightmap(void)
{
   memset(&sLightmap, 0, sizeof(sLightmap));
   sLightmap.surface.use = GX2_SURFACE_USE_TEXTURE;
   sLightmap.surface.dim = GX2_SURFACE_DIM_TEXTURE_2D;
   sLightmap.surface.width = 16;
   sLightmap.surface.height = 16;
   sLightmap.surface.depth = 1;
   sLightmap.surface.mipLevels = 1;
   sLightmap.surface.format = GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8;
   sLightmap.surface.aa = GX2_AA_MODE1X;
   sLightmap.surface.tileMode = GX2_TILE_MODE_LINEAR_ALIGNED;
   sLightmap.viewNumMips = 1;
   sLightmap.viewNumSlices = 1;
   sLightmap.compMap = GX2_COMP_MAP(GX2_SQ_SEL_R, GX2_SQ_SEL_G, GX2_SQ_SEL_B, GX2_SQ_SEL_A);

   const GX2RResourceFlags textureFlags = GX2R_RESOURCE_BIND_TEXTURE
                                          | GX2R_RESOURCE_USAGE_CPU_WRITE
                                          | GX2R_RESOURCE_USAGE_GPU_READ;
   if (!GX2RCreateSurface(&sLightmap.surface, textureFlags)) {
      return FALSE;
   }

   uint8_t *pixels = (uint8_t *)GX2RLockSurfaceEx(&sLightmap.surface, 0, textureFlags);
   if (!pixels) {
      GX2RDestroySurfaceEx(&sLightmap.surface, textureFlags);
      memset(&sLightmap, 0, sizeof(sLightmap));
      return FALSE;
   }

   const uint32_t pitch = sLightmap.surface.pitch;
   for (uint32_t y = 0; y < 16; ++y) {
#ifdef HORIZON_WIIU_CAPTURE
      memcpy(pixels + (y * pitch * 4),
             horizon_wiiu_capture_lightmap_pixels + (y * 16 * 4),
             16 * 4);
#else
      memset(pixels + (y * pitch * 4), 0xFF, 16 * 4);
#endif
   }

   GX2RUnlockSurfaceEx(&sLightmap.surface, 0, textureFlags);
   GX2InitTextureRegs(&sLightmap);
   GX2InitSampler(&sLightmapSampler, GX2_TEX_CLAMP_MODE_CLAMP, GX2_TEX_XY_FILTER_MODE_LINEAR);
   return TRUE;
}

static BOOL
init_block_mesh(void)
{
   if (!WHBGfxLoadGFDShaderGroup(&sBlockShader, 0, _binary_horizon_block_gsh_start)) {
#ifdef HORIZON_WIIU_WORLDGEN
      sBlockInitFailure = 1;
#endif
      return FALSE;
   }

   if (!WHBGfxInitShaderAttribute(&sBlockShader, "aPosition", 0,
                                  offsetof(Vertex, position), GX2_ATTRIB_FORMAT_FLOAT_32_32_32_32)
       || !WHBGfxInitShaderAttribute(&sBlockShader, "aColor", 0,
                                     offsetof(Vertex, color), GX2_ATTRIB_FORMAT_FLOAT_32_32_32)
       || !WHBGfxInitShaderAttribute(&sBlockShader, "aUv", 0,
                                     offsetof(Vertex, uv), GX2_ATTRIB_FORMAT_FLOAT_32_32)
       || !WHBGfxInitShaderAttribute(&sBlockShader, "aAlphaCutout", 0,
                                     offsetof(Vertex, alphaCutout), GX2_ATTRIB_FORMAT_FLOAT_32)
       || !WHBGfxInitShaderAttribute(&sBlockShader, "aLightUv", 0,
                                     offsetof(Vertex, lightUv), GX2_ATTRIB_FORMAT_FLOAT_32_32)
       || !WHBGfxInitFetchShader(&sBlockShader)) {
#ifdef HORIZON_WIIU_WORLDGEN
      sBlockInitFailure = 2;
#endif
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }

#ifdef HORIZON_WIIU_CAPTURE
   sBlockSection = horizon_wiiu_capture_vertices;
   sBlockSectionVertexCount = horizon_wiiu_capture_vertex_count;
   memcpy(sSectionOrigin, horizon_wiiu_capture_section_origin, sizeof(sSectionOrigin));
   if (!sBlockSection || sBlockSectionVertexCount == 0 || sBlockSectionVertexCount % 4 != 0) {
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }
   sCameraYaw = horizon_wiiu_capture_view[0];
   sCameraPitch = horizon_wiiu_capture_view[1];
   sFocalScale = horizon_wiiu_capture_view[2];
   sAspect = horizon_wiiu_capture_view[3];
   sBlockOutputCapacity = sBlockSectionVertexCount / 4 * 6;
#else
#ifdef HORIZON_WIIU_WORLDGEN
   int32_t spawn[2] = {0, 0};
   char error[256] = {0};
   /* The reduced flat preset is a deterministic renderer diagnostic fixture. */
   if (!horizon_wiiu_world_init(HORIZON_WIIU_RESOURCE_PATH, INT64_C(12345),
                                HORIZON_WIIU_WORLD_PROFILE_SUPERFLAT,
                                spawn, error, sizeof(error))) {
#ifdef HORIZON_WIIU_WORLDGEN
      sBlockInitFailure = 3;
#endif
      OSReport("HorizonWiiU: world init failed: %s\n", error);
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }
   const int32_t centerChunkX = spawn[0] / 16 - (spawn[0] < 0 && spawn[0] % 16 != 0);
   const int32_t centerChunkZ = spawn[1] / 16 - (spawn[1] < 0 && spawn[1] % 16 != 0);
   sRuntimeWorldChunk = horizon_wiiu_world_generate_region(
      centerChunkX, centerChunkZ, HORIZON_WIIU_WORLD_REGION_RADIUS,
      error, sizeof(error));
   if (!sRuntimeWorldChunk) {
#ifdef HORIZON_WIIU_WORLDGEN
      sBlockInitFailure = 4;
#endif
      OSReport("HorizonWiiU: renderer diagnostic region (%d,%d) failed: %s\n",
               centerChunkX, centerChunkZ, error);
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }
   int32_t regionOrigin[2] = {0, 0};
   if (!horizon_wiiu_world_chunk_position(sRuntimeWorldChunk, regionOrigin)) {
#ifdef HORIZON_WIIU_WORLDGEN
      sBlockInitFailure = 6;
#endif
      OSReport("HorizonWiiU: could not read generated Horizon region origin\n");
      horizon_wiiu_world_free_chunk(sRuntimeWorldChunk);
      sRuntimeWorldChunk = NULL;
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }
   int32_t spawnSurfaceY = 0;
   sBlockSection = horizon_wiiu_world_chunk_vertices(
      sRuntimeWorldChunk, &sBlockSectionVertexCount);
   if (!sBlockSection || sBlockSectionVertexCount == 0
       || (sBlockSectionVertexCount % 4) != 0
       || !horizon_wiiu_world_chunk_diagnostic_surface_y(
          sRuntimeWorldChunk, &spawnSurfaceY)) {
#ifdef HORIZON_WIIU_WORLDGEN
      sBlockInitFailure = 5;
#endif
      OSReport("HorizonWiiU: could not read diagnostic mesh or spawn surface\n");
      horizon_wiiu_world_free_chunk(sRuntimeWorldChunk);
      sRuntimeWorldChunk = NULL;
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }
   sBlockOutputCapacity = sBlockSectionVertexCount / 4 * 6;
   sSectionOrigin[0] = (float)(regionOrigin[0] * 16);
   sSectionOrigin[1] = 0.0f;
   sSectionOrigin[2] = (float)(regionOrigin[1] * 16);
   sCameraX = (float)spawn[0] + 0.5f;
   sCameraZ = (float)spawn[1] + 0.5f;
   sCameraY = (float)spawnSurfaceY + HORIZON_WIIU_DIAGNOSTIC_CAMERA_MARGIN;
   sCameraPitch = 8.0f;
   OSReport("HorizonWiiU: generated renderer diagnostic region at (%d,%d), %u vertices, anchor (%d,%d)\n",
            regionOrigin[0], regionOrigin[1], sBlockSectionVertexCount, spawn[0], spawn[1]);
#else
   sBlockSection = sRuntimeChunk;
   sSectionOrigin[0] = 0.0f;
   sSectionOrigin[1] = 64.0f;
   sSectionOrigin[2] = 0.0f;
   sBlockSectionVertexCount = horizon_wiiu_build_runtime_chunk(
      sRuntimeChunk, WORLD_VERTEX_CAPACITY,
      sGroundHeights, GROUND_HEIGHT_COUNT, INT64_C(12345));
   if (sBlockSectionVertexCount == 0 || sBlockSectionVertexCount > WORLD_VERTEX_CAPACITY
       || (sBlockSectionVertexCount % 4) != 0) {
      OSReport("HorizonWiiU: runtime chunk generation failed (%u vertices)\n",
               sBlockSectionVertexCount);
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }
   OSReport("HorizonWiiU: generated chunk (-1,-1), %u exposed-face vertices\n",
            sBlockSectionVertexCount);
   sBlockOutputCapacity = sBlockSectionVertexCount / 4 * 6;
   sCameraX = -8.0f;
   sCameraZ = -2.0f;
   sCameraY = (float)sGroundHeights[14 * 16 + 8] + 1.62f;
   sCameraPitch = 8.0f;
#endif
#endif

   if (sBlockOutputCapacity == 0 || sBlockOutputCapacity > UINT32_MAX / sizeof(Vertex)) {
#ifdef HORIZON_WIIU_WORLDGEN
      sBlockInitFailure = 7;
#endif
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }

   sBlockBuffer.flags = GX2R_RESOURCE_BIND_VERTEX_BUFFER
                        | GX2R_RESOURCE_USAGE_CPU_READ
                        | GX2R_RESOURCE_USAGE_CPU_WRITE
                        | GX2R_RESOURCE_USAGE_GPU_READ;
   sBlockBuffer.elemSize = sizeof(Vertex);
   sBlockBuffer.elemCount = sBlockOutputCapacity;

   if (!GX2RCreateBuffer(&sBlockBuffer)) {
#ifdef HORIZON_WIIU_WORLDGEN
      sBlockInitFailure = 8;
#endif
      WHBGfxFreeShaderGroup(&sBlockShader);
      return FALSE;
   }

   return TRUE;
}

#ifdef HORIZON_WIIU_WORLDGEN
#if HORIZON_WIIU_WORLD_REGION_RADIUS > 0
static int32_t
world_chunk_from_coordinate(float coordinate)
{
   const float scaled = coordinate / 16.0f;
   int32_t chunk = (int32_t)scaled;
   if (scaled < (float)chunk) {
      --chunk;
   }
   return chunk;
}

static BOOL
install_runtime_world_region(void *newChunk, int32_t centerChunkX, int32_t centerChunkZ)
{
   if (!newChunk) {
      OSReport("HorizonWiiU: region reload (%d,%d) returned no data\n",
               centerChunkX, centerChunkZ);
      sRegionReloadCooldown = 120;
      return FALSE;
   }

   uint32_t vertexCount = 0;
   const HorizonTerrainVertex *vertices =
      horizon_wiiu_world_chunk_vertices(newChunk, &vertexCount);
   int32_t regionOrigin[2] = {0, 0};
   if (!vertices || vertexCount == 0 || (vertexCount % 4) != 0
       || !horizon_wiiu_world_chunk_position(newChunk, regionOrigin)
       || vertexCount / 4 > UINT32_MAX / 6) {
      OSReport("HorizonWiiU: region reload (%d,%d) returned invalid mesh data\n",
               centerChunkX, centerChunkZ);
      horizon_wiiu_world_free_chunk(newChunk);
      sRegionReloadCooldown = 120;
      return FALSE;
   }

   const uint32_t requiredCapacity = vertexCount / 4 * 6;
   if (requiredCapacity > sBlockBuffer.elemCount) {
      GX2RBuffer replacement = {0};
      replacement.flags = sBlockBuffer.flags;
      replacement.elemSize = sizeof(Vertex);
      replacement.elemCount = requiredCapacity;
      if (!GX2RCreateBuffer(&replacement)) {
         OSReport("HorizonWiiU: could not grow terrain buffer to %u vertices\n",
                  requiredCapacity);
         horizon_wiiu_world_free_chunk(newChunk);
         sRegionReloadCooldown = 120;
         return FALSE;
      }
      GX2DrawDone();
      GX2RDestroyBufferEx(&sBlockBuffer, sBlockBuffer.flags);
      sBlockBuffer = replacement;
      sBlockOutputCapacity = requiredCapacity;
   }

   void *oldChunk = sRuntimeWorldChunk;
   sRuntimeWorldChunk = newChunk;
   sBlockSection = vertices;
   sBlockSectionVertexCount = vertexCount;
   sSectionOrigin[0] = (float)(regionOrigin[0] * 16);
   sSectionOrigin[1] = 0.0f;
   sSectionOrigin[2] = (float)(regionOrigin[1] * 16);
   if (oldChunk) {
      horizon_wiiu_world_free_chunk(oldChunk);
   }
   sRegionReloadCooldown = 0;
   sBlockMeshDirty = TRUE;
   OSReport("HorizonWiiU: region moved to (%d,%d), %u vertices\n",
            regionOrigin[0], regionOrigin[1], vertexCount);
   return TRUE;
}
#endif

static void
stream_runtime_world_region(void)
{
#if HORIZON_WIIU_WORLD_REGION_RADIUS > 0
   if (sRegionLoadPending) {
      char error[256] = {0};
      void *newChunk = NULL;
      const int32_t result = horizon_wiiu_world_take_region_result(
         &newChunk, error, sizeof(error));
      if (result == 0) {
         return;
      }
      sRegionLoadPending = FALSE;
      if (result < 0 || !newChunk) {
         OSReport("HorizonWiiU: region reload (%d,%d) failed: %s\n",
                  sPendingRegionCenterX, sPendingRegionCenterZ, error);
         sRegionReloadCooldown = 120;
         return;
      }
      install_runtime_world_region(newChunk,
                                   sPendingRegionCenterX, sPendingRegionCenterZ);
      return;
   }

   if (sRegionReloadCooldown != 0) {
      --sRegionReloadCooldown;
      return;
   }
   const float minX = sSectionOrigin[0] + 0.5f;
   const float maxX = sSectionOrigin[0] + HORIZON_WIIU_WORLD_REGION_DIAMETER - 0.5f;
   const float minZ = sSectionOrigin[2] + 0.5f;
   const float maxZ = sSectionOrigin[2] + HORIZON_WIIU_WORLD_REGION_DIAMETER - 0.5f;
   int32_t centerChunkX = (int32_t)sSectionOrigin[0] / 16
                          + HORIZON_WIIU_WORLD_REGION_RADIUS;
   int32_t centerChunkZ = (int32_t)sSectionOrigin[2] / 16
                          + HORIZON_WIIU_WORLD_REGION_RADIUS;

   if (sCameraX < minX + 4.0f || sCameraX > maxX - 4.0f
       || sCameraZ < minZ + 4.0f || sCameraZ > maxZ - 4.0f) {
      centerChunkX = world_chunk_from_coordinate(sCameraX);
      centerChunkZ = world_chunk_from_coordinate(sCameraZ);
   }

   const int32_t currentCenterX = (int32_t)sSectionOrigin[0] / 16
                                  + HORIZON_WIIU_WORLD_REGION_RADIUS;
   const int32_t currentCenterZ = (int32_t)sSectionOrigin[2] / 16
                                  + HORIZON_WIIU_WORLD_REGION_RADIUS;
   if (centerChunkX != currentCenterX || centerChunkZ != currentCenterZ) {
      char error[256] = {0};
      const int32_t requested = horizon_wiiu_world_request_region(
         centerChunkX, centerChunkZ, HORIZON_WIIU_WORLD_REGION_RADIUS,
         error, sizeof(error));
      if (requested < 0) {
         OSReport("HorizonWiiU: could not start region reload (%d,%d): %s\n",
                  centerChunkX, centerChunkZ, error);
         sRegionReloadCooldown = 120;
      } else {
         sRegionLoadPending = TRUE;
         sPendingRegionCenterX = centerChunkX;
         sPendingRegionCenterZ = centerChunkZ;
         if (requested > 0) {
            OSReport("HorizonWiiU: generating region around (%d,%d)\n",
                     centerChunkX, centerChunkZ);
         }
      }
   }
#endif
}
#endif

static void
update_camera(void)
{
   const float previousCameraX = sCameraX;
   const float previousCameraY = sCameraY;
   const float previousCameraZ = sCameraZ;
   const float previousCameraYaw = sCameraYaw;
   const float previousCameraPitch = sCameraPitch;
   HorizonVpadBatch batch;
   if (horizon_wiiu_vpad_read(&batch) != 1) {
      return;
   }
   HorizonVpadSample status = batch.latest;

   const float deadzone = 0.12f;
   float forward = 0.0f;
   float strafe = 0.0f;
   if (status.left_x > deadzone || status.left_x < -deadzone) {
      strafe += status.left_x * 0.025f;
   }
   if (status.left_y > deadzone || status.left_y < -deadzone) {
      forward += status.left_y * 0.025f;
   }
   if (status.buttons & VPAD_BUTTON_LEFT) {
      strafe -= 0.025f;
   }
   if (status.buttons & VPAD_BUTTON_RIGHT) {
      strafe += 0.025f;
   }
   if (status.buttons & VPAD_BUTTON_UP) {
      forward += 0.025f;
   }
   if (status.buttons & VPAD_BUTTON_DOWN) {
      forward -= 0.025f;
   }

   if (status.right_x > deadzone || status.right_x < -deadzone) {
      sCameraYaw += status.right_x * 2.0f;
   }
   if (status.right_y > deadzone || status.right_y < -deadzone) {
      sCameraPitch -= status.right_y * 2.0f;
   }
   if (sCameraYaw >= 360.0f) sCameraYaw -= 360.0f;
   if (sCameraYaw < 0.0f) sCameraYaw += 360.0f;
   if (sCameraPitch < -89.0f) sCameraPitch = -89.0f;
   if (sCameraPitch > 89.0f) sCameraPitch = 89.0f;

   float movement[2];
   horizon_wiiu_camera_motion(sCameraYaw, forward, strafe, movement);
   float candidateCameraX = sCameraX + movement[0];
   float candidateCameraZ = sCameraZ + movement[1];

#ifdef HORIZON_WIIU_CAPTURE
   if (candidateCameraX < -4.0f) candidateCameraX = -4.0f;
   if (candidateCameraX > 4.0f) candidateCameraX = 4.0f;
   if (candidateCameraZ < -4.0f) candidateCameraZ = -4.0f;
   if (candidateCameraZ > 4.0f) candidateCameraZ = 4.0f;
   sCameraX = candidateCameraX;
   sCameraZ = candidateCameraZ;
#else
   /* Renderer diagnostics use free-fly controls: A/B raise or lower the camera. */
   sCameraX = candidateCameraX;
   sCameraZ = candidateCameraZ;
   if (status.buttons & VPAD_BUTTON_A) sCameraY += 0.05f;
   if (status.buttons & VPAD_BUTTON_B) sCameraY -= 0.05f;
#ifdef HORIZON_WIIU_WORLDGEN
   stream_runtime_world_region();
#endif
#endif
   if (sCameraX != previousCameraX || sCameraZ != previousCameraZ
       || sCameraY != previousCameraY || sCameraYaw != previousCameraYaw
       || sCameraPitch != previousCameraPitch) {
      sBlockMeshDirty = TRUE;
   }
}

static BOOL
update_block_mesh(void)
{
   void *buffer = GX2RLockBufferEx(&sBlockBuffer, 0);
   if (!buffer) {
      return FALSE;
   }

   const float camera[9] = {
      sCameraX, sCameraY, sCameraZ, sCameraYaw, sCameraPitch,
      sFocalScale, sAspect, 0.05f, 256.0f,
   };
#ifdef HORIZON_WIIU_CAPTURE
   const uint32_t perspective = 1;
#else
   const uint32_t perspective = 1;
#endif
   uint32_t vertexCount = horizon_wiiu_encode_terrain_quads(sBlockSection,
                                                             sBlockSectionVertexCount,
                                                             (float *)buffer,
                                                             sBlockOutputCapacity,
                                                             sSectionOrigin[0],
                                                             sSectionOrigin[1],
                                                             sSectionOrigin[2],
                                                             camera,
                                                             perspective);
   GX2RUnlockBufferEx(&sBlockBuffer, 0);
   if (vertexCount == 0 || vertexCount > sBlockOutputCapacity) {
      return FALSE;
   }

   sBlockVertexCount = vertexCount;
   return TRUE;
}

static void
free_block_atlas(void)
{
   if (sBlockAtlas.surface.image) {
      const GX2RResourceFlags textureFlags = GX2R_RESOURCE_BIND_TEXTURE
                                             | GX2R_RESOURCE_USAGE_CPU_WRITE
                                             | GX2R_RESOURCE_USAGE_GPU_READ;
      GX2RDestroySurfaceEx(&sBlockAtlas.surface, textureFlags);
      memset(&sBlockAtlas, 0, sizeof(sBlockAtlas));
   }
}

static void
free_lightmap(void)
{
   if (sLightmap.surface.image) {
      const GX2RResourceFlags textureFlags = GX2R_RESOURCE_BIND_TEXTURE
                                             | GX2R_RESOURCE_USAGE_CPU_WRITE
                                             | GX2R_RESOURCE_USAGE_GPU_READ;
      GX2RDestroySurfaceEx(&sLightmap.surface, textureFlags);
      memset(&sLightmap, 0, sizeof(sLightmap));
   }
}

static void
draw_block_mesh(void)
{
   GX2SetVertexShader(sBlockShader.vertexShader);
   GX2SetPixelShader(sBlockShader.pixelShader);
   GX2SetFetchShader(&sBlockShader.fetchShader);
   GX2RSetAttributeBuffer(&sBlockBuffer, 0, sizeof(Vertex), 0);
   GX2SetPixelTexture(&sBlockAtlas, 0);
   GX2SetPixelSampler(&sBlockSampler, 0);
   GX2SetPixelTexture(&sLightmap, 1);
   GX2SetPixelSampler(&sLightmapSampler, 1);
   GX2SetDepthOnlyControl(TRUE, TRUE, GX2_COMPARE_FUNC_LEQUAL);
   GX2DrawEx(GX2_PRIMITIVE_MODE_TRIANGLES, sBlockVertexCount, 0, 1);
}

static void
free_block_mesh(void)
{
   if (GX2RBufferExists(&sBlockBuffer)) {
      GX2RDestroyBufferEx(&sBlockBuffer, sBlockBuffer.flags);
   }
#ifdef HORIZON_WIIU_WORLDGEN
   if (sRuntimeWorldChunk) {
      horizon_wiiu_world_free_chunk(sRuntimeWorldChunk);
      sRuntimeWorldChunk = NULL;
   }
#endif
   WHBGfxFreeShaderGroup(&sBlockShader);
}

int
main(int argc, char **argv)
{
   (void)argc;
   (void)argv;

   OSReport("HorizonWiiU: Cafe entrypoint reached\n");
   WHBProcInit();
#ifdef HORIZON_WIIU_WORLDGEN
   horizon_wiiu_log_init();
#endif
   OSReport("HorizonWiiU: process services initialized\n");
   if (!WHBGfxInit()) {
      OSReport("HorizonWiiU: GX2 initialization failed\n");
      WHBProcShutdown();
      return 1;
   }
   OSReport("HorizonWiiU: GX2 initialization completed\n");

   OSReport("HorizonWiiU: renderer/world initialization started\n");
   if (!init_block_mesh()) {
#ifdef HORIZON_WIIU_WORLDGEN
      OSReport("HorizonWiiU: renderer/world initialization failed (%d)\n",
               sBlockInitFailure);
#else
      OSReport("HorizonWiiU: renderer/world initialization failed\n");
#endif
      WHBGfxShutdown();
      WHBProcShutdown();
#ifdef HORIZON_WIIU_WORLDGEN
      return 40 + sBlockInitFailure;
#else
      return 4;
#endif
   }

   if (!init_block_atlas()) {
      free_block_mesh();
      WHBGfxShutdown();
      WHBProcShutdown();
      return 2;
   }

   if (!init_lightmap()) {
      free_block_atlas();
      free_block_mesh();
      WHBGfxShutdown();
      WHBProcShutdown();
      return 3;
   }
   OSReport("HorizonWiiU: renderer resources initialized\n");

   uint32_t frame = 0;
   while (WHBProcIsRunning()) {
      float rgba[4];
      horizon_wiiu_frame_color(frame++, rgba);
      update_camera();
      if (sBlockMeshDirty && update_block_mesh()) {
         sBlockMeshDirty = FALSE;
      }
      const BOOL hasMesh = sBlockVertexCount != 0;

      WHBGfxBeginRender();
      WHBGfxBeginRenderTV();
      WHBGfxClearColor(rgba[0], rgba[1], rgba[2], rgba[3]);
      if (hasMesh) draw_block_mesh();
      WHBGfxFinishRenderTV();

      WHBGfxBeginRenderDRC();
      WHBGfxClearColor(rgba[0], rgba[1], rgba[2], rgba[3]);
      if (hasMesh) draw_block_mesh();
      WHBGfxFinishRenderDRC();
      WHBGfxFinishRender();
   }

   free_block_mesh();
   free_lightmap();
   free_block_atlas();
   WHBGfxShutdown();
   WHBProcShutdown();
   return 0;
}
