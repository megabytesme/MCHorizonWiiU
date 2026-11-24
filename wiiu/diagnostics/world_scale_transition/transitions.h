/* Included after the production world scale code and pattern helper. */

static bool transition_complete(void)
{
   GX2Flush();
   const OSTime submitted = GX2GetLastSubmittedTimeStamp();
   GX2WaitTimeStamp(submitted);
   return GX2GetRetiredTimeStamp() >= submitted;
}

static bool matches_pattern(const uint8_t *top, const uint8_t *bottom, bool reverse)
{
   const unsigned red = reverse ? 2 : 0, blue = 2 - red;
   return top[red] >= 245 && top[1] <= 10 && top[blue] <= 10 &&
      bottom[red] <= 10 && bottom[1] <= 10 && bottom[blue] >= 245;
}

static void log_transition(const char *view, unsigned trial, unsigned target,
                           const uint8_t *top, const uint8_t *bottom, bool ok)
{
   OSReport("GX2 scale %s trial=%u target=%u top=%u,%u,%u bot=%u,%u,%u ok=%u\n",
            view, trial, target, top[0], top[1], top[2], bottom[0], bottom[1], bottom[2],
            (unsigned)ok);
}

static bool output_pixels(GX2Surface *surface, unsigned trial, unsigned target, bool reverse)
{
   const uint32_t w = surface->width, h = surface->height;
   const size_t bytes = (size_t)w * h * 4;
   uint8_t *pixels = malloc(bytes);
   if (!pixels) return false;
   const bool read = horizon_loading_host_read_rgba(target, pixels, bytes) == 0;
   bool ok = false;
   if (read) {
      const uint8_t *top = pixels + ((size_t)(h / 4) * w + w / 2) * 4;
      const uint8_t *bottom = pixels + ((size_t)(3 * h / 4) * w + w / 2) * 4;
      ok = matches_pattern(top, bottom, reverse);
      log_transition("output", trial, target, top, bottom, ok);
   }
   free(pixels);
   return ok;
}

static bool source_pixels(GX2Surface *source, unsigned trial, unsigned target, bool reverse)
{
   GX2Surface copy = world_surface(source->width, source->height, source->format);
   copy.tileMode = GX2_TILE_MODE_LINEAR_SPECIAL;
   const GX2RResourceFlags flags = GX2R_RESOURCE_BIND_TEXTURE |
      GX2R_RESOURCE_USAGE_CPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE;
   if (!GX2RCreateSurface(&copy, flags)) return false;
   GX2CopySurface(source, 0, 0, &copy, 0, 0);
   bool ok = false;
   if (transition_complete()) {
      const uint8_t *pixels = GX2RLockSurfaceEx(&copy, 0, flags);
      if (pixels) {
         const uint8_t *top = pixels + ((size_t)(copy.height / 4) * copy.pitch + copy.width / 2) * 4;
         const uint8_t *bottom = pixels + ((size_t)(3 * copy.height / 4) * copy.pitch + copy.width / 2) * 4;
         ok = matches_pattern(top, bottom, reverse);
         log_transition("source", trial, target, top, bottom, ok);
         GX2RUnlockSurfaceEx(&copy, 0, flags);
      }
   }
   GX2RDestroySurfaceEx(&copy, 0);
   return ok;
}

static bool consume_target(HorizonGx2WorldScale *state, unsigned target, const uint32_t rect[4],
                           GX2Surface *copy)
{
   const GX2InvalidateMode both = GX2_INVALIDATE_MODE_COLOR_BUFFER | GX2_INVALIDATE_MODE_TEXTURE;
   WorldTarget *world = &state->outputs[target];
   GX2Surface *source = &world->color.surface;
   if (transition_mode != RC26_ORDER) {
      const GX2InvalidateMode modes = transition_mode == SPLIT_SYNC ? GX2_INVALIDATE_MODE_COLOR_BUFFER : both;
      GX2Invalidate(modes, source->image, source->imageSize);
   }
   if (transition_mode == DRAINED_SYNC && !transition_complete()) return false;
   if (transition_mode == COPY_TEXTURE) {
      GX2CopySurface(source, 0, 0, copy, 0, 0);
      GX2Invalidate(GX2_INVALIDATE_MODE_TEXTURE, copy->image, copy->imageSize);
      world->texture.surface = *copy;
      GX2InitTextureRegs(&world->texture);
   }

   horizon_gx2_world_scale_end(state, target, rect, true);
   if (transition_mode == COPY_TEXTURE) {
      world->texture.surface = *source;
      GX2InitTextureRegs(&world->texture);
   }
   return true;
}

static bool check_transitions(GX2Surface *outputs[2], const GX2Texture *pattern)
{
   GX2Texture reversed = {0};
   if (!reversed_texture(&reversed)) return false;
   const uint32_t tv_w = outputs[0]->width, tv_h = outputs[0]->height;
   const uint32_t drc_w = outputs[1]->width, drc_h = outputs[1]->height;
   // Match the fitted RC 26 viewport at 1080p and the standard GamePad size.
   const uint32_t rects[2][4] = {{0, 0, tv_w, tv_h == 1080 ? 1079 : tv_h}, {0, 0, drc_w, drc_h}};
   const uint32_t sizes[4] = {(rects[0][2] * 75 + 50) / 100, (rects[0][3] * 75 + 50) / 100,
                              (rects[1][2] * 75 + 50) / 100, (rects[1][3] * 75 + 50) / 100};
   uint32_t bytes;
   HorizonGx2WorldScale *state = horizon_gx2_world_scale_create(sizes, &bytes);
   if (!state) {
      GX2RDestroySurfaceEx(&reversed.surface, 0);
      return false;
   }
   OSReport("GX2 transitions TV=%ux%u DRC=%ux%u bytes=%u\n", sizes[0], sizes[1], sizes[2], sizes[3], bytes);
   // Direct WHB control with the same shader and vertices, no attachment alias.
   bool passed = true;
   WHBGfxBeginRender();
   for (unsigned target = 0; target < 2; ++target) {
      if (target == 0) WHBGfxBeginRenderTV();
      else WHBGfxBeginRenderDRC();
      WHBGfxClearColor(0.2f, 0.2f, 0.2f, 1.0f);
      draw_pattern(state, pattern, 0, 0, rects[target][2], rects[target][3]);
      const bool read_ok = output_pixels(outputs[target], 6, target, false);
      OSReport("GX2 scale direct target=%u ok=%u\n", target, (unsigned)read_ok);
      passed &= read_ok;
      if (target == 0) WHBGfxFinishRenderTV();
      else WHBGfxFinishRenderDRC();
   }
   WHBGfxFinishRender();

   const enum TransitionMode modes[] = {RC26_ORDER, BEFORE_RESTORE, SPLIT_SYNC, DRAINED_SYNC, COPY_TEXTURE, RC26_ORDER};
   const char *names[] = {"rc26", "before-restore", "split-sync", "drained", "copy-texture"};
   for (unsigned trial = 0; trial < sizeof(modes) / sizeof(modes[0]); ++trial) {
      const bool reverse = (trial & 1) != 0;
      transition_mode = modes[trial];
      OSReport("GX2 scale transition trial=%u mode=%s reverse=%u\n", trial, names[modes[trial]], (unsigned)reverse);
      WHBGfxBeginRender();
      for (unsigned target = 0; target < 2; ++target) {
         GX2Surface copy = {0};
         if (transition_mode == COPY_TEXTURE) {
            copy = world_surface(sizes[target * 2], sizes[target * 2 + 1], GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8);
            copy.tileMode = GX2_TILE_MODE_LINEAR_ALIGNED;
            if (!GX2RCreateSurface(&copy, GX2R_RESOURCE_BIND_TEXTURE |
                  GX2R_RESOURCE_USAGE_GPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE)) {
               OSReport("GX2 scale transition copy alloc failed target=%u\n", target);
               passed = false;
               continue;
            }
         }
         if (target == 0) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         WHBGfxClearColor(0.2f, 0.2f, 0.2f, 1.0f);
         horizon_gx2_world_scale_begin(state, target, (float[]){0.0f, 1.0f, 0.0f, 1.0f});
         draw_pattern(state, reverse ? &reversed : pattern, 0, 0, sizes[target * 2], sizes[target * 2 + 1]);
         const bool submitted = consume_target(state, target, rects[target], &copy);
         // Read the destination before copying the source for CPU inspection.
         // Reading the source first would introduce another synchronization.
         const bool presented = submitted && output_pixels(outputs[target], trial, target, reverse);
         const bool source = source_pixels(&state->outputs[target].color.surface, trial, target, reverse);
         if (copy.image) GX2RDestroySurfaceEx(&copy, 0);
         passed &= source && presented;
         if (target == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
   }
   transition_mode = RC26_ORDER;
   horizon_gx2_world_scale_destroy(state);
   GX2RDestroySurfaceEx(&reversed.surface, 0);
   return passed;
}
