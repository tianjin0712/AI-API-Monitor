//! Platform-specific private filesystem permissions.

use std::path::Path;

#[cfg(windows)]
pub fn harden_private_path(path: &Path, directory: bool) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{
        SetFileSecurityW, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
    };

    // Protected DACL: SYSTEM and the object's owner only. Directory ACEs are
    // inherited by both files and child directories.
    let sddl = if directory {
        "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;OW)"
    } else {
        "D:P(A;;FA;;;SY)(A;;FA;;;OW)"
    };
    let wide_sddl: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
    let wide_path: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut descriptor = std::ptr::null_mut();

    // SAFETY: both UTF-16 inputs are NUL-terminated and descriptor is released
    // with LocalFree exactly once after the Windows API call.
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide_sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    };
    if converted == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: descriptor was allocated by the conversion API and wide_path is
    // valid for the duration of this call.
    let applied = unsafe {
        SetFileSecurityW(
            wide_path.as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        )
    };
    // SAFETY: descriptor is a LocalAlloc-compatible pointer documented for
    // release via LocalFree.
    unsafe { LocalFree(descriptor) };
    if applied == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
pub fn harden_private_path(path: &Path, directory: bool) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = if directory { 0o700 } else { 0o600 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(not(any(windows, unix)))]
pub fn harden_private_path(_path: &Path, _directory: bool) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_acl_can_be_applied_to_directory_and_file() {
        let root = std::env::temp_dir().join(format!("ai-monitor-acl-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        harden_private_path(&root, true).unwrap();
        let file = root.join("private.db");
        std::fs::write(&file, b"test").unwrap();
        harden_private_path(&file, false).unwrap();
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
