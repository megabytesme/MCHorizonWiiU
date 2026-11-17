#include <coreinit/debug.h>
#include <stdint.h>
#include <whb/proc.h>

// This is the sequence emitted by the Cafe toolchain for an unsigned ordering.
__attribute__((noinline)) static int32_t carry_order(uint32_t a, uint32_t b) {
   uint32_t x, y;
   __asm__ volatile("subfc %0,%2,%3\n\t"
                    "subf %1,%3,%2\n\t"
                    "subfe %0,%3,%2\n\t"
                    "subfe %0,%0,%1"
                    : "=&r"(x), "=&r"(y) : "r"(a), "r"(b) : "xer");
   return (int32_t)x;
}

// Independent control with no carry dependency.
__attribute__((noinline)) static int32_t branch_order(uint32_t a, uint32_t b) {
   int32_t result;
   __asm__ volatile("cmplw %1,%2\n\t"
                    "blt 1f\n\t"
                    "bgt 2f\n\t"
                    "li %0,0\n\t"
                    "b 3f\n"
                    "1: li %0,-1\n\t"
                    "b 3f\n"
                    "2: li %0,1\n"
                    "3:"
                    : "=&r"(result) : "r"(a), "r"(b) : "cr0");
   return result;
}

int main(void) {
   static const uint32_t pairs[][2] = {
      {105, 112}, {112, 105}, {105, 105}, {0, 1},
      {1, 0}, {0, UINT32_MAX}, {UINT32_MAX, 0}, {UINT32_MAX, UINT32_MAX},
   };
   static const int32_t expected[] = {-1, 1, 0, -1, 1, -1, 1, 0};
   uint32_t first_carry = 0, first_branch = 0;
   int32_t first_value = 0;

   WHBProcInit();
   for (uint32_t i = 1; i <= 1000000; ++i) {
      unsigned j = i % 8;
      int32_t carry = carry_order(pairs[j][0], pairs[j][1]);
      int32_t branch = branch_order(pairs[j][0], pairs[j][1]);
      if (carry != expected[j] && !first_carry) {
         first_carry = i;
         first_value = carry;
      }
      if (branch != expected[j] && !first_branch) first_branch = i;
   }
   OSReport("CARRY_MINIMAL calls=1000000 chain_first_bad=%u value=%d branch_first_bad=%u\n",
            first_carry, first_value, first_branch);
   WHBProcShutdown();
   return first_carry || first_branch ? 1 : 0;
}
