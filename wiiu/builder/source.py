"""Exact upstream and native source assembly for the port-only repository."""
from pathlib import Path, PurePosixPath
import tempfile

from core import (ROOT, TARGET, VERSION, VERSION_PATTERN, digest, fields, file_digest,
                  git, isolated, parse_json, pattern, verify_source, write_record)

MANIFEST = ROOT / "catalog/compatibility-v1.json"
SHA1 = "[0-9a-f]{40}"
SHA256 = "[0-9a-f]{64}"
NATIVE_TOOLS = {
    "tools/horizon_wiiu_capture.py", "tools/wiiu_normalize_rpl_relocations.py",
    "tools/wiiu_recompress_rpx.py", ".gitmodules",
}


def relative(value):
    if not isinstance(value, str):
        raise ValueError("expected a relative file path")
    path = PurePosixPath(value)
    if (not value or path.is_absolute() or path.as_posix() != value or "\\" in value
            or ":" in value or any(part in {".", "..", ".git"} for part in path.parts)
            or any(ord(char) < 32 for char in value)):
        raise ValueError(f"unsafe relative file path: {value!r}")
    return path


def contained(root, name):
    p = root / relative(name)
    if not p.resolve().is_relative_to(root.resolve()):
        raise ValueError(f"input escapes its root: {name}")
    if p.is_symlink() or not p.is_file():
        raise ValueError(f"expected a regular input: {name}")
    return p


def checked_bytes(root, name, expected):
    data = contained(root, name).read_bytes()
    if digest(data) != expected:
        raise ValueError(f"SHA-256 mismatch: {name}")
    return data


def pinned_tree(value):
    fields(value, "commit", "tree", "inventory_sha256")
    for key, expression in (("commit", SHA1), ("tree", SHA1), ("inventory_sha256", SHA256)):
        pattern(value[key], expression, key)


def validate_recipe(recipe):
    fields(recipe, "schema", "builder", "client", "launcher", "patch", "toolchain", "target",
           "upstream", "native", "series", "diagnostic", "inputs_sha256")
    if type(recipe["schema"]) is not int or recipe["schema"] != 1:
        raise ValueError("unsupported recipe schema")
    for name in ("builder", "client", "launcher", "patch", "toolchain"):
        pattern(recipe[name], VERSION_PATTERN, name)
    if (recipe["builder"], recipe["client"], recipe["launcher"], recipe["patch"],
        recipe["toolchain"], recipe["target"]) != (VERSION, "0.4.0", "0.3.0", "0.2.0", "1.0.0", TARGET):
        raise ValueError("unsupported version combination")
    pinned_tree(recipe["upstream"])
    pinned_tree(recipe["native"])
    pattern(recipe["inputs_sha256"], SHA256, "toolchain input spec SHA-256")
    previous = recipe["upstream"]["commit"]
    filenames = set()
    if not isinstance(recipe["series"], list) or not recipe["series"]:
        raise ValueError("patch series is empty")
    for item in recipe["series"]:
        fields(item, "file", "from", "to", "sha256")
        pattern(item["file"], "[0-9]{4}-[a-z0-9-]+[.]patch", "patch filename")
        pattern(item["to"], SHA1, "patch destination revision")
        pattern(item["sha256"], SHA256, "patch SHA-256")
        if item["file"] in filenames or item["from"] != previous or item["to"] == previous:
            raise ValueError("noncontiguous or duplicate patch series")
        previous = item["to"]
        filenames.add(item["file"])
    if previous != recipe["native"]["commit"]:
        raise ValueError("patch series must end at the native source pin")
    diagnostic = recipe["diagnostic"]
    fields(diagnostic, "file", "from", "to", "tree", "sha256")
    pattern(diagnostic["file"], "diagnostics/[0-9]{4}-[a-z0-9-]+[.]patch", "diagnostic filename")
    for name, expression in (("to", SHA1), ("tree", SHA1), ("sha256", SHA256)):
        pattern(diagnostic[name], expression, f"diagnostic {name}")
    if diagnostic["from"] != previous or diagnostic["to"] == previous:
        raise ValueError("diagnostic base differs from the stable pin")


def read_recipe(path):
    data = path.read_bytes()
    release = parse_json(data)
    validate_recipe(release)
    return release, digest(data)


def native_files(recipe, root):
    entries = parse_json(checked_bytes(root, "catalog/native-files-v1.json",
                                       recipe["native"]["inventory_sha256"]))
    if not isinstance(entries, list) or not entries:
        raise ValueError("missing native file inventory")
    names = set()
    for item in entries:
        if type(item) is not dict:
            raise ValueError("invalid native file entry")
        mode = item.get("mode")
        if mode == "160000":
            fields(item, "destination", "mode", "object")
            pattern(item["object"], SHA1, "gitlink")
            if item["destination"] != "wiiu/bootstrap/.cafe":
                raise ValueError("unexpected gitlink")
        else:
            fields(item, "source", "destination", "mode", "sha256")
            pattern(item["sha256"], SHA256, "native source SHA-256")
            if mode not in {"100644", "100755"}:
                raise ValueError("unsupported native file mode")
            relative(item["source"])
        name = relative(item["destination"]).as_posix()
        if (name in names or not (name.startswith(("wiiu/", "third_party/getrandom-0.4.3/"))
                                  or name in NATIVE_TOOLS)):
            raise ValueError(f"conflicting native destination: {name}")
        names.add(name)
    return entries


def apply_patch(workspace, patch):
    args = ("--index", "--binary", "--whitespace=nowarn", "-")
    git(workspace, "apply", "--check", *args, input_bytes=patch)
    git(workspace, "apply", *args, input_bytes=patch)


def staged_tree(workspace):
    return git(workspace, "write-tree").stdout.decode("ascii").strip()


def install_native(workspace, root, files):
    materialized = []
    for item in files:
        destination = item["destination"]
        mode = item["mode"]
        if mode == "160000":
            git(workspace, "update-index", "--add", "--cacheinfo",
                f'{mode},{item["object"]},{destination}')
            continue
        target = workspace / relative(destination)
        if not target.parent.resolve().is_relative_to(workspace.resolve()):
            raise ValueError(f"native destination escapes the checkout: {destination}")
        data = checked_bytes(root, item["source"], item["sha256"])
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("xb") as stream:
            stream.write(data)
        materialized.append(destination)
    git(workspace, "add", "--force", "--", *materialized)
    executables = [item["destination"] for item in files if item["mode"] == "100755"]
    if executables:
        git(workspace, "update-index", "--chmod=+x", "--", *executables)


def assemble(source, revision, recipe, recipe_hash, output, diagnostic=False, root=ROOT):
    verify_source(source, revision, recipe["upstream"])
    output = isolated(output)
    if output.exists():
        raise ValueError("assembly output already exists")
    output.parent.mkdir(parents=True, exist_ok=True)
    patches = root / "patches" / recipe["upstream"]["commit"]
    files = native_files(recipe, root)
    with tempfile.TemporaryDirectory(prefix="wiiu-assembly-", dir=output.parent) as scratch:
        staging = Path(scratch)
        workspace = staging / "source"
        workspace.mkdir()
        git(workspace, "init", "--quiet", "--template=")
        commit = recipe["upstream"]["commit"]
        git(workspace, "fetch", "--quiet", "--depth=1", "--no-tags", "--no-recurse-submodules",
            "--", str(source.resolve()), commit)
        git(workspace, "checkout", "--quiet", "--detach", commit)
        verify_source(workspace, "HEAD", recipe["upstream"])
        steps = []
        for item in recipe["series"]:
            apply_patch(workspace, checked_bytes(patches, item["file"], item["sha256"]))
            steps.append({"patch": item["file"], "sha256": item["sha256"],
                          "tree": staged_tree(workspace)})
        install_native(workspace, root, files)
        baseline = staged_tree(workspace)
        if baseline != recipe["native"]["tree"]:
            raise ValueError(f"reconstructed tree differs from the game pin: {baseline}")
        target = recipe["native"]["commit"]
        if diagnostic:
            item = recipe["diagnostic"]
            apply_patch(workspace, checked_bytes(patches, item["file"], item["sha256"]))
            target = item["to"]
            if staged_tree(workspace) != item["tree"]:
                raise ValueError("diagnostic tree differs from its pin")
        record = {
            "schema": 1, "kind": "wiiu-source-assembly", "assembly": "passed",
            "recipe_sha256": recipe_hash, "upstream": commit, "native": target,
            "native_inventory_sha256": recipe["native"]["inventory_sha256"],
            "baseline_tree": baseline, "tree": staged_tree(workspace), "patches": steps,
            "diagnostic": diagnostic,
        }
        write_record(staging / "assembly.json", record)
        output.mkdir()
        for name in ("source", "assembly.json"):
            (staging / name).rename(output / name)
    return output / "assembly.json"


def verify_assembly(root, recipe, recipe_hash):
    record = parse_json((root / "assembly.json").read_bytes())
    workspace = root / "source"
    verify_source(workspace, "HEAD", recipe["upstream"])
    if (record.get("kind") != "wiiu-source-assembly" or record.get("recipe_sha256") != recipe_hash
            or record.get("diagnostic") is not False
            or record.get("native") != recipe["native"]["commit"]
            or record.get("tree") != recipe["native"]["tree"]
            or staged_tree(workspace) != recipe["native"]["tree"]):
        raise ValueError("assembly does not match the stable game recipe")
    if git(workspace, "diff", "--quiet", "--ignore-submodules=all", check=False).returncode:
        raise ValueError("assembled source differs from the verified index")
    if git(workspace, "ls-files", "--others", "--exclude-standard", "-z").stdout:
        raise ValueError("assembled source contains untracked files")
    return workspace
