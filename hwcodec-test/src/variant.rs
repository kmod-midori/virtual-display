use std::mem::ManuallyDrop;

use windows::{
    core::BSTR,
    Win32::{
        Foundation::{VARIANT_FALSE, VARIANT_TRUE},
        System::Com::{
            VARENUM, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_BSTR, VT_I4,
            VT_UI4,
        },
    },
};

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
// impl From<String> for Variant {
//     fn from(value: String) -> Variant {
//         Variant::new(
//             VT_BSTR,
//             VARIANT_0_0_0 {
//                 bstrVal: ManuallyDrop::new(BSTR::from(value)),
//             },
//         )
//     }
// }

// impl From<&str> for Variant {
//     fn from(value: &str) -> Variant {
//         Variant::from(value.to_string())
//     }
// }

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

// impl Drop for Variant {
//     fn drop(&mut self) {
//         match VARENUM(unsafe { self.0.Anonymous.Anonymous.vt.0 }) {
//             VT_BSTR => unsafe { drop(&mut &self.0.Anonymous.Anonymous.Anonymous.bstrVal) },
//             _ => {}
//         }
//         unsafe { drop(&mut self.0.Anonymous.Anonymous) }
//     }
// }
