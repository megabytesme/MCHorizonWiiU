#include <coreinit/debug.h>
#include <coreinit/filesystem_fsa.h>
#include <coreinit/time.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <whb/proc.h>
#include <whb/sdcard.h>

#define HEADER_SIZE 8192u
#define SD_PATH_MAX 256

static _Alignas(64) uint8_t data[HEADER_SIZE];
static _Alignas(64) FSAStat stat;

static bool write_record(FSAClientHandle client, const char *path, uint8_t value)
{
   FSAFileHandle handle;
   if (FSAOpenFileEx(client, path, "w", 0666, 0, 0, &handle) < 0) {
      return false;
   }
   memset(data, value, 64);
   const int written = FSAWriteFile(client, data, 1, 64, handle, 0);
   const int flushed = FSAFlushFile(client, handle);
   const int closed = FSACloseFile(client, handle);
   return written == 64 && flushed == 0 && closed == 0;
}

static bool read_record(FSAClientHandle client, const char *path, uint8_t value)
{
   FSAFileHandle handle;
   if (FSAOpenFileEx(client, path, "r", 0, 0, 0, &handle) < 0) {
      return false;
   }
   const int count = FSAReadFile(client, data, 1, 64, handle, 0);
   const int closed = FSACloseFile(client, handle);
   return count == 64 && closed == 0 && data[0] == value && data[63] == value;
}

static bool test_replacement(FSAClientHandle client, const char *directory)
{
   char live[SD_PATH_MAX], stage_dir[SD_PATH_MAX], stage[SD_PATH_MAX], previous[SD_PATH_MAX];
   snprintf(live, sizeof(live), "%s/live", directory);
   snprintf(stage_dir, sizeof(stage_dir), "%s/stage", directory);
   snprintf(stage, sizeof(stage), "%s/stage/payload", directory);
   snprintf(previous, sizeof(previous), "%s/previous", directory);
   if (FSAMakeDir(client, stage_dir, 0666) != 0
       || !write_record(client, live, 1) || !write_record(client, stage, 2)) {
      return false;
   }
   const int rename_existing = FSARename(client, stage, live);
   OSReport("Horizon Cafe FS probe: rename onto existing status=%d\n", rename_existing);
   if (rename_existing == 0 && !write_record(client, stage, 2)) {
      return false;
   }
   const uint8_t original = rename_existing == 0 ? 2 : 1;
   if (!read_record(client, live, original)
       || FSARename(client, live, previous) != 0
       || !read_record(client, previous, original)
       || FSARename(client, stage, live) != 0
       || !read_record(client, live, 2)
       || !read_record(client, previous, original)) {
      return false;
   }
   return FSARemove(client, live) == 0 && FSARemove(client, previous) == 0
       && FSARemove(client, stage_dir) == 0;
}

static bool test_region(FSAClientHandle client, const char *directory)
{
   char region[SD_PATH_MAX];
   snprintf(region, sizeof(region), "%s/region", directory);
   FSAFileHandle handle;
   if (FSAOpenFileEx(client, region, "w+", 0666, 0, 0, &handle) < 0) {
      return false;
   }
   // Raw FSA reports the status without putting a high-level FSClient into
   // the fatal-volume state when the probe deliberately requests bad input.
   const int seek_empty = FSASetPosFile(client, handle, HEADER_SIZE);
   OSReport("Horizon Cafe FS probe: seek to 8192 on empty w+ status=%d\n", seek_empty);
   memset(data, 0, sizeof(data));
   bool ok = FSASetPosFile(client, handle, 0) == 0
       && FSAWriteFile(client, data, 1, sizeof(data), handle, 0) == sizeof(data)
       && FSASetPosFile(client, handle, HEADER_SIZE) == 0;
   memset(data, 0x6a, 64);
   ok = ok && FSAWriteFile(client, data, 1, 64, handle, 0) == 64
       && FSASetPosFile(client, handle, HEADER_SIZE + 32) == 0
       && FSATruncateFile(client, handle) == 0
       && FSAFlushFile(client, handle) == 0
       && FSAGetStatFile(client, handle, &stat) == 0 && stat.size == HEADER_SIZE + 32;
   const bool closed = FSACloseFile(client, handle) == 0;
   return ok && closed && FSARemove(client, region) == 0;
}

int main(void)
{
   WHBProcInit();
   if (!WHBMountSdCard()) {
      OSReport("Horizon Cafe FS probe: SD mount failed\n");
      WHBProcShutdown();
      return 1;
   }
   const char *mount = WHBGetSdCardMountPath();
   char directory[SD_PATH_MAX - 16];
   const int length = snprintf(directory, sizeof(directory), "%s/wiiu/horizon-fs-probe-%llx",
                               mount, (unsigned long long)OSGetTime());
   const FSAClientHandle client = FSAAddClient(NULL);
   unsigned failures = 0;
   if (client < 0 || length < 0 || (size_t)length >= sizeof(directory)
       || FSAMakeDir(client, directory, 0666) != 0) {
      failures = 1;
   } else {
      failures |= test_replacement(client, directory) ? 0 : 2;
      failures |= test_region(client, directory) ? 0 : 4;
      const int flush_volume = FSAFlushVolume(client, mount);
      OSReport("Horizon Cafe FS probe: flush SD volume status=%d\n", flush_volume);
      if (!failures && FSARemove(client, directory) != 0) {
         failures |= 8;
      }
   }
   if (client >= 0 && FSADelClient(client) != 0) {
      failures |= 16;
   }
   OSReport("Horizon Cafe FS probe: failure_flags=%u\n", failures);
   WHBUnmountSdCard();
   WHBProcShutdown();
   return (int)failures;
}
