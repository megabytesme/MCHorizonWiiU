typedef struct {
   WHBGfxShaderGroup shader;
   float *vertices;
   uint32_t clip[4], color;
} Clear;

static bool clear_init(Clear *clear)
{
   WHBGfxShaderGroup *shader = &clear->shader;
   if (!WHBGfxLoadGFDShaderGroup(shader, 0, _binary_clear_gsh_start) ||
       !WHBGfxInitShaderAttribute(shader, "aPosition", 0, 0, GX2_ATTRIB_FORMAT_FLOAT_32_32_32) ||
       !WHBGfxInitFetchShader(shader)) return false;
   static const char *const names[4] = {"uClipX", "uClipY", "uClipZ", "uClipW"};
   for (unsigned i = 0; i < 4; ++i) {
      const GX2UniformVar *var = GX2GetVertexUniformVar(shader->vertexShader, names[i]);
      if (!var) return false;
      clear->clip[i] = var->offset;
   }
   const GX2UniformVar *var = GX2GetPixelUniformVar(shader->pixelShader, "uColor");
   if (!var) return false;
   clear->color = var->offset;
   static const float verts[16] = {-1,-1,1,0, -1,1,1,0, 1,1,1,0, 1,-1,1,0};
   clear->vertices = MEMAllocFromDefaultHeapEx(sizeof(verts), 64);
   if (!clear->vertices) return false;
   memcpy(clear->vertices, verts, sizeof(verts));
   DCFlushRange(clear->vertices, sizeof(verts));
   return true;
}

static void draw_clear(const Clear *clear, HorizonGx2Terrain *terrain, unsigned w, unsigned h)
{
   static const _Alignas(16) float identity[16] = {1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1};
   static const _Alignas(16) float black[4] = {0,0,0,1};
   GX2SetViewport(0, 0, w, h, 0, 1);
   GX2SetScissor(0, 0, w, h);
   GX2SetShaderMode(GX2_SHADER_MODE_UNIFORM_REGISTER);
   GX2SetFetchShader(&clear->shader.fetchShader);
   GX2SetVertexShader(clear->shader.vertexShader);
   GX2SetPixelShader(clear->shader.pixelShader);
   for (unsigned i = 0; i < 4; ++i)
      GX2SetVertexUniformReg(clear->clip[i], 4, identity + i * 4);
   GX2SetPixelUniformReg(clear->color, 4, black);
   GX2SetAttribBuffer(0, 16 * sizeof(float), 4 * sizeof(float), clear->vertices);
   GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW, FALSE, FALSE);
   GX2SetAlphaTest(FALSE, GX2_COMPARE_FUNC_ALWAYS, 0);
   GX2SetDepthOnlyControl(TRUE, TRUE, GX2_COMPARE_FUNC_ALWAYS);
   GX2SetColorControl(GX2_LOGIC_OP_COPY, 0, FALSE, TRUE);
   GX2SetTargetChannelMasks((GX2ChannelMask)15, 0, 0, 0, 0, 0, 0, 0);
   horizon_gx2_terrain_draw_quads(terrain, 4);
}
