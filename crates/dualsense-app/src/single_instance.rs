use std::net::SocketAddr;
use std::path::PathBuf;

/// Runtime file storing the active instance's HTTP URL (`http://127.0.0.1:<port>`).
pub fn active_url_file_path() -> PathBuf {
    std::env::temp_dir()
        .join("DualSenseStudio")
        .join("active_url.txt")
}

/// Reads the active instance's HTTP URL if available.
pub fn read_active_instance_url() -> Option<String> {
    let raw = std::fs::read_to_string(active_url_file_path()).ok()?;
    let trimmed = raw.trim();
    if trimmed.starts_with("http://") {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Persists the active instance's HTTP URL so secondary launches can focus/open it.
pub fn write_active_instance_url(addr: SocketAddr) {
    let path = active_url_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, format!("http://{addr}"));
}

/// OS-level single-instance lock guard. Releases lock and URL file on drop.
pub struct SingleInstanceGuard {
    #[cfg(target_os = "windows")]
    mutex_handle: isize,
    #[cfg(not(target_os = "windows"))]
    lock_path: Option<PathBuf>,
    owns_url_file: bool,
}

impl SingleInstanceGuard {
    /// Attempts to acquire the global single-instance lock.
    /// Returns `Ok(Some(guard))` if this is the primary instance, or `Ok(None)` if another instance is already running.
    pub fn try_acquire(bypass: bool) -> Self {
        if bypass {
            return Self {
                #[cfg(target_os = "windows")]
                mutex_handle: 0,
                #[cfg(not(target_os = "windows"))]
                lock_path: None,
                owns_url_file: false,
            };
        }

        #[cfg(target_os = "windows")]
        {
            match windows_lock::acquire_named_mutex() {
                Some(handle) => Self {
                    mutex_handle: handle,
                    owns_url_file: true,
                },
                None => Self {
                    mutex_handle: 0,
                    owns_url_file: false,
                },
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let lock_path = std::env::temp_dir().join("dualsense-studio.pid");
            if let Ok(existing) = std::fs::read_to_string(&lock_path) {
                if let Ok(pid) = existing.trim().parse::<u32>() {
                    if std::path::Path::new(&format!("/proc/{pid}")).exists() {
                        return Self {
                            lock_path: None,
                            owns_url_file: false,
                        };
                    }
                }
            }
            let _ = std::fs::write(&lock_path, std::process::id().to_string());
            Self {
                lock_path: Some(lock_path),
                owns_url_file: true,
            }
        }
    }

    pub fn is_primary(&self) -> bool {
        #[cfg(target_os = "windows")]
        {
            self.mutex_handle != 0
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.lock_path.is_some()
        }
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if self.owns_url_file {
            let _ = std::fs::remove_file(active_url_file_path());
        }
        #[cfg(target_os = "windows")]
        {
            if self.mutex_handle != 0 {
                windows_lock::release_named_mutex(self.mutex_handle);
                self.mutex_handle = 0;
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(p) = self.lock_path.take() {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod windows_lock {
    const ERROR_ALREADY_EXISTS: u32 = 183;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateMutexW(
            lp_mutex_attributes: *const std::ffi::c_void,
            b_initial_owner: i32,
            lp_name: *const u16,
        ) -> isize;
        fn GetLastError() -> u32;
        fn CloseHandle(h_object: isize) -> i32;
    }

    pub fn acquire_named_mutex() -> Option<isize> {
        let name: Vec<u16> = "Local\\DualSenseStudio_SingleInstance_Mutex"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let handle = CreateMutexW(std::ptr::null(), 1, name.as_ptr());
            if handle == 0 {
                return None;
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(handle);
                return None;
            }
            Some(handle)
        }
    }

    pub fn release_named_mutex(handle: isize) {
        unsafe {
            CloseHandle(handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_url_file_path_ends_with_expected_name() {
        let p = active_url_file_path();
        assert!(p.ends_with("active_url.txt"));
    }
}
