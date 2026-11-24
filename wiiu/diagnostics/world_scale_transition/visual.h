/* The physical fixture checks scan-out with projector and GamePad stills.
 * It never maps or reads a render surface from the CPU. */

#ifdef HORIZON_SCALE_VISUAL_STORAGE
enum StorageTrial { STANDARD_TARGET, NO_COLOR_CLEAR, KEEP_CONTEXT, COPY_TARGET, RESET_AFTER_CLEAR };

static void composite_copy(HorizonGx2WorldScale *state, unsigned target,
                           const uint32_t rect[4], GX2Texture *copy)
{
   WorldTarget *output = &state->outputs[target];
   GX2Surface *source = &output->color.surface;
   horizon_gx2_world_scale_end(state, target, rect, false);
   GX2Invalidate(GX2_INVALIDATE_MODE_COLOR_BUFFER, source->image, source->imageSize);
   GX2CopySurface(source, 0, 0, &copy->surface, 0, 0);
   GX2Invalidate(GX2_INVALIDATE_MODE_TEXTURE, copy->surface.image, copy->surface.imageSize);
   const GX2Texture original = output->texture;
   output->texture = *copy;
   horizon_gx2_world_scale_end(state, target, rect, true);
   output->texture = original;
}

static const GX2RResourceFlags copy_flags = GX2R_RESOURCE_BIND_TEXTURE |
   GX2R_RESOURCE_USAGE_GPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE;

static void free_copies(GX2Texture copies[2])
{
   for (unsigned i = 0; i < 2; ++i) {
      if (copies[i].surface.image) GX2RDestroySurfaceEx(&copies[i].surface, copy_flags);
   }
}

static bool create_copies(const HorizonGx2WorldScale *state, GX2Texture copies[2])
{
   for (unsigned i = 0; i < 2; ++i) {
      copies[i] = state->outputs[i].texture;
      const GX2Surface *color = &state->outputs[i].color.surface;
      copies[i].surface = world_surface(color->width, color->height, color->format);
      if (!GX2RCreateSurface(&copies[i].surface, copy_flags)) {
         free_copies(copies);
         return false;
      }
      GX2InitTextureRegs(&copies[i]);
      OSReport("GX2 scale copy layout target=%u pitch=%u tile=%u\n", i,
               copies[i].surface.pitch, copies[i].surface.tileMode);
   }
   return true;
}
#endif
static bool check_transitions(GX2Surface *outputs[2], const GX2Texture *pattern)
{
   GX2Texture reversed = {0};
   if (!reversed_texture(&reversed)) return false;
   const uint32_t tv_height = outputs[0]->height;
   const uint32_t rects[2][4] = {
      {0, 0, outputs[0]->width, tv_height == 1080 ? 1079 : tv_height},
      {0, 0, outputs[1]->width, outputs[1]->height},
   };
   const uint32_t sizes[4] = {
      (rects[0][2] * 75 + 50) / 100, (rects[0][3] * 75 + 50) / 100,
      (rects[1][2] * 75 + 50) / 100, (rects[1][3] * 75 + 50) / 100,
   };
   uint32_t bytes;
   HorizonGx2WorldScale *state = horizon_gx2_world_scale_create(sizes, &bytes);
   if (!state) {
      GX2RDestroySurfaceEx(&reversed.surface, 0);
      return false;
   }
#ifdef HORIZON_SCALE_VISUAL_STORAGE
   GX2Texture copies[2] = {0};
   if (!create_copies(state, copies)) {
      horizon_gx2_world_scale_destroy(state);
      GX2RDestroySurfaceEx(&reversed.surface, 0);
      return false;
   }
#endif
   OSReport("GX2 transitions TV=%ux%u DRC=%ux%u bytes=%u\n", sizes[0], sizes[1], sizes[2], sizes[3], bytes);
   for (unsigned target = 0; target < 2; ++target) {
      const GX2Surface *color = &state->outputs[target].color.surface;
      const GX2Surface *depth = &state->outputs[target].depth.surface;
      OSReport("GX2 scale layout target=%u color=%u/%u/%u depth=%u/%u/%u\n", target,
               color->pitch, color->tileMode, color->use, depth->pitch, depth->tileMode, depth->use);
   }
   const struct {
      const char *name;
      bool scaled;
      bool reverse;
      bool bind_before_clear;
      bool test_depth;
      GX2CompareFunction compare;
#ifdef HORIZON_SCALE_VISUAL_STORAGE
      enum StorageTrial storage;
#endif
   } trials[] = {
#ifdef HORIZON_SCALE_VISUAL_STORAGE
      {"direct", false, false, false, true, GX2_COMPARE_FUNC_LEQUAL, STANDARD_TARGET},
      {"rc26", true, true, false, true, GX2_COMPARE_FUNC_LEQUAL, STANDARD_TARGET},
      {"depth-off", true, false, false, false, GX2_COMPARE_FUNC_ALWAYS, STANDARD_TARGET},
      {"no-color-clear", true, true, true, false, GX2_COMPARE_FUNC_ALWAYS, NO_COLOR_CLEAR},
      {"keep-context", true, false, false, false, GX2_COMPARE_FUNC_ALWAYS, KEEP_CONTEXT},
      {"copy-target", true, true, false, false, GX2_COMPARE_FUNC_ALWAYS, COPY_TARGET},
      {"reset-after-clear", true, false, false, false, GX2_COMPARE_FUNC_ALWAYS, RESET_AFTER_CLEAR},
      {"rc26-repeat", true, false, false, true, GX2_COMPARE_FUNC_LEQUAL, STANDARD_TARGET},
#else
      {"direct", false, false, false, true, GX2_COMPARE_FUNC_LEQUAL},
      {"rc26", true, true, false, true, GX2_COMPARE_FUNC_LEQUAL},
      {"depth-off", true, false, false, false, GX2_COMPARE_FUNC_ALWAYS},
      {"depth-always", true, true, false, true, GX2_COMPARE_FUNC_ALWAYS},
      {"bind-first", true, false, true, true, GX2_COMPARE_FUNC_LEQUAL},
      {"bind-first-depth-off", true, true, true, false, GX2_COMPARE_FUNC_ALWAYS},
      {"rc26-repeat", true, false, false, true, GX2_COMPARE_FUNC_LEQUAL},
#endif
   };
   bool completed = true;
   for (unsigned trial = 0; trial < sizeof(trials) / sizeof(trials[0]); ++trial) {
      const bool reverse = trials[trial].reverse;
      const OSTime hold_until = OSGetSystemTime() + OSMillisecondsToTicks(7000);
      bool announced = false;
      do {
         if (!WHBProcIsRunning()) {
            completed = false;
            break;
         }
         if (!outputs[0]->image || !outputs[1]->image) {
            OSSleepTicks(OSMillisecondsToTicks(16));
            continue;
         }
         WHBGfxBeginRender();
         for (unsigned target = 0; target < 2; ++target) {
            const uint32_t *rect = rects[target];
            if (target == 0) WHBGfxBeginRenderTV();
            else WHBGfxBeginRenderDRC();
            WHBGfxClearColor(0.2f, 0.2f, 0.2f, 1.0f);
            if (trials[trial].scaled) {
               WorldTarget *output = &state->outputs[target];
               if (trials[trial].bind_before_clear) {
                  GX2SetColorBuffer(&output->color, GX2_RENDER_TARGET_0);
                  GX2SetDepthBuffer(&output->depth);
               }
#ifdef HORIZON_SCALE_VISUAL_STORAGE
               if (trials[trial].storage != NO_COLOR_CLEAR)
#endif
                  horizon_gx2_world_scale_begin(state, target, (float[]){0.0f, 1.0f, 0.0f, 1.0f});
#ifdef HORIZON_SCALE_VISUAL_STORAGE
               if (trials[trial].storage == RESET_AFTER_CLEAR) {
                  GX2SetContextState(target == 0 ? WHBGfxGetTVContextState() : WHBGfxGetDRCContextState());
                  GX2SetColorBuffer(&output->color, GX2_RENDER_TARGET_0);
                  GX2SetDepthBuffer(&output->depth);
               }
#endif
               draw_pattern_with_depth(state, reverse ? &reversed : pattern, 0, 0,
                                       sizes[target * 2], sizes[target * 2 + 1],
                                       trials[trial].test_depth, trials[trial].compare);
#ifdef HORIZON_SCALE_VISUAL_STORAGE
               if (trials[trial].storage == COPY_TARGET) {
                  composite_copy(state, target, rect, &copies[target]);
               } else {
                  restore_context = trials[trial].storage != KEEP_CONTEXT;
                  horizon_gx2_world_scale_end(state, target, rect, true);
                  restore_context = true;
               }
#else
               horizon_gx2_world_scale_end(state, target, rect, true);
#endif
            } else {
               draw_pattern(state, pattern, rect[0], rect[1], rect[2], rect[3]);
            }
            if (target == 0) WHBGfxFinishRenderTV();
            else WHBGfxFinishRenderDRC();
         }
         WHBGfxFinishRender();
         if (!announced) {
            OSReport("GX2 scale still trial=%u mode=%s reverse=%u\n", trial, trials[trial].name, (unsigned)reverse);
            announced = true;
         }
         OSSleepTicks(OSMillisecondsToTicks(32));
      } while (OSGetSystemTime() < hold_until);
      if (!announced || !completed) {
         completed = false;
         break;
      }
   }
#ifdef HORIZON_SCALE_VISUAL_STORAGE
   GX2DrawDone();
   free_copies(copies);
#endif
   horizon_gx2_world_scale_destroy(state);
   GX2RDestroySurfaceEx(&reversed.surface, 0);
   return completed;
}
