#include <coreinit/filesystem.h>
#include <stdint.h>
#include <stdlib.h>

/* Rust std has initialized FS before this is called. Each call has its own
   client and command block, so concurrent callers do not share mutable state. */
int32_t horizon_wiiu_available_space(const char *path, uint64_t *out)
{
   struct Query { FSClient client; FSCmdBlock cmd; };
   struct Query *query = calloc(1, sizeof(*query));
   if (!query) return FS_STATUS_FATAL_ERROR;
   FSStatus status = FSAddClient(&query->client, FS_ERROR_FLAG_ALL);
   if (status == FS_STATUS_OK) {
      FSInitCmdBlock(&query->cmd);
      uint64_t size = 0;
      status = FSGetFreeSpaceSize(&query->client, &query->cmd, path, &size, FS_ERROR_FLAG_ALL);
      FSStatus del_status = FSDelClient(&query->client, FS_ERROR_FLAG_ALL);
      if (del_status != FS_STATUS_OK) {
         /* The OS may still hold this client: retain its backing storage. */
         return status == FS_STATUS_OK ? del_status : status;
      }
      if (status == FS_STATUS_OK) *out = size;
   }
   free(query);
   return status;
}
