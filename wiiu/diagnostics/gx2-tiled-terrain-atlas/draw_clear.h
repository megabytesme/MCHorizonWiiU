#pragma once

extern const uint8_t _binary_test_color_gsh_start[];

typedef struct {
   WHBGfxShaderGroup group;
   float (*positions)[4];
   uint32_t clip[4];
   uint32_t color;
} DrawClear;

static bool draw_clear_init(DrawClear *clear)
{
   static const char *const clip[] = {"uClipX", "uClipY", "uClipZ", "uClipW"};
   static const float quad[4][4] = {
      {-1,-1,.95f,0}, {-1,1,.95f,0}, {1,1,.95f,0}, {1,-1,.95f,0},
   };
   if (!WHBGfxLoadGFDShaderGroup(&clear->group, 0, _binary_test_color_gsh_start) ||
       !WHBGfxInitShaderAttribute(&clear->group, "aPosition", 0, 0, GX2_ATTRIB_FORMAT_FLOAT_32_32_32) ||
       !WHBGfxInitFetchShader(&clear->group)) return false;
   for (unsigned i = 0; i < 4; ++i) {
      const GX2UniformVar *uniform = GX2GetVertexUniformVar(clear->group.vertexShader, clip[i]);
      if (!uniform) return false;
      clear->clip[i] = uniform->offset;
   }
   const GX2UniformVar *color = GX2GetPixelUniformVar(clear->group.pixelShader, "uColor");
   if (!color) return false;
   clear->color = color->offset;
   clear->positions = MEMAllocFromDefaultHeapEx(sizeof(quad), 64);
   if (!clear->positions) return false;
   memcpy(clear->positions, quad, sizeof(quad));
   DCFlushRange(clear->positions, sizeof(quad));
   return true;
}

/* Stamp a known old color after the ordinary clear. The control omits only
 * the black draw so that a working hardware clear cannot fake its result. */
static void draw_clear_output(const DrawClear *clear, HorizonGx2Terrain *terrain,
                              unsigned w, unsigned h, bool omit_black)
{
   static const _Alignas(16) float identity[16] = {
      1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1,
   };
   static const _Alignas(16) float poison[4] = {1,0,1,1};
   static const _Alignas(16) float black[4] = {0,0,0,1};
   GX2SetViewport(0,0,w,h,0,1);
   GX2SetScissor(0,0,w,h);
   GX2SetRasterizerClipControlEx(TRUE,TRUE,TRUE);
   GX2SetTargetChannelMasks(GX2_CHANNEL_MASK_RGBA,0,0,0,0,0,0,0);
   GX2SetShaderMode(GX2_SHADER_MODE_UNIFORM_REGISTER);
   GX2SetFetchShader(&clear->group.fetchShader);
   GX2SetVertexShader(clear->group.vertexShader);
   GX2SetPixelShader(clear->group.pixelShader);
   for (unsigned i = 0; i < 4; ++i)
      GX2SetVertexUniformReg(clear->clip[i],4,identity + 4*i);
   GX2SetAttribBuffer(0,4*sizeof(*clear->positions),sizeof(*clear->positions),clear->positions);
   GX2SetAlphaTest(FALSE,GX2_COMPARE_FUNC_ALWAYS,0);
   GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW,FALSE,FALSE);
   GX2SetDepthOnlyControl(TRUE,TRUE,GX2_COMPARE_FUNC_ALWAYS);
   GX2SetColorControl(GX2_LOGIC_OP_COPY,0,FALSE,TRUE);
   GX2SetPixelUniformReg(clear->color,4,poison);
   horizon_gx2_terrain_draw_quads(terrain,4);
   if (!omit_black) {
      GX2SetPixelUniformReg(clear->color,4,black);
      horizon_gx2_terrain_draw_quads(terrain,4);
   }
}
