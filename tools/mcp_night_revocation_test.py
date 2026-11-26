"""Exercise stock Night and the OAuth router against exact assembled source."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
CLIENT = Path("crates/horizon-client/src/bin/horizon-ctl/commands.rs")
NIGHT = Path("wiiu/loading_app/src")

# Minimal host entry points for the assembled modules. Only the game-thread
# reply and the pause before Night's state lock are controlled by the test.
HOST = r"""
#![allow(dead_code)]
extern crate self as horizon_client;
extern crate self as horizon_client_platform;

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex, OnceLock};
use hosted::night_menu::{GameFiles, LocalService, ResourcePin, World};

const CAFE_APP_ROOT: &str = "/unused-cafe-root";
static FIXTURE_ROOT: OnceLock<PathBuf> = OnceLock::new();
static BEFORE_NIGHT_LOCK: Mutex<Option<mpsc::Sender<()>>> = Mutex::new(None);

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

mod cafe_net {
    pub fn assigned_ipv4() -> std::io::Result<std::net::Ipv4Addr> { unreachable!() }
}
mod control {
    pub struct Request { pub line: String, pub reply: std::sync::mpsc::Sender<String> }
    pub fn install_hosted_mcp(_: fn(bool, bool) -> (Option<Request>, bool), _: fn() -> bool) {
        unreachable!()
    }
}
mod lan {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    pub fn base64(bytes: &[u8]) -> String { STANDARD.encode(bytes) }
}
mod benchmark {
    pub const CAFE_STATIONARY_SCENARIO: &str = "cafe-stationary";
    pub const CAFE_WORKSPACE_MARKER: &str = "horizon-benchmark-run.txt";
    pub const CAFE_WORKSPACE_OWNER: &[u8] = b"Horizon Night benchmark workspace v1\n";
}

#[path = "hosted.rs"]
mod hosted;

struct Files;
impl GameFiles for Files {
    fn world_tree_safe(&self, _: &Path, _: &str) -> bool { false }
    fn world(&self, _: &Path, _: &str, _: Option<u128>) -> Option<World> { None }
}

fn local_service() -> &'static Mutex<LocalService<Files>> {
    if std::thread::current().name() == Some("night-revocation-mutation") {
        if let Some(ready) = BEFORE_NIGHT_LOCK.lock().unwrap().take() {
            ready.send(()).unwrap();
        }
    }
    static SERVICE: OnceLock<Mutex<LocalService<Files>>> = OnceLock::new();
    SERVICE.get_or_init(|| {
        let hash = "1111111111111111111111111111111111111111111111111111111111111111";
        Mutex::new(LocalService::new(
            FIXTURE_ROOT.get().unwrap().clone(),
            ResourcePin { version: "26.3", schema: 7, client: hash, server: hash, exporter: hash },
            Files,
            &[],
        ))
    })
}
"""


HOSTED = r"""
use super::{local_service, CAFE_APP_ROOT};

#[path = "night_mcp.rs"]
pub(super) mod mcp;
#[path = "local_wire.rs"]
mod local_wire;
#[path = "night_menu.rs"]
pub(super) mod night_menu;
#[path = "night_online.rs"]
mod night_online;
#[path = "night_store.rs"]
mod night_store;
"""


CASES = {
    "revocation": ("revocation_fixture", "night_mcp_revocation.rs",
                   "copied_grant_revocation_closes_queued_write_and_reply"),
    "benchmark-run-id": ("benchmark_run_id_fixture", "night_mcp_benchmark_run_id.rs",
                         "benchmark_run_id_survives_foreground_handoff_and_scopes_cancel"),
}


def run(source, case="revocation"):
    module, filename, test = CASES[case]
    fixture_test = ROOT / "tools/fixtures" / filename
    with tempfile.TemporaryDirectory(prefix="horizon-night-revocation-") as name:
        scratch = Path(name)
        shutil.copytree(source / NIGHT, scratch / NIGHT)
        for path in (CLIENT, Path("docs/horizon-ctl-commands.md")):
            (scratch / path).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source / path, scratch / path)
        bridge = scratch / NIGHT / "night_mcp.rs"
        shutil.copy2(fixture_test, bridge.with_name(filename))
        with bridge.open("a", encoding="utf-8") as file:
            file.write(f'\n#[cfg(test)]\n#[path = "{filename}"]\nmod {module};\n')
        # A path module beside Night preserves its native hosted::mcp alias.
        (scratch / NIGHT / "host_fixture.rs").write_text(HOST, encoding="utf-8")
        (scratch / NIGHT / "hosted.rs").write_text(HOSTED, encoding="utf-8")
        manifest = scratch / "Cargo.toml"
        manifest.write_text(
            "[package]\nname = 'horizon-night-revocation-fixture'\nversion = '0.0.0'\n"
            "edition = '2021'\n[lib]\npath = 'wiiu/loading_app/src/host_fixture.rs'\n"
            "[profile.test]\ndebug = 0\n[dependencies]\n"
            "base64 = '0.22.1'\nserde = { version = '1', features = ['derive'] }\n"
            "serde_json = '1'\nzeroize = '1'\n"
            + "".join(
                f"horizon-wiiu-{crate} = {{ path = {json.dumps((source / 'wiiu' / crate).as_posix())}"
                f"{', features = [\"fixture\"]' if crate == 'microsoft' else ''} }}\n"
                for crate in ("mcp", "microsoft", "security")
            ),
            encoding="utf-8",
        )
        runtime = scratch / "runtime"
        runtime.mkdir()
        env = dict(
            os.environ, HORIZON_NIGHT_ACCOUNT_FIXTURE="1", TMPDIR=str(runtime),
            TEMP=str(runtime), TMP=str(runtime),
        )
        subprocess.run(
            ["cargo", "test", "--offline", "--manifest-path", str(manifest), "--lib",
             f"hosted::mcp::{module}::{test}",
             "--", "--exact", "--test-threads=1"],
            env=env, cwd=runtime, check=True,
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path, help="assembled Nightly source directory")
    parser.add_argument("--case", choices=CASES, default="revocation")
    args = parser.parse_args()
    run(args.source.resolve(), args.case)
