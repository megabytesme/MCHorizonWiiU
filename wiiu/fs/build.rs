use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=available_space.c");
    println!("cargo:rerun-if-env-changed=DEVKITPRO");
    println!("cargo:rerun-if-env-changed=DEVKITPPC");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("cafe") {
        return;
    }
    let devkitpro =
        PathBuf::from(std::env::var_os("DEVKITPRO").expect("DEVKITPRO must point to devkitPro"));
    let devkitppc = std::env::var_os("DEVKITPPC")
        .map(PathBuf::from)
        .unwrap_or_else(|| devkitpro.join("devkitPPC"));
    cc::Build::new()
        .compiler(devkitppc.join("bin/powerpc-eabi-gcc"))
        .archiver(devkitppc.join("bin/powerpc-eabi-ar"))
        .pic(false)
        .include(devkitpro.join("wut/include"))
        .flag("-mcpu=750")
        .flag("-meabi")
        .flag("-mhard-float")
        .define("__WIIU__", None)
        .define("__WUT__", None)
        .file("available_space.c")
        .compile("horizon_wiiu_fs");
}
