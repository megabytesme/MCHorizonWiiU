#!/usr/bin/env python3
"""Exercise the exact C draw_case helper without importing native SDK code."""
import subprocess
from pathlib import Path
from tempfile import TemporaryDirectory

source = Path(__file__).with_name("main.c").read_text(encoding="utf-8")
constants = source[source.index("enum { STOCK"):source.index("static const GX2ScanTarget")]
case = source[source.index("static bool draw_case"):source.index("static void cycle(")]
wrapper = r'''
#include <stdbool.h>
#include <assert.h>
#include <stdio.h>
typedef struct { unsigned ids[64], counts[64], n; } Fixture;
static const float clip[2][16] = {{1}, {0.91f}};
static const float close[2][4] = {{11}, {9}}, work[2][4] = {{89}, {92}};
static bool draw(Fixture *f, unsigned first, unsigned count, const float c[16], const float o[4])
{
   assert(f->n < 64 && c != NULL && o != NULL);
   f->ids[f->n] = first;
   f->counts[f->n++] = count;
   return true;
}
'''
tests = r'''
int main(void)
{
   for (unsigned output = 0; output < OUTPUTS; ++output)
      for (unsigned level = 0; level < LEVELS; ++level) {
         const unsigned n = levels[level];
         const unsigned expected = 2+n+(n+3)/4;
         for (unsigned order = 0; order < ORDERS; ++order) {
            Fixture f = {0};
            assert(draw_case(&f,output,level,order));
            assert(f.n == expected && f.ids[0] == FIRST_GUARD && f.counts[0] == GUARDS);
            unsigned far = 0, front = 0, masked = 0;
            for (unsigned i = 1; i < f.n; ++i) {
               assert(f.counts[i] == 1);
               if (f.ids[i] >= FIRST_FAR) assert(f.ids[i] == FIRST_FAR+far++);
               else if (f.ids[i] == FIRST_FRONT) {
                  assert(far == (order ? n/2 : 0));
                  ++front;
               } else {
                  assert(f.ids[i] == FIRST_FRONT+1);
                  assert(far%4 == 1);
                  ++masked;
               }
            }
            assert(far == n && front == 1 && masked == (n+3)/4);
         }
         /* Minimum negative-control rectangle span under the DRC clip is
          * about .46×.91×.41 / 4, larger than the 1/30 image threshold. */
         assert(0.46 * 0.91 * 0.41 / 4.0 > 1.0/30.0);
      }
   puts("dual-output seeded order, exact matched draw counts and negative extent PASS");
   return 0;
}
'''
with TemporaryDirectory(prefix="gx2-rez-") as directory:
    test = Path(directory) / "draw-stream-test.c"
    program = test.with_suffix("")
    test.write_text(wrapper + constants + case + tests, encoding="utf-8")
    subprocess.run(["cc", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", str(test),
                    "-o", str(program)], check=True)
    subprocess.run([str(program)], check=True)
