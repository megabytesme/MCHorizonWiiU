#include "swkbd_bridge.h"

#include <coreinit/debug.h>
#include <nn/swkbd/swkbd_cpp.h>
#include <vpad/input.h>

#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <malloc.h>

namespace {

enum class ResultState : int32_t { None = 0, Accepted = 1, Cancelled = 2 };

constexpr size_t kMaxTextUnits = 1024;
constexpr size_t kMaxUtf8Bytes = kMaxTextUnits * 4;

bool s_created = false;
bool s_open = false;
bool s_fs_client_registered = false;
bool s_have_last_vpad = false;
uint32_t s_last_hold = 0;
uint16_t s_last_touched = 0;
uint16_t s_last_touch_validity = 0;
ResultState s_result_state = ResultState::None;
char16_t *s_work_memory = nullptr;
size_t s_work_memory_size = 0;
alignas(0x40) FSClient s_fs_client{};
char16_t s_initial_text[kMaxTextUnits + 1]{};
char s_result[kMaxUtf8Bytes + 1]{};
size_t s_result_length = 0;
#ifdef HORIZON_WIIU_TRACE_SWKBD
uint32_t s_trace_calc_calls = 0;
uint32_t s_trace_tv_draw_calls = 0;
uint32_t s_trace_drc_draw_calls = 0;
uint32_t s_trace_calc_until_call = 0;

bool trace_call(uint32_t call) {
   return call <= 96 && (call <= 4 || call % 16 == 0);
}

bool trace_poll(uint32_t call) {
   return call <= 96;
}

void report_stage(uint32_t call, const char *stage) {
   OSReport("Horizon Cafe SWKBD call=%u stage=%s\n", (unsigned)call, stage);
}
#endif

void remember_result(ResultState state) {
   s_result_state = state;
   s_result_length = 0;
   if (state != ResultState::Accepted) {
      s_result[0] = '\0';
      return;
   }

   const char16_t *input = nn::swkbd::GetInputFormString();
   if (!input) {
      s_result_state = ResultState::Cancelled;
      s_result[0] = '\0';
      return;
   }

   // Convert UTF-16 to UTF-8, including supplementary characters. The fixed
   // output is sized for the maximum UTF-16 input accepted by this bridge.
   for (size_t i = 0; i < kMaxTextUnits && input[i] != 0; ++i) {
      uint32_t cp = static_cast<uint16_t>(input[i]);
      if (cp >= 0xD800 && cp <= 0xDBFF && i + 1 < kMaxTextUnits) {
         const uint32_t low = static_cast<uint16_t>(input[i + 1]);
         if (low >= 0xDC00 && low <= 0xDFFF) {
            cp = 0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
            ++i;
         } else {
            cp = 0xFFFD;
         }
      } else if (cp >= 0xDC00 && cp <= 0xDFFF) {
         cp = 0xFFFD;
      }

      if (cp <= 0x7F) {
         s_result[s_result_length++] = static_cast<char>(cp);
      } else if (cp <= 0x7FF) {
         s_result[s_result_length++] = static_cast<char>(0xC0 | (cp >> 6));
         s_result[s_result_length++] = static_cast<char>(0x80 | (cp & 0x3F));
      } else if (cp <= 0xFFFF) {
         s_result[s_result_length++] = static_cast<char>(0xE0 | (cp >> 12));
         s_result[s_result_length++] = static_cast<char>(0x80 | ((cp >> 6) & 0x3F));
         s_result[s_result_length++] = static_cast<char>(0x80 | (cp & 0x3F));
      } else {
         s_result[s_result_length++] = static_cast<char>(0xF0 | (cp >> 18));
         s_result[s_result_length++] = static_cast<char>(0x80 | ((cp >> 12) & 0x3F));
         s_result[s_result_length++] = static_cast<char>(0x80 | ((cp >> 6) & 0x3F));
         s_result[s_result_length++] = static_cast<char>(0x80 | (cp & 0x3F));
      }
   }
   s_result[s_result_length] = '\0';
}

void close_form() {
   if (s_open) {
      nn::swkbd::DisappearInputForm();
      s_open = false;
   }
}

void copy_initial_text(const uint16_t *text, int32_t max_utf16_units) {
   size_t limit = kMaxTextUnits;
   if (max_utf16_units >= 0 && static_cast<size_t>(max_utf16_units) < limit) {
      limit = static_cast<size_t>(max_utf16_units);
   }
   size_t length = 0;
   while (length < limit && text[length] != 0) {
      s_initial_text[length] = static_cast<char16_t>(text[length]);
      ++length;
   }
   // Do not leave a dangling high surrogate if the field text reaches the
   // configured UTF-16-unit limit in the middle of a supplementary scalar.
   if (length > 0 && length == limit && s_initial_text[length - 1] >= 0xD800 &&
       s_initial_text[length - 1] <= 0xDBFF) {
      --length;
   }
   s_initial_text[length] = 0;
}

} // namespace

extern "C" int32_t horizon_swkbd_request(const uint16_t *initial_text,
                                          int32_t max_utf16_units) {
   if (!initial_text || max_utf16_units < -1 ||
       static_cast<size_t>(max_utf16_units < 0 ? 0 : max_utf16_units) >
           kMaxTextUnits) {
      return -1;
   }
   if (s_open) return -2;
   copy_initial_text(initial_text, max_utf16_units);

   if (!s_created) {
      // Cafe Rust std has already initialized the shared filesystem service
      // before the loading app reaches this point; only register our client.
      if (FSAddClient(&s_fs_client, FS_ERROR_FLAG_NONE) != FS_STATUS_OK) {
         return -3;
      }
      s_fs_client_registered = true;
      s_work_memory_size = nn::swkbd::GetWorkMemorySize(0);
      if (!s_work_memory_size) {
         FSDelClient(&s_fs_client, FS_ERROR_FLAG_NONE);
         s_fs_client_registered = false;
         return -4;
      }
      // swkbd requires caller-owned work memory. Keep it 0x100-aligned and
      // alive from Create through Destroy as required by the WUT interface.
      s_work_memory = static_cast<char16_t *>(memalign(0x100, s_work_memory_size));
      if (!s_work_memory) {
         FSDelClient(&s_fs_client, FS_ERROR_FLAG_NONE);
         s_fs_client_registered = false;
         return -5;
      }
      nn::swkbd::CreateArg create_args;
      create_args.workMemory = s_work_memory;
      create_args.regionType = nn::swkbd::RegionType::Europe;
      create_args.fsClient = &s_fs_client;
      if (!nn::swkbd::Create(create_args)) {
         free(s_work_memory);
         s_work_memory = nullptr;
         s_work_memory_size = 0;
         FSDelClient(&s_fs_client, FS_ERROR_FLAG_NONE);
         s_fs_client_registered = false;
         return -6;
      }
      s_created = true;
   }

   nn::swkbd::AppearArg args;
   args.keyboardArg.configArg.controllerType = nn::swkbd::ControllerType::DrcGamepad;
   args.inputFormArg.initialText = s_initial_text;
   // The result bridge uses a fixed-size buffer, so keep the system keyboard
   // within that bound even when the caller requests an unlimited field.
   args.inputFormArg.maxTextLength =
       max_utf16_units < 0 ? static_cast<int32_t>(kMaxTextUnits) : max_utf16_units;
   args.inputFormArg.higlightInitialText = false;
   args.inputFormArg.showCopyPasteButtons = true;
   if (!nn::swkbd::AppearInputForm(args)) return -7;

   s_result_state = ResultState::None;
   s_result_length = 0;
   s_have_last_vpad = false;
   s_open = true;
   return 0;
}

extern "C" int32_t horizon_swkbd_is_open(void) {
   return s_open ? 1 : 0;
}

extern "C" int32_t horizon_swkbd_calc(const void *vpad_status) {
   if (!s_open) return 0;

   auto *status = const_cast<VPADStatus *>(static_cast<const VPADStatus *>(vpad_status));
#ifdef HORIZON_WIIU_TRACE_SWKBD
   const uint32_t call = ++s_trace_calc_calls;
   if (status && status->tpNormal.touched != 0 &&
       (!s_have_last_vpad || s_last_touched == 0)) {
      s_trace_calc_until_call = call + 16;
   }
   const bool trace = trace_poll(call) || call <= s_trace_calc_until_call;
   const bool trace_detail = trace_call(call) || call <= s_trace_calc_until_call;
   if (trace) report_stage(call, "calc.begin");
#endif

   if (status) {
      const auto &touch = status->tpNormal;
      const bool first_sample = !s_have_last_vpad;
      const bool hold_changed = first_sample || status->hold != s_last_hold;
      const bool touch_changed = first_sample || touch.touched != s_last_touched ||
                                 (touch.touched != 0 &&
                                  touch.validity != s_last_touch_validity);
      if (hold_changed) {
         OSReport("Horizon Cafe swkbd VPAD hold=%08x trigger=%08x release=%08x\n",
                  status->hold, status->trigger, status->release);
      }
      if (touch_changed) {
         OSReport("Horizon Cafe swkbd touch=%u validity=%u x=%u y=%u\n",
                  touch.touched, touch.validity, touch.x, touch.y);
      }
      s_last_hold = status->hold;
      s_last_touched = touch.touched;
      s_last_touch_validity = touch.validity;
      s_have_last_vpad = true;
   }

   nn::swkbd::ControllerInfo info;
   info.vpad = status;
   nn::swkbd::Calc(info);
#ifdef HORIZON_WIIU_TRACE_SWKBD
   if (trace) report_stage(call, "calc.returned");
   if (trace_detail) report_stage(call, "font_check.begin");
#endif
   const bool need_font = nn::swkbd::IsNeedCalcSubThreadFont();
#ifdef HORIZON_WIIU_TRACE_SWKBD
   if (trace_detail) {
      OSReport("Horizon Cafe SWKBD call=%u stage=font_check.returned needed=%u\n",
               (unsigned)call, need_font ? 1u : 0u);
   }
#endif
   if (need_font) {
#ifdef HORIZON_WIIU_TRACE_SWKBD
      if (trace_detail) report_stage(call, "font_calc.begin");
#endif
      nn::swkbd::CalcSubThreadFont();
#ifdef HORIZON_WIIU_TRACE_SWKBD
      if (trace_detail) report_stage(call, "font_calc.returned");
#endif
   }
#ifdef HORIZON_WIIU_TRACE_SWKBD
   if (trace_detail) report_stage(call, "predict_check.begin");
#endif
   const bool need_predict = nn::swkbd::IsNeedCalcSubThreadPredict();
#ifdef HORIZON_WIIU_TRACE_SWKBD
   if (trace_detail) {
      OSReport("Horizon Cafe SWKBD call=%u stage=predict_check.returned needed=%u\n",
               (unsigned)call, need_predict ? 1u : 0u);
   }
#endif
   if (need_predict) {
#ifdef HORIZON_WIIU_TRACE_SWKBD
      if (trace_detail) report_stage(call, "predict_calc.begin");
#endif
      nn::swkbd::CalcSubThreadPredict();
#ifdef HORIZON_WIIU_TRACE_SWKBD
      if (trace_detail) report_stage(call, "predict_calc.returned");
#endif
   }

#ifdef HORIZON_WIIU_TRACE_SWKBD
   if (trace_detail) report_stage(call, "decide_ok.begin");
#endif
   const bool accepted = nn::swkbd::IsDecideOkButton(nullptr);
#ifdef HORIZON_WIIU_TRACE_SWKBD
   if (trace_detail) {
      OSReport("Horizon Cafe SWKBD call=%u stage=decide_ok.returned accepted=%u\n",
               (unsigned)call, accepted ? 1u : 0u);
   }
#endif
   if (accepted) {
      OSReport("Horizon Cafe swkbd decision=accept open=%d\n", s_open ? 1 : 0);
#ifdef HORIZON_WIIU_TRACE_SWKBD
      if (trace_detail) report_stage(call, "accepted_result.begin");
#endif
      remember_result(ResultState::Accepted);
#ifdef HORIZON_WIIU_TRACE_SWKBD
      if (trace_detail) report_stage(call, "accepted_result.returned");
      if (trace_detail) report_stage(call, "disappear.begin");
#endif
      close_form();
#ifdef HORIZON_WIIU_TRACE_SWKBD
      if (trace_detail) report_stage(call, "disappear.returned");
#endif
      OSReport("Horizon Cafe swkbd closed accepted open=%d\n", s_open ? 1 : 0);
      return 1;
   }
#ifdef HORIZON_WIIU_TRACE_SWKBD
   if (trace_detail) report_stage(call, "decide_cancel.begin");
#endif
   const bool cancelled = nn::swkbd::IsDecideCancelButton(nullptr);
#ifdef HORIZON_WIIU_TRACE_SWKBD
   if (trace_detail) {
      OSReport("Horizon Cafe SWKBD call=%u stage=decide_cancel.returned cancelled=%u\n",
               (unsigned)call, cancelled ? 1u : 0u);
   }
#endif
   if (cancelled) {
      OSReport("Horizon Cafe swkbd decision=cancel open=%d\n", s_open ? 1 : 0);
#ifdef HORIZON_WIIU_TRACE_SWKBD
      if (trace_detail) report_stage(call, "cancelled_result.begin");
#endif
      remember_result(ResultState::Cancelled);
#ifdef HORIZON_WIIU_TRACE_SWKBD
      if (trace_detail) report_stage(call, "cancelled_result.returned");
      if (trace_detail) report_stage(call, "disappear.begin");
#endif
      close_form();
#ifdef HORIZON_WIIU_TRACE_SWKBD
      if (trace_detail) report_stage(call, "disappear.returned");
#endif
      OSReport("Horizon Cafe swkbd closed cancelled open=%d\n", s_open ? 1 : 0);
      return 2;
   }
   return 0;
}

extern "C" int32_t horizon_swkbd_draw(uint32_t target) {
   if (!s_open) return 0;
   if (target == 0) {
#ifdef HORIZON_WIIU_TRACE_SWKBD
      const uint32_t call = ++s_trace_tv_draw_calls;
      const bool trace = trace_poll(call);
      const bool trace_detail = trace_call(call);
      if (trace) report_stage(call, "draw_tv.begin");
#endif
      nn::swkbd::DrawTV();
#ifdef HORIZON_WIIU_TRACE_SWKBD
      if (trace_detail) report_stage(call, "draw_tv.returned");
#endif
   } else if (target == 1) {
#ifdef HORIZON_WIIU_TRACE_SWKBD
      const uint32_t call = ++s_trace_drc_draw_calls;
      const bool trace = trace_poll(call);
      const bool trace_detail = trace_call(call);
      if (trace) report_stage(call, "draw_drc.begin");
#endif
      nn::swkbd::DrawDRC();
#ifdef HORIZON_WIIU_TRACE_SWKBD
      if (trace_detail) report_stage(call, "draw_drc.returned");
#endif
   } else {
      return -1;
   }
   return 0;
}

extern "C" int32_t horizon_swkbd_take_result(char *utf8, size_t capacity,
                                              size_t *length) {
   if (!length) return -1;
   *length = 0;
   if (s_result_state == ResultState::None) return 0;
   if (s_result_state == ResultState::Cancelled) {
      s_result_state = ResultState::None;
      return 2;
   }
   if (!utf8 || capacity <= s_result_length) return -2;
   std::memcpy(utf8, s_result, s_result_length + 1);
   *length = s_result_length;
   s_result_state = ResultState::None;
   s_result_length = 0;
   return 1;
}

extern "C" void horizon_swkbd_close(void) {
   close_form();
   if (s_created) {
      nn::swkbd::Destroy();
      s_created = false;
   }
   if (s_fs_client_registered) {
      FSDelClient(&s_fs_client, FS_ERROR_FLAG_NONE);
      s_fs_client_registered = false;
   }
   free(s_work_memory);
   s_work_memory = nullptr;
   s_work_memory_size = 0;
   s_result_state = ResultState::None;
   s_result_length = 0;
   s_have_last_vpad = false;
   s_result[0] = '\0';
   s_initial_text[0] = 0;
}
