use bdja_core::types::VolumeInfo;

#[cfg(windows)]
pub fn list_system_volumes() -> Vec<VolumeInfo> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        GetDriveTypeW, GetLogicalDriveStringsW, GetVolumeInformationW, GetDiskFreeSpaceExW,
    };

    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    const DRIVE_REMOTE: u32 = 4;
    const DRIVE_CDROM: u32 = 5;
    const DRIVE_RAMDISK: u32 = 6;

    let mut volumes = Vec::new();
    let mut buffer = [0u16; 512];
    let len = unsafe { GetLogicalDriveStringsW(Some(&mut buffer)) };
    if len == 0 || len as usize > buffer.len() {
        return volumes;
    }

    let mut start = 0;
    let mut vol_id = 0u8;

    for i in 0..len as usize {
        if buffer[i] == 0 {
            if i > start {
                let drive_slice = &buffer[start..i];
                let drive_path = OsString::from_wide(drive_slice).to_string_lossy().to_string();

                let mut wide_drive: Vec<u16> = drive_slice.to_vec();
                wide_drive.push(0);

                let drive_type = unsafe { GetDriveTypeW(PCWSTR(wide_drive.as_ptr())) };

                // Omitir lectores CD/DVD o unidades virtuales inaccesibles
                if drive_type == DRIVE_CDROM {
                    start = i + 1;
                    continue;
                }

                let is_removable = drive_type == DRIVE_REMOVABLE;
                let is_remote = drive_type == DRIVE_REMOTE;
                let _is_fixed = drive_type == DRIVE_FIXED || drive_type == DRIVE_RAMDISK;

                let mut vol_name = [0u16; 256];
                let mut fs_name = [0u16; 256];
                let mut serial = 0u32;
                let mut max_comp_len = 0u32;
                let mut flags = 0u32;

                let success = unsafe {
                    GetVolumeInformationW(
                        PCWSTR(wide_drive.as_ptr()),
                        Some(&mut vol_name),
                        Some(&mut serial),
                        Some(&mut max_comp_len),
                        Some(&mut flags),
                        Some(&mut fs_name),
                    )
                }
                .is_ok();

                let (label, fs_type, is_ready) = if success {
                    let name_len = vol_name.iter().position(|&c| c == 0).unwrap_or(vol_name.len());
                    let fs_len = fs_name.iter().position(|&c| c == 0).unwrap_or(fs_name.len());

                    let l = OsString::from_wide(&vol_name[..name_len]).to_string_lossy().to_string();
                    let f = OsString::from_wide(&fs_name[..fs_len]).to_string_lossy().to_string();
                    (l, f, true)
                } else {
                    (
                        String::new(),
                        if is_remote { "Red".to_string() } else { "Desconocido".to_string() },
                        false,
                    )
                };

                let mut free_bytes_available = 0u64;
                let mut total_number_of_bytes = 0u64;
                let mut total_number_of_free_bytes = 0u64;

                let _ = unsafe {
                    GetDiskFreeSpaceExW(
                        PCWSTR(wide_drive.as_ptr()),
                        Some(&mut free_bytes_available),
                        Some(&mut total_number_of_bytes),
                        Some(&mut total_number_of_free_bytes),
                    )
                };

                volumes.push(VolumeInfo {
                    id: vol_id,
                    path: drive_path,
                    label: if label.is_empty() {
                        if is_removable { "Unidad USB".to_string() } else { "Disco Local".to_string() }
                    } else {
                        label
                    },
                    fs_type,
                    is_removable,
                    is_ready,
                    total_bytes: total_number_of_bytes,
                    free_bytes: free_bytes_available,
                });

                vol_id = vol_id.wrapping_add(1);
            }
            start = i + 1;
        }
    }

    volumes
}

#[cfg(not(windows))]
pub fn list_system_volumes() -> Vec<VolumeInfo> {
    // Implementación portable POSIX / macOS
    Vec::new()
}
