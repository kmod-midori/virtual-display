use std::ptr::null_mut;

use crate::{check_error, Result};

use mfx_dispatch_sys as ffi;

fn get_adapters_number() -> Result<usize> {
    let mut len = 0;

    unsafe {
        check_error(ffi::MFXQueryAdaptersNumber(&mut len))?;
    }

    Ok(len as usize)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdapterType {
    /// Discrete graphics adapter.
    Discrete,
    /// Integrated graphics adapter.
    Integrated,
    /// Unknown type.
    Unknown,
}

#[derive(Debug)]
pub struct Adapter {
    inner: ffi::mfxAdapterInfo,
}

impl Adapter {
    pub fn enumerate() -> Result<Vec<Adapter>> {
        let len = get_adapters_number()?;

        let mut adapters = vec![unsafe { std::mem::zeroed() }; len];
        let mut adapters_info = ffi::mfxAdaptersInfo {
            Adapters: adapters.as_mut_ptr(),
            NumAlloc: len as _,
            NumActual: 0,
            reserved: [0; 4],
        };

        unsafe {
            check_error(ffi::MFXQueryAdapters(null_mut(), &mut adapters_info))?;
        }

        let adapters = adapters
            .into_iter()
            .take(adapters_info.NumActual as usize)
            .map(|inner| Adapter { inner })
            .collect();

        Ok(adapters)
    }
}

impl Adapter {
    pub fn adapter_type(&self) -> AdapterType {
        match self.inner.Platform.MediaAdapterType as i32 {
            ffi::mfxMediaAdapterType_MFX_MEDIA_DISCRETE => AdapterType::Discrete,
            ffi::mfxMediaAdapterType_MFX_MEDIA_INTEGRATED => AdapterType::Integrated,
            _ => AdapterType::Unknown,
        }
    }

    pub fn device_id(&self) -> u16 {
        self.inner.Platform.DeviceId
    }

    pub fn number(&self) -> u32 {
        self.inner.Number
    }
}
