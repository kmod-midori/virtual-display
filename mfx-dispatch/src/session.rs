use std::{mem::MaybeUninit, ptr::null_mut};

use crate::{check_error, Result};

use mfx_dispatch_sys as ffi;

#[derive(Debug)]
pub struct Session {
    pub(crate) raw: ffi::mfxSession,
    impl_: i32,
}

impl Session {
    pub fn new() -> Result<Self> {
        let mut version = ffi::mfxVersion {
            __bindgen_anon_1: ffi::mfxVersion__bindgen_ty_1 { Major: 1, Minor: 0 },
        };
        let mut session = null_mut();

        unsafe {
            check_error(ffi::MFXInit(
                ffi::MFX_IMPL_HARDWARE_ANY,
                &mut version,
                &mut session,
            ))?;
        };

        let mut impl_ = MaybeUninit::uninit();

        unsafe {
            check_error(ffi::MFXQueryIMPL(session, impl_.as_mut_ptr()))?;
        }

        Ok(Self {
            raw: session,
            impl_: unsafe { impl_.assume_init() },
        })
    }

    pub fn implementation(&self) -> i32 {
        self.impl_
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            ffi::MFXClose(self.raw);
        }
    }
}
