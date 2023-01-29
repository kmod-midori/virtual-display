use std::mem::ManuallyDrop;

use windows::Win32::{
    Foundation::{VARIANT_FALSE, VARIANT_TRUE},
    System::Com::{
        VARENUM, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_I4, VT_UI4,
    },
};

/// A wrapper around COM's `VARIANT` type.
#[repr(transparent)]
pub struct Variant(VARIANT);
impl Variant {
    pub fn new(num: VARENUM, contents: VARIANT_0_0_0) -> Variant {
        Variant(VARIANT {
            Anonymous: VARIANT_0 {
                Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                    vt: num,
                    wReserved1: 0,
                    wReserved2: 0,
                    wReserved3: 0,
                    Anonymous: contents,
                }),
            },
        })
    }

    pub fn as_ptr(&self) -> *const VARIANT {
        &self.0
    }
}

impl From<i32> for Variant {
    fn from(value: i32) -> Variant {
        Variant::new(VT_I4, VARIANT_0_0_0 { lVal: value })
    }
}

impl From<u32> for Variant {
    fn from(value: u32) -> Variant {
        Variant::new(VT_UI4, VARIANT_0_0_0 { ulVal: value })
    }
}

impl From<bool> for Variant {
    fn from(value: bool) -> Variant {
        Variant::new(
            VT_BOOL,
            VARIANT_0_0_0 {
                boolVal: if value { VARIANT_TRUE } else { VARIANT_FALSE },
            },
        )
    }
}
