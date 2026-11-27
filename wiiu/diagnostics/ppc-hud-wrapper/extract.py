from pathlib import Path
import sys

path, arm = sys.argv[1:]
assert arm in {"original", "repaired"}
native = Path(path).read_text(encoding="utf-8")
kind = "horizon_loading_host_present_view_hud"
start = native.index(f"\n{kind}:\n")
end = native.index(f"\t.size\t{kind}", start)
end = native.index("\n", end)
body = native[start:end]
assert body.count("\n\tbl present_with_clear") == 1
if arm == "original":
    assert "\tcmpwi 0,30,0\n.L603:\n\tmfcr 9\n" in body
    assert "\tmtcrf 128,9\n\tbne- 0,.L604\n" in body
else:
    assert "mfcr" not in body and "mtcrf" not in body
print(f".text\n.align 2\n.globl probe_{arm}\n.type probe_{arm}, @function")
print(body.replace(kind, f"probe_{arm}"))
