use self::private::InputBufferInternal;
use mfx_dispatch_sys as ffi;

pub(crate) mod private {
    use mfx_dispatch_sys as ffi;

    pub trait InputBufferInternal {
        /// `buffer_width` and `buffer_height` must be aligned to 32.
        fn allocate(frame_info: ffi::mfxFrameInfo, buffer_width: u32, buffer_height: u32) -> Self;

        fn surface(&self) -> &ffi::mfxFrameSurface1;

        fn surface_mut(&mut self) -> &mut ffi::mfxFrameSurface1;

        fn locked(&self) -> bool {
            self.surface().Data.Locked != 0
        }
    }
}

/// See https://github.com/Intel-Media-SDK/MediaSDK/blob/master/samples/sample_common/src/sysmem_allocator.cpp#L77
/// for the actual layout of these formats.
#[derive(Debug, Clone, Copy)]
pub enum InputFormat {
    /// `Y` followed by a single interleaved `UV` plane.
    NV12,
    /// Actual layout: `BGR`.
    ///
    /// You will need to use `RGB4` if you want to directly submit
    /// to the encoder.
    RGB3,
    /// This is actually `BGRA`, but it is unfortunate that we
    /// are stuck in little-endian land.
    ///
    /// Note that directly encoding from this format
    /// is only supported on 6th generation Intel CPUs and later.
    RGB4,
}

impl InputFormat {
    pub(crate) fn fill_config(&self, config: &mut ffi::mfxVideoParam) {
        config.__bindgen_anon_1.mfx.FrameInfo.FourCC = match self {
            InputFormat::NV12 => ffi::MFX_FOURCC_NV12 as _,
            InputFormat::RGB3 => ffi::MFX_FOURCC_RGB3 as _,
            InputFormat::RGB4 => ffi::MFX_FOURCC_RGB4 as _,
        }
    }
}

pub trait InputBuffer: private::InputBufferInternal {
    const FORMAT: InputFormat;
}

/// A buffer for [`InputFormat::NV12`].
pub struct Nv12Buffer {
    data: Vec<u8>,
    surface: ffi::mfxFrameSurface1,
}

impl Nv12Buffer {
    /// Actual height of the buffer.
    pub fn buffer_height(&self) -> usize {
        unsafe { self.surface.Info.__bindgen_anon_1.__bindgen_anon_1.Height as usize }
    }

    /// Returns a mutable reference to the underlying buffer.
    /// 
    /// Note that the Y plane may have extra rows at the end,
    /// use the [`Nv12Buffer::buffer_height`] function to get
    /// the actual height of the buffer.
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
    
    /// The stride (actual width) of the buffer in bytes.
    pub fn stride(&self) -> usize {
        unsafe { self.surface.Info.__bindgen_anon_1.__bindgen_anon_1.Width as usize }
    }
}

impl InputBufferInternal for Nv12Buffer {
    fn allocate(frame_info: ffi::mfxFrameInfo, buffer_width: u32, buffer_height: u32) -> Self {
        let buffer_size = buffer_width * buffer_height * 3 / 2;
        let mut buffer = vec![0u8; buffer_size as usize];
        let buffer_ptr = buffer.as_mut_ptr();

        let mut surface: ffi::mfxFrameSurface1 = unsafe { std::mem::zeroed() };
        surface.Info = frame_info;

        unsafe {
            let y = buffer_ptr;
            let uv = buffer_ptr.offset(buffer_width as isize * buffer_height as isize);
            surface.Data.__bindgen_anon_3.Y = y;
            surface.Data.__bindgen_anon_4.UV = uv;

            surface.Data.PitchHigh = 0;
            surface.Data.__bindgen_anon_2.PitchLow = buffer_width as u16;
        }

        Self {
            data: buffer,
            surface,
        }
    }

    fn surface(&self) -> &ffi::mfxFrameSurface1 {
        &self.surface
    }

    fn surface_mut(&mut self) -> &mut ffi::mfxFrameSurface1 {
        &mut self.surface
    }
}

impl InputBuffer for Nv12Buffer {
    const FORMAT: InputFormat = InputFormat::NV12;
}

/// A buffer for [`InputFormat::RGB4`].
pub struct BgraBuffer {
    data: Vec<u8>,
    surface: ffi::mfxFrameSurface1,
}

impl BgraBuffer {
    /// Returns a mutable reference to the underlying buffer.
    /// 
    /// Note that the buffer may have extra rows and columns.
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// The stride (actual width) of the buffer in bytes.
    pub fn stride(&self) -> usize {
        unsafe { self.surface.Info.__bindgen_anon_1.__bindgen_anon_1.Width as usize * 4 }
    }
}

impl InputBufferInternal for BgraBuffer {
    fn allocate(frame_info: ffi::mfxFrameInfo, buffer_width: u32, buffer_height: u32) -> Self {
        let buffer_size = buffer_width * buffer_height * 4;
        let mut buffer = vec![0u8; buffer_size as usize];
        let buffer_ptr = buffer.as_mut_ptr();

        let mut surface: ffi::mfxFrameSurface1 = unsafe { std::mem::zeroed() };
        surface.Info = frame_info;

        unsafe {
            surface.Data.__bindgen_anon_5.B = buffer_ptr;
            surface.Data.__bindgen_anon_4.G = buffer_ptr.offset(1);
            surface.Data.__bindgen_anon_3.R = buffer_ptr.offset(2);
            surface.Data.A = buffer_ptr.offset(3);

            let pitch = buffer_width * 4;
            surface.Data.PitchHigh = (pitch / (1 << 16)) as u16;
            surface.Data.__bindgen_anon_2.PitchLow = (pitch % (1 << 16)) as u16;
        }

        Self {
            data: buffer,
            surface,
        }
    }

    fn surface(&self) -> &ffi::mfxFrameSurface1 {
        &self.surface
    }

    fn surface_mut(&mut self) -> &mut ffi::mfxFrameSurface1 {
        &mut self.surface
    }
}

impl InputBuffer for BgraBuffer {
    const FORMAT: InputFormat = InputFormat::RGB4;
}
