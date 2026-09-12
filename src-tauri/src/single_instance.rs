//! Windows single-instance coordination.
//!
//! The mutex prevents a second process from initializing the database. An
//! auto-reset event lets the existing process bring its window forward.

#[cfg(target_os = "windows")]
use std::ptr::null_mut;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE,
    INFINITE, SYNCHRONIZE,
};

#[cfg(target_os = "windows")]
const MUTEX_NAME: &[u16] = &[
    'L' as u16, 'o' as u16, 'c' as u16, 'a' as u16, 'l' as u16, '\\' as u16, 'A' as u16,
    'I' as u16, '_' as u16, 'A' as u16, 'P' as u16, 'I' as u16, '_' as u16, 'M' as u16,
    'o' as u16, 'n' as u16, 'i' as u16, 't' as u16, 'o' as u16, 'r' as u16, '_' as u16,
    'S' as u16, 'i' as u16, 'n' as u16, 'g' as u16, 'l' as u16, 'e' as u16, '_' as u16,
    'I' as u16, 'n' as u16, 's' as u16, 't' as u16, 'a' as u16, 'n' as u16, 'c' as u16,
    'e' as u16, 0,
];

#[cfg(target_os = "windows")]
const EVENT_NAME: &[u16] = &[
    'L' as u16, 'o' as u16, 'c' as u16, 'a' as u16, 'l' as u16, '\\' as u16, 'A' as u16,
    'I' as u16, '_' as u16, 'A' as u16, 'P' as u16, 'I' as u16, '_' as u16, 'M' as u16,
    'o' as u16, 'n' as u16, 'i' as u16, 't' as u16, 'o' as u16, 'r' as u16, '_' as u16,
    'S' as u16, 'i' as u16, 'n' as u16, 'g' as u16, 'l' as u16, 'e' as u16, '_' as u16,
    'I' as u16, 'n' as u16, 's' as u16, 't' as u16, 'a' as u16, 'n' as u16, 'c' as u16,
    'e' as u16, '_' as u16, 'E' as u16, 'v' as u16, 'e' as u16, 'n' as u16, 't' as u16, 0,
];

#[cfg(target_os = "windows")]
pub struct Guard {
    pub event: HANDLE,
    mutex: HANDLE,
}

#[cfg(target_os = "windows")]
unsafe impl Send for Guard {}
#[cfg(target_os = "windows")]
unsafe impl Sync for Guard {}

#[cfg(target_os = "windows")]
impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.event);
            CloseHandle(self.mutex);
        }
    }
}

#[cfg(target_os = "windows")]
pub enum Acquire {
    Primary(Guard),
    Secondary,
}

#[cfg(target_os = "windows")]
pub fn acquire() -> Acquire {
    unsafe {
        let mutex = CreateMutexW(null_mut(), 1, MUTEX_NAME.as_ptr());
        if mutex.is_null() {
            return Acquire::Primary(Guard {
                mutex: null_mut(),
                event: null_mut(),
            });
        }
        let secondary = GetLastError() == ERROR_ALREADY_EXISTS;
        let event = if secondary {
            OpenEventW(EVENT_MODIFY_STATE | SYNCHRONIZE, 0, EVENT_NAME.as_ptr())
        } else {
            CreateEventW(null_mut(), 0, 0, EVENT_NAME.as_ptr())
        };
        if secondary {
            if !event.is_null() {
                SetEvent(event);
                CloseHandle(event);
            }
            CloseHandle(mutex);
            Acquire::Secondary
        } else {
            Acquire::Primary(Guard { mutex, event })
        }
    }
}

#[cfg(target_os = "windows")]
pub fn wait_for_activation(event: HANDLE) {
    unsafe {
        WaitForSingleObject(event, INFINITE);
    }
}

#[cfg(not(target_os = "windows"))]
pub struct Guard;

#[cfg(not(target_os = "windows"))]
pub enum Acquire {
    Primary(Guard),
}

#[cfg(not(target_os = "windows"))]
pub fn acquire() -> Acquire {
    Acquire::Primary(Guard)
}
