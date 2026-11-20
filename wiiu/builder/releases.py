"""Read the Wii U release catalog and resolve explicit compatible versions."""
from datetime import datetime, timezone
from pathlib import Path
import re
import subprocess

from core import ROOT, TARGET, VERSION_PATTERN, digest, fields, git, parse_json, pattern, write_record
from source import SHA1, SHA256, checked_bytes, relative

CATALOG = ROOT / "catalog/releases-v2.json"
REPOSITORY = "https://github.com/megabytesme/MCHorizonWiiU"
UPSTREAM = "https://github.com/horizon-rust/horizon.git"
KINDS = ("builder", "client", "patch", "toolchain")
CHANNELS = ("stable", "development", "nightly")
STATUSES = ("active", "deprecated", "revoked")
MAX_CATALOG_BYTES = 1024 * 1024


def utc(value):
    pattern(value, r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", "UTC timestamp")
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def read_catalog(path=CATALOG):
    with path.open("rb") as stream:
        data = stream.read(MAX_CATALOG_BYTES + 1)
    if len(data) > MAX_CATALOG_BYTES:
        raise ValueError("catalog exceeds the size limit")
    catalog = parse_json(data.decode("utf-8"))
    fields(catalog, "schema", "sequence", "signing", "published", "expires", "releases", "compatibility")
    if type(catalog["schema"]) is not int or catalog["schema"] != 2:
        raise ValueError("unsupported release catalog schema")
    if type(catalog["sequence"]) is not int or catalog["sequence"] < 1:
        raise ValueError("invalid catalog sequence")
    if catalog["signing"] not in ("unsigned-development", "p256-sha256-p1363"):
        raise ValueError("unsupported catalog signing policy")
    if utc(catalog["published"]) >= utc(catalog["expires"]):
        raise ValueError("invalid catalog validity interval")
    fields(catalog["releases"], *KINDS)
    for kind, records in catalog["releases"].items():
        if not isinstance(records, dict) or not records:
            raise ValueError(f"missing {kind} releases")
        for version, record in records.items():
            pattern(version, VERSION_PATTERN, f"{kind} version")
            validate_release(kind, record)
    rules = catalog["compatibility"]
    if not isinstance(rules, list) or not rules:
        raise ValueError("missing compatibility rules")
    known = set()
    for rule in rules:
        fields(rule, *KINDS, "channel", "target", "bridge_abi", "upstream")
        pattern(rule["upstream"], SHA1, "upstream commit")
        if rule["channel"] not in CHANNELS or rule["target"] != TARGET or type(rule["bridge_abi"]) is not int:
            raise ValueError("unsupported channel, target or bridge ABI")
        versions = []
        for kind in KINDS:
            version = rule[kind]
            pattern(version, VERSION_PATTERN, f"{kind} selector")
            record = catalog["releases"][kind].get(version)
            if record is None:
                raise ValueError(f"unknown {kind} {version}")
            if rule["channel"] == "stable" and record["channel"] != "stable":
                raise ValueError("stable combinations require stable releases")
            if kind in ("client", "patch") and record["bridge_abi"] != rule["bridge_abi"]:
                raise ValueError("bridge ABI mismatch")
            if kind == "patch" and record["upstream"] != rule["upstream"]:
                raise ValueError("upstream pin mismatch")
            versions.append(version)
        key = (rule["channel"], *versions)
        if key in known:
            raise ValueError("duplicate compatibility rule")
        known.add(key)
    if catalog["signing"] == "unsigned-development" and any(rule["channel"] == "stable" for rule in rules):
        raise ValueError("stable compatibility requires a signed catalog")
    return catalog, data


def validate_release(kind, record):
    extra = {"client": ("bridge_abi", "evidence"), "patch": ("bridge_abi", "upstream")}.get(kind, ())
    fields(record, "channel", "status", "description", "notes", "artifacts", *extra)
    if record["channel"] not in CHANNELS or record["status"] not in STATUSES:
        raise ValueError(f"invalid {kind} release status or channel")
    if not isinstance(record["description"], str) or not record["description"].strip():
        raise ValueError("missing release description")
    relative(record["notes"])
    if kind in ("client", "patch") and (type(record["bridge_abi"]) is not int or record["bridge_abi"] < 1):
        raise ValueError("invalid bridge ABI")
    if kind == "patch":
        pattern(record["upstream"], SHA1, "release upstream")
    if kind == "client":
        fields(record["evidence"], "build", "cemu", "hardware")
        for evidence in record["evidence"].values():
            fields(evidence, "status", "record")
            if evidence["status"] not in ("passed", "partial", "failed", "pending"):
                raise ValueError("invalid evidence status")
            relative(evidence["record"])
    artifacts = record["artifacts"]
    if not isinstance(artifacts, dict) or not artifacts:
        raise ValueError(f"missing {kind} artifacts")
    for key, artifact in artifacts.items():
        pattern(key, r"[a-z][a-z0-9-]*", "artifact name")
        if type(artifact) is dict and "github_asset_id" in artifact:
            fields(artifact, "github_asset_id", "name", "bytes", "sha256")
            if kind != "client" or key not in ("rpx", "wuhb"):
                raise ValueError("only prebuilt client packages can use a GitHub asset ID")
            if (type(artifact["github_asset_id"]) is not int or artifact["github_asset_id"] < 1
                    or type(artifact["bytes"]) is not int or artifact["bytes"] < 1
                    or artifact["name"] != f"HorizonWiiU.{key}"):
                raise ValueError("invalid prebuilt client package reference")
        else:
            fields(artifact, "commit", "path", "sha256")
            pattern(artifact["commit"], SHA1, "artifact source commit")
            relative(artifact["path"])
        pattern(artifact["sha256"], SHA256, "artifact SHA-256")


def verify_artifact(artifact, root=ROOT, *, check_worktree=True):
    """Resolve a GitHub artifact at its immutable commit and verify its bytes."""
    path = relative(artifact["path"]).as_posix()
    pinned = git(root, "cat-file", "blob", f'{artifact["commit"]}:{path}').stdout
    if digest(pinned) != artifact["sha256"]:
        raise ValueError(f"Git object does not match {path}")
    if check_worktree:
        checked_bytes(root, path, artifact["sha256"])
    return pinned


def verify_source_inventory(artifact, root=ROOT, revision=None, *, check_worktree=True):
    entries = parse_json(verify_artifact(artifact, root, check_worktree=check_worktree))
    if not isinstance(entries, dict) or not entries:
        raise ValueError("empty release source inventory")
    for path, expected in entries.items():
        relative(path)
        pattern(expected, SHA256, "source inventory SHA-256")
        if check_worktree:
            checked_bytes(root, path, expected)
        if revision is not None:
            blob = git(root, "cat-file", "blob", f"{revision}:{path}").stdout
            if digest(blob) != expected:
                raise ValueError(f"release source at {revision} differs: {path}")
    return entries


def verify_release_files(catalog, root=ROOT):
    for records in catalog["releases"].values():
        for record in records.values():
            check_worktree = record["status"] == "active"
            for name, artifact in record["artifacts"].items():
                if name == "sources":
                    verify_source_inventory(artifact, root, artifact["commit"], check_worktree=check_worktree)
                elif "commit" in artifact:
                    verify_artifact(artifact, root, check_worktree=check_worktree)
                elif catalog["signing"] != "p256-sha256-p1363":
                    raise ValueError("prebuilt client references require a signed catalog")
            paths = [record["notes"], *(entry["record"] for entry in record.get("evidence", {}).values())]
            for path in paths:
                if not (root / relative(path)).is_file():
                    raise ValueError(f"release document is missing: {path}")


def verify_client_asset(catalog, version, kind, path):
    from core import file_digest

    if catalog["signing"] != "p256-sha256-p1363" or kind not in ("rpx", "wuhb"):
        raise ValueError("prebuilt packages require a verified signed catalog")
    client = catalog["releases"]["client"][version]
    if client["status"] != "active":
        raise ValueError(f"client {version} is {client['status']}")
    artifact = client["artifacts"][kind]
    if ("github_asset_id" not in artifact or path.name != artifact["name"] or
            path.stat().st_size != artifact["bytes"] or file_digest(path) != artifact["sha256"]):
        raise ValueError("prebuilt client package does not match its signed size and SHA-256")
    return f"https://api.github.com/repos/megabytesme/MCHorizonWiiU/releases/assets/{artifact['github_asset_id']}"


def resolve(catalog, *, builder, client, patch, toolchain, channel, target=TARGET):
    requested = (builder, client, patch, toolchain)
    for rule in catalog["compatibility"]:
        if (rule["target"], rule["channel"], *(rule[kind] for kind in KINDS)) != (target, channel, *requested):
            continue
        for kind, version in zip(KINDS, requested):
            status = catalog["releases"][kind][version]["status"]
            if status == "revoked" or (status == "deprecated" and channel == "stable"):
                raise ValueError(f"{kind} {version} is {status}")
        return rule
    raise ValueError("no compatible builder, client, patch, toolchain and channel combination")


def check_sequence(catalog, data, state, now=None):
    now = now or datetime.now(timezone.utc)
    if not utc(catalog["published"]) <= now < utc(catalog["expires"]):
        raise ValueError("signed catalog is outside its UTC validity interval")
    fingerprint = digest(data)
    if state.exists():
        old = parse_json(state.read_bytes())
        fields(old, "sequence", "sha256")
        if type(old["sequence"]) is not int or old["sequence"] < 1:
            raise ValueError("invalid remembered catalog sequence")
        pattern(old["sha256"], SHA256, "remembered catalog digest")
        if old["sequence"] > catalog["sequence"] or (
            old["sequence"] == catalog["sequence"] and old["sha256"] != fingerprint
        ):
            raise ValueError("catalog rollback or sequence reuse detected")
    state.parent.mkdir(parents=True, exist_ok=True)
    write_record(state, {"sequence": catalog["sequence"], "sha256": fingerprint})


def verify_signature(path, signature, public_key, state):
    """Verify the identical bytes used for parsing, then remember their sequence."""
    import tempfile

    catalog, data = read_catalog(path)
    if catalog["signing"] != "p256-sha256-p1363":
        raise ValueError("catalog is explicitly unsigned")
    signer = ROOT / "tools/catalog-signer/CatalogSigner.csproj"
    with tempfile.TemporaryDirectory(prefix="wiiu-catalog-") as scratch:
        copy = Path(scratch) / "catalog.json"
        copy.write_bytes(data)
        subprocess.run(["dotnet", "run", "--project", str(signer), "--", "verify",
                        str(copy), str(signature), str(public_key)], check=True)
    # A single lock serializes sequence updates. A crash leaves a fail-closed
    # lock that can be removed when no verifier is running.
    state = state.resolve()
    state.parent.mkdir(parents=True, exist_ok=True)
    lock = state.with_name(state.name + ".lock")
    try:
        lock.mkdir()
    except FileExistsError as error:
        raise ValueError("catalog state is locked by another verifier") from error
    try:
        check_sequence(catalog, data, state)
    finally:
        lock.rmdir()
    return catalog, data
