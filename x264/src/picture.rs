use core::mem::MaybeUninit;

use x264_sys::*;

/// Output information about an encoded frame.
pub struct Picture {
    pub(crate) raw: x264_picture_t
}

impl Picture {
    /// Creates a new picture.
    pub fn new() -> Self {
        unsafe {
            let mut raw = MaybeUninit::<x264_picture_t>::uninit();
            x264_picture_init(raw.as_mut_ptr());
            Self { raw: raw.assume_init() }
        }
    }

    /// Whether the picture is a keyframe.
    pub fn keyframe(&self) -> bool {
        self.raw.b_keyframe != 0
    }

    /// The presentation timestamp.
    pub fn pts(&self) -> i64 {
        self.raw.i_pts
    }

    /// The decoding timestamp.
    pub fn dts(&self) -> i64 {
        self.raw.i_dts
    }

    #[doc(hidden)]
    pub unsafe fn from_raw(raw: x264_picture_t) -> Self {
        Self { raw }
    }
}
