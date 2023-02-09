use mfx_dispatch_sys as ffi;

/// See <https://github.com/Intel-Media-SDK/MediaSDK/blob/master/samples/sample_common/src/sysmem_allocator.cpp#L77>
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

    pub fn buffer_size(&self, buffer_width: u32, buffer_height: u32) -> usize {
        match self {
            InputFormat::NV12 => {
                let y_size = buffer_width * buffer_height;
                let uv_size = y_size / 2;
                (y_size + uv_size) as usize
            }
            InputFormat::RGB3 => (buffer_width * buffer_height * 3) as usize,
            InputFormat::RGB4 => (buffer_width * buffer_height * 4) as usize,
        }
    }
}

pub struct InputBuffer {
    format: InputFormat,
    data: Vec<u8>,
    surface: ffi::mfxFrameSurface1,
}

impl InputBuffer {
    /// Create a new input buffer, size of which is determined by the
    /// given `format` and `frame_info`.
    pub fn new(format: InputFormat, frame_info: ffi::mfxFrameInfo) -> Self {
        let buffer_width = unsafe { frame_info.__bindgen_anon_1.__bindgen_anon_1.Width as u32 };
        let buffer_height = unsafe { frame_info.__bindgen_anon_1.__bindgen_anon_1.Height as u32 };
        let buffer_size = format.buffer_size(buffer_width, buffer_height);

        let mut buffer = vec![0u8; buffer_size];
        let buffer_ptr = buffer.as_mut_ptr();

        let mut surface: ffi::mfxFrameSurface1 = unsafe { std::mem::zeroed() };
        surface.Info = frame_info;

        match format {
            InputFormat::NV12 => unsafe {
                let y = buffer_ptr;
                let uv = buffer_ptr.offset(buffer_width as isize * buffer_height as isize);
                surface.Data.__bindgen_anon_3.Y = y;
                surface.Data.__bindgen_anon_4.UV = uv;

                surface.Data.PitchHigh = 0;
                surface.Data.__bindgen_anon_2.PitchLow = buffer_width as u16;
            },
            InputFormat::RGB3 => unsafe {
                let bgr = buffer_ptr;
                surface.Data.__bindgen_anon_5.B = bgr;
                surface.Data.__bindgen_anon_4.G = bgr.offset(1);
                surface.Data.__bindgen_anon_3.R = bgr.offset(2);

                let pitch = buffer_width * 3;
                surface.Data.PitchHigh = (pitch / (1 << 16)) as u16;
                surface.Data.__bindgen_anon_2.PitchLow = (pitch % (1 << 16)) as u16;
            },
            InputFormat::RGB4 => unsafe {
                let bgra = buffer_ptr;
                surface.Data.__bindgen_anon_5.B = bgra;
                surface.Data.__bindgen_anon_4.G = bgra.offset(1);
                surface.Data.__bindgen_anon_3.R = bgra.offset(2);
                surface.Data.A = bgra.offset(3);

                let pitch = buffer_width * 4;
                surface.Data.PitchHigh = (pitch / (1 << 16)) as u16;
                surface.Data.__bindgen_anon_2.PitchLow = (pitch % (1 << 16)) as u16;
            },
        }

        Self {
            format,
            data: buffer,
            surface,
        }
    }

    /// The pixel format of the buffer.
    pub fn format(&self) -> InputFormat {
        self.format
    }

    /// Returns a mutable reference to the underlying buffer.
    ///
    /// Note that thanks to Intel's requirement of padding both width and height,
    /// planar and semi-planar formats may have extra rows at the end of each plane,
    /// use the [`InputBuffer::plane_height`] function to get
    /// the actual height of the buffer.
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn surface_mut(&mut self) -> &mut ffi::mfxFrameSurface1 {
        &mut self.surface
    }

    /// Actual height of each plane in pixels.
    pub fn plane_height(&self) -> usize {
        unsafe { self.surface.Info.__bindgen_anon_1.__bindgen_anon_1.Height as usize }
    }

    /// Stride (actual width) of the buffer in bytes.
    pub fn stride(&self) -> usize {
        unsafe { self.surface.Info.__bindgen_anon_1.__bindgen_anon_1.Width as usize }
    }

    pub(crate) fn locked(&self) -> bool {
        self.surface.Data.Locked != 0
    }
}
