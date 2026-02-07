use anyhow::{Context, Result};
use std::path::Path;

/// Set the OS-level hidden flag on the symlink so it disappears from Finder/Explorer.
pub fn hide_path(root: &Path, target: &str) -> Result<()> {
    let path = root.join(target);

    #[cfg(target_os = "macos")]
    {
        macos_set_hidden(&path, true)?;
    }

    #[cfg(target_os = "windows")]
    {
        windows_set_hidden(&path, true)?;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // On Linux, dotfiles are already hidden by convention.
        // No OS-level hidden attribute available.
        let _ = &path;
    }

    Ok(())
}

/// Remove the OS-level hidden flag from the path.
pub fn unhide_path(root: &Path, target: &str) -> Result<()> {
    let path = root.join(target);

    #[cfg(target_os = "macos")]
    {
        macos_set_hidden(&path, false)?;
    }

    #[cfg(target_os = "windows")]
    {
        windows_set_hidden(&path, false)?;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = &path;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_set_hidden(path: &Path, hidden: bool) -> Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes()).context("path contains null byte")?;

    // UF_HIDDEN = 0x8000
    const UF_HIDDEN: u32 = 0x8000;

    // Get current flags via lstat
    let mut stat_buf: libc::stat = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::lstat(c_path.as_ptr(), &mut stat_buf) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error())
            .context(format!("lstat failed on {}", path.display()));
    }

    let new_flags = if hidden {
        stat_buf.st_flags | UF_HIDDEN
    } else {
        stat_buf.st_flags & !UF_HIDDEN
    };

    // Use lchflags to operate on the symlink itself, not its target
    unsafe extern "C" {
        fn lchflags(path: *const libc::c_char, flags: libc::c_uint) -> libc::c_int;
    }

    let ret = unsafe { lchflags(c_path.as_ptr(), new_flags) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error())
            .context(format!("lchflags failed on {}", path.display()));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_set_hidden(path: &Path, hidden: bool) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::fileapi::{GetFileAttributesW, SetFileAttributesW};
    use winapi::um::winnt::FILE_ATTRIBUTE_HIDDEN;

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let attrs = unsafe { GetFileAttributesW(wide.as_ptr()) };
    if attrs == u32::MAX {
        return Err(std::io::Error::last_os_error())
            .context(format!("GetFileAttributesW failed on {}", path.display()));
    }

    let new_attrs = if hidden {
        attrs | FILE_ATTRIBUTE_HIDDEN
    } else {
        attrs & !FILE_ATTRIBUTE_HIDDEN
    };

    let ret = unsafe { SetFileAttributesW(wide.as_ptr(), new_attrs) };
    if ret == 0 {
        return Err(std::io::Error::last_os_error())
            .context(format!("SetFileAttributesW failed on {}", path.display()));
    }

    Ok(())
}
