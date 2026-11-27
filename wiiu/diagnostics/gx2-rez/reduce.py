#!/usr/bin/env python3
"""Validate the same-RPX paired parity gate and summarize matched bottom-cycle deltas."""
from collections import defaultdict
import json
from pathlib import Path
import re
from statistics import median
import sys

FAR = (0, 8, 24)
CLOCK = 27_000_000
PIXELS = re.compile(
    r"GX2 ReZ pixels out=([01]) size=(\d+)x(\d+) arm=([012]) far=(\d+) "
    r"order=([01]) diff=(\d+) ctrl=1 hole=1 PASS$"
)
CYCLES = re.compile(
    r"GX2 ReZ cycles hz=27000000 out=([01]) far=(\d+) order=([01]) "
    r"rep=([0-3]) stock=(\d+) rez=(\d+)$"
)


def reduce(text):
    if text.count("GX2 ReZ fixture result=PASS") != 1 or text.count("GX2 ReZ parity stage result=PASS") != 1:
        raise ValueError("the standalone did not report completed parity")
    snapshots = defaultdict(list)
    cycles = {}
    for line in text.splitlines():
        if m := PIXELS.search(line):
            output, width, height, arm, far, order, diff = map(int, m.groups())
            if far not in FAR or (arm == 2 and (far != 0 or order != 0)):
                raise ValueError("unexpected pixel parity arm")
            if arm < 2 and diff or arm == 2 and diff <= width * height // 30:
                raise ValueError("invalid pixel differential")
            snapshots[output, far, order, arm].append((width, height))
        if m := CYCLES.search(line):
            output, far, order, repetition, stock, rez = map(int, m.groups())
            key = output, far, order, repetition
            if key in cycles or far not in FAR:
                raise ValueError("unexpected or repeated timing record")
            cycles[key] = stock, rez

    expected = {}
    for output in range(2):
        for far in FAR:
            for order in range(2):
                expected[output, far, order, 0] = 1 + (far == order == 0)
                expected[output, far, order, 1] = 1
        expected[output, 0, 0, 2] = 1
    if {key: len(values) for key, values in snapshots.items()} != expected:
        raise ValueError("incomplete paired picture controls")
    sizes = []
    for output in range(2):
        observed = {size for key, values in snapshots.items() if key[0] == output for size in values}
        if len(observed) != 1:
            raise ValueError("output geometry changed")
        sizes.append(observed.pop())
    report = {"parity": True, "sizes": sizes, "repeated_controls": sum(expected.values()), "timings": []}
    if "GX2 ReZ timing complete=1" not in text:
        report["timing_complete"] = False
        return report
    if len(cycles) != 48 or any(not 0 < count <= 10 * CLOCK for pair in cycles.values() for count in pair):
        raise ValueError("missing or invalid complete timing samples")
    report["timing_complete"] = True
    ms = 1000 / CLOCK
    for output in range(2):
        for far in FAR:
            for order in range(2):
                observed = [cycles[output, far, order, rep] for rep in range(4)]
                deltas = [(stock-rez)*ms for stock, rez in observed]
                report["timings"].append({
                    "output": output, "far_draws": far, "order": order,
                    "stock_median_ms": median(stock * ms for stock, _ in observed),
                    "rez_median_ms": median(rez * ms for _, rez in observed),
                    "paired_delta_ms": deltas, "paired_median_delta_ms": median(deltas),
                })
    return report


if __name__ == "__main__":
    print(json.dumps(reduce(Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")), indent=2))
