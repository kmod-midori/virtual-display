use std::{ffi::CString, str::FromStr};

use windows::{
    core::{PCSTR, PCWSTR},
    Win32::{
        Foundation::{
            CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, INVALID_HANDLE_VALUE,
        },
        Security::PSECURITY_DESCRIPTOR,
        System::{
            Memory::{
                CreateFileMappingW, LocalFree, MapViewOfFile, UnmapViewOfFile, FILE_MAP_ALL_ACCESS,
                PAGE_READWRITE,
            },
            Threading::{WaitForMultipleObjects, WaitForSingleObject},
        },
    },
};

fn convert_to_utf16(s: &str) -> Vec<u16> {
    let mut buf = s.encode_utf16().collect::<Vec<_>>();
    buf.push(0);
    buf
}

pub enum WaitState {
    Signaled(usize),
    Abandoned(usize),
    TimedOut,
}

pub trait Waitable {
    fn handle(&self) -> HANDLE;
    fn wait(&self, timeout: Option<u32>) -> windows::core::Result<WaitState> {
        let result = unsafe { WaitForSingleObject(self.handle(), timeout.unwrap_or(0xFFFFFFFF)) };

        match result.0 {
            0 => Ok(WaitState::Signaled(0)),
            0x80 => Ok(WaitState::Abandoned(0)),
            0x102 => Ok(WaitState::TimedOut),
            _ => Err(windows::core::Error::from_win32()),
        }
    }
}

pub fn wait_multiple(
    handles: &[&dyn Waitable],
    timeout: Option<u32>,
) -> windows::core::Result<WaitState> {
    let handle_count = handles.len();
    assert!(handle_count <= 64);

    let handles = handles.iter().map(|h| h.handle()).collect::<Vec<_>>();

    let result =
        unsafe { WaitForMultipleObjects(&handles, false, timeout.unwrap_or(0xFFFFFFFF)).0 };

    match result {
        0..=63 => {
            // WAIT_OBJECT_0
            Ok(WaitState::Signaled(result as usize))
        }
        128..=191 => {
            // WAIT_ABANDONED_0
            Ok(WaitState::Abandoned(result as usize - 128))
        }
        0x102 => {
            // WAIT_TIMEOUT
            Ok(WaitState::TimedOut)
        }
        _ => {
            // WAIT_FAILED
            Err(windows::core::Error::from_win32())
        }
    }
}

pub struct SecurityDescriptor {
    raw: PSECURITY_DESCRIPTOR,
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.raw.0 as _);
        }
    }
}

impl FromStr for SecurityDescriptor {
    type Err = windows::core::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cs = CString::new(s).unwrap();

        let mut security_descriptor = Default::default();
        unsafe {
            windows::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorA(
                // GENERIC_READ | GENERIC_WRITE | EVENT_ALL_ACCESS | MUTEX_ALL_ACCESS
                PCSTR::from_raw(cs.as_ptr() as *const _),
                windows::Win32::Security::Authorization::SDDL_REVISION_1,
                &mut security_descriptor,
                None,
            )
            .ok()?;
        };
        Ok(Self {
            raw: security_descriptor,
        })
    }
}

pub struct Mutex {
    handle: HANDLE,
}

impl Mutex {
    pub fn new(
        name: &str,
        security_descriptor: Option<&SecurityDescriptor>,
    ) -> windows::core::Result<Self> {
        let name = convert_to_utf16(name);

        let attrs = security_descriptor.map(|sd| windows::Win32::Security::SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<windows::Win32::Security::SECURITY_ATTRIBUTES>() as _,
            lpSecurityDescriptor: sd.raw.0,
            bInheritHandle: false.into(),
        });

        let handle = unsafe {
            windows::Win32::System::Threading::CreateMutexW(
                attrs.as_ref().map(|a| a as *const _),
                false,
                PCWSTR::from_raw(name.as_ptr()),
            )?
        };

        Ok(Self { handle })
    }

    pub fn lock(&self) -> windows::core::Result<MutexGuard> {
        let result = unsafe {
            windows::Win32::System::Threading::WaitForSingleObject(self.handle, 0xFFFFFFFF)
        };

        match result.0 {
            0x0 | 0x80 => Ok(MutexGuard { mutex: self }),
            _ => Err(windows::core::Error::from_win32()),
        }
    }
}

impl Drop for Mutex {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

pub struct MutexGuard<'a> {
    mutex: &'a Mutex,
}

impl Drop for MutexGuard<'_> {
    fn drop(&mut self) {
        unsafe {
            windows::Win32::System::Threading::ReleaseMutex(self.mutex.handle);
        }
    }
}

pub struct Event {
    handle: HANDLE,
}

impl Event {
    pub fn new(
        name: &str,
        security_descriptor: Option<&SecurityDescriptor>,
        manual_reset: bool,
        initial_state: bool,
    ) -> windows::core::Result<Self> {
        let name = convert_to_utf16(name);

        let attrs = security_descriptor.map(|sd| windows::Win32::Security::SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<windows::Win32::Security::SECURITY_ATTRIBUTES>() as _,
            lpSecurityDescriptor: sd.raw.0,
            bInheritHandle: false.into(),
        });

        let handle = unsafe {
            windows::Win32::System::Threading::CreateEventW(
                attrs.as_ref().map(|a| a as *const _),
                manual_reset,
                initial_state,
                PCWSTR::from_raw(name.as_ptr()),
            )?
        };

        Ok(Self { handle })
    }
}

impl Waitable for Event {
    fn handle(&self) -> HANDLE {
        self.handle
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

pub struct FileMapping {
    handle: HANDLE,
    ptr: *mut u8,
    len: usize,
    // buf: &'static mut [u8],
}

impl FileMapping {
    // Returns a new file mapping and whether the file already exists.
    pub unsafe fn new(
        name: &str,
        security_descriptor: Option<&SecurityDescriptor>,
        size: usize,
    ) -> windows::core::Result<(Self, bool)> {
        let name = convert_to_utf16(name);

        let attrs = security_descriptor.map(|sd| windows::Win32::Security::SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<windows::Win32::Security::SECURITY_ATTRIBUTES>() as _,
            lpSecurityDescriptor: sd.raw.0,
            bInheritHandle: false.into(),
        });

        let handle = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                attrs.as_ref().map(|a| a as *const _),
                PAGE_READWRITE,
                (size >> 32) as _,
                size as _,
                PCWSTR::from_raw(name.as_ptr()),
            )?
        };

        let buf_ptr = MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, size as _);
        if buf_ptr.is_null() {
            return Err(windows::core::Error::from_win32());
        }

        let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;

        Ok((
            Self {
                handle,
                ptr: buf_ptr as *mut u8,
                len: size,
            },
            already_exists,
        ))
    }
    
    /// # Safety
    /// Since this file is mapped as read-write, it is possible to modify the file
    /// from other processes.
    pub unsafe fn buf(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// # Safety
    /// Since this file is mapped as read-write, it is possible to modify the file
    /// from other processes.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn buf_mut(&self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl Drop for FileMapping {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(self.ptr as _);
            CloseHandle(self.handle);
        }
    }
}

unsafe impl Send for FileMapping {}
unsafe impl Sync for FileMapping {}
