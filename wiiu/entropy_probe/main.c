#include <coreinit/debug.h>
#include <coreinit/filesystem.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <whb/proc.h>
#include <whb/sdcard.h>

extern uint32_t horizon_entropy_probe(void);
static FSClient client;
static FSCmdBlock cmd;
static _Alignas(64) char report[128];

int main(void)
{
   WHBProcInit();
   int ready = WHBMountSdCard() && FSAddClient(&client, FS_ERROR_FLAG_NONE) == FS_STATUS_OK;
   if (ready) FSInitCmdBlock(&cmd);
   uint32_t result = horizon_entropy_probe();
   OSReport("Horizon std entropy diagnostic: failure flags=%u\n", result);
   if (ready) {
      char path[256];
      FSFileHandle file;
      snprintf(path, sizeof(path), "%s/wiiu/apps/HorizonWiiU/std-entropy-status.log", WHBGetSdCardMountPath());
      if (FSOpenFile(&client, &cmd, path, "w", &file, FS_ERROR_FLAG_NONE) == FS_STATUS_OK) {
         snprintf(report, sizeof(report), "failure_flags=%u\n", result);
         FSWriteFile(&client, &cmd, (uint8_t *)report, 1, (uint32_t)strlen(report), file, 0, FS_ERROR_FLAG_NONE);
         FSCloseFile(&client, &cmd, file, FS_ERROR_FLAG_NONE);
      }
      FSDelClient(&client, FS_ERROR_FLAG_NONE);
   }
   WHBUnmountSdCard();
   WHBProcShutdown();
   return (int)result;
}
