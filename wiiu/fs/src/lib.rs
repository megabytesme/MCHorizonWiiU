//! Cafe filesystem queries used by the original Horizon save code.

#[cfg(target_os = "cafe")]
pub fn available_space(path: &std::path::Path) -> std::io::Result<u64> {
    use std::ffi::CString;
    use std::io::{Error, ErrorKind};
    use std::os::raw::{c_char, c_int};

    unsafe extern "C" {
        fn horizon_wiiu_available_space(path: *const c_char, out: *mut u64) -> c_int;
    }
    // A fresh WUT client has its own current directory. Resolve relative
    // paths against Rust std's client before passing them to that client.
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let path_string = CString::new(absolute.as_os_str().as_encoded_bytes())
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "path contains NUL"))?;
    // Besides preserving missing-path errors, this initializes Cafe std's FS
    // client and SD mount before the independent WUT query registers its client.
    std::fs::metadata(&absolute)?;
    let mut size = 0;
    // The C function borrows both pointers for the duration of the call only.
    let status = unsafe { horizon_wiiu_available_space(path_string.as_ptr(), &mut size) };
    if status == 0 {
        Ok(size)
    } else {
        Err(Error::other(format!(
            "Cafe free-space query failed: {status}"
        )))
    }
}
