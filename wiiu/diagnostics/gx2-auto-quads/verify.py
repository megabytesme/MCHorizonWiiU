"""Validate the 22 observed targets in the warm-control native quad fixture."""
from hashlib import sha256
from pathlib import Path
import json
import re
import sys

log = Path(sys.argv[1]).read_bytes()
text = log.decode("utf-8", "replace")
row = re.compile(
    r"GX2 auto quad s=(?P<scene>\d+) arm=(?P<arm>\d+) out=(?P<out>\d+) "
    r"draws=(?P<colored>\d+) green=(?P<depth_witness>\d+) clear=(?P<clear>[01]) "
    r"diff=(?P<diff>\d+) alpha=(?P<alpha>\d+) rgba=(?P<digest>[0-9a-f]{16}) (?P<result>PASS|FAIL)"
)
observed = {}
for item in row.finditer(text):
    key = tuple(int(item[k]) for k in ("scene", "arm", "out"))
    if key in observed:
        raise SystemExit(f"Repeated target record {key}")
    observed[key] = item.groupdict()
expected = [(0, 5, 0), (0, 5, 1)] + [
    (scene, arm, out)
    for scene, arms in enumerate(((0, 1, 2, 3, 4), (0, 2, 1, 4, 3)))
    for arm in arms
    for out in range(2)
]
if list(observed) != expected:
    raise SystemExit("Missing or out-of-order fixture records")
for (scene, arm, out), item in observed.items():
    if item["result"] != "PASS" or item["clear"] != "1":
        raise SystemExit(f"Fixture target failed {scene, arm, out}: {item}")
    if arm == 5:
        continue
    reference = observed[scene, 0, out]
    exact = item["digest"] == reference["digest"] and item["diff"] == item["alpha"] == "0"
    if arm <= 2 and not exact:
        raise SystemExit(f"Compared target differs {scene, arm, out}: {item}")
    if arm in (3, 4) and (item["digest"] == reference["digest"] or item["diff"] == "0"):
        raise SystemExit(f"Negative did not differ {scene, arm, out}: {item}")
if text.count("GX2 auto quad fixture result=PASS") != 1 or "coreinit.exit(0)" not in text:
    raise SystemExit("Missing successful fixture exit")
print(json.dumps({
    "log_sha256": sha256(log).hexdigest(),
    "records": len(observed),
    "cold_display_matches_warm": [
        observed[0, 5, out]["digest"] == observed[0, 0, out]["digest"]
        for out in range(2)
    ],
    "warm_digests": [[observed[scene, 0, out]["digest"] for out in range(2)] for scene in range(2)],
}, indent=2))
