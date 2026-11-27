#include <coreinit/debug.h>
#include <stdint.h>
#include <string.h>

int main(void) {
    static const char counted[] = {'a', 'b', 'c', 'd'};
    static char long_text[8192];
    memset(long_text, 'X', sizeof(long_text));

    OSReport("Horizon format probe start\n");
    OSReport("Horizon format probe precision=[%.*s] tail=%u\n", 3, counted, 37u);
    OSReport("Horizon format probe zero=[%.*s] negative=[%.*s] early=[%.*s] tail=%u\n",
        0, counted, -1, "word", 7, "short", 92u);
    OSReport("Horizon format probe decimal=%llu hex=%llx tail=%u\n",
        0xfedcba9876543210ULL, 0x9876543210fedcbaULL, 43u);
    OSReport("Horizon format probe cap=%.*s", (int)sizeof(long_text), long_text);
    OSReport("\nHorizon format probe end\n");
    return 0;
}
