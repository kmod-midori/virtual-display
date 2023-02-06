use std::ffi::c_void;

use nvcodec_sys as ffi;

use crate::{check_error, guid::BufferFormat, nvenc_api_struct_version, Library, Result};

pub struct InputBuffer {
    library: Library,
    encoder: *mut c_void,
    format: BufferFormat,
    pub(crate) buffer: *mut c_void,
    height: u32,
    pub(crate) locked: bool,
}

impl Drop for InputBuffer {
    fn drop(&mut self) {
        unsafe {
            check_error(self.library.0.fnlist.nvEncUnlockInputBuffer.unwrap()(
                self.encoder,
                self.buffer,
            ))
            .expect("Failed to unlock input buffer");
            check_error(self.library.0.fnlist.nvEncDestroyInputBuffer.unwrap()(
                self.encoder,
                self.buffer,
            ))
            .expect("Failed to destroy input buffer");
        }
    }
}

impl InputBuffer {
    pub(crate) fn new(
        library: Library,
        encoder: *mut c_void,
        width: u32,
        height: u32,
        format: BufferFormat,
    ) -> Result<Self> {
        let mut args: ffi::NV_ENC_CREATE_INPUT_BUFFER = unsafe { std::mem::zeroed() };
        args.version = nvenc_api_struct_version(1);
        args.width = width;
        args.height = height;
        args.bufferFmt = format.into();

        unsafe {
            check_error(library.0.fnlist.nvEncCreateInputBuffer.unwrap()(
                encoder, &mut args,
            ))?;
        }

        Ok(Self {
            library,
            encoder,
            format,
            buffer: args.inputBuffer,
            height,
            locked: false,
        })
    }

    pub(crate) fn lock(&mut self) -> Result<LockedInputBuffer<'_>> {
        assert!(!self.locked, "Input buffer already locked");

        let mut args: ffi::NV_ENC_LOCK_INPUT_BUFFER = unsafe { std::mem::zeroed() };
        args.version = nvenc_api_struct_version(1);
        args.inputBuffer = self.buffer;

        unsafe {
            check_error(self.library.0.fnlist.nvEncLockInputBuffer.unwrap()(
                self.encoder,
                &mut args,
            ))?;
        }

        let pitch = args.pitch as usize;
        let ptr = args.bufferDataPtr as *mut u8;

        let slice = unsafe { std::slice::from_raw_parts_mut(ptr, pitch * self.height as usize) };

        self.locked = true;

        Ok(LockedInputBuffer {
            buffer: self,
            data: slice,
            stride: pitch,
        })
    }

    pub(crate) fn format(&self) -> BufferFormat {
        self.format
    }
}

pub struct LockedInputBuffer<'b> {
    pub(crate) buffer: &'b mut InputBuffer,
    data: &'b mut [u8],
    stride: usize,
}

impl<'b> LockedInputBuffer<'b> {
    pub fn data(&mut self) -> &mut [u8] {
        self.data
    }

    pub fn stride(&self) -> usize {
        self.stride
    }
}

impl Drop for LockedInputBuffer<'_> {
    fn drop(&mut self) {
        unsafe {
            check_error(
                self.buffer.library.0.fnlist.nvEncUnlockInputBuffer.unwrap()(
                    self.buffer.encoder,
                    self.buffer.buffer,
                ),
            )
            .expect("Failed to unlock input buffer");
        }
        self.buffer.locked = false;
    }
}

pub struct OutputBuffer {
    library: Library,
    encoder: *mut c_void,
    pub(crate) buffer: *mut c_void,
}

impl Drop for OutputBuffer {
    fn drop(&mut self) {
        unsafe {
            check_error(self.library.0.fnlist.nvEncUnlockBitstream.unwrap()(
                self.encoder,
                self.buffer,
            ))
            .expect("Failed to unlock bitstream buffer");
            check_error(self.library.0.fnlist.nvEncDestroyBitstreamBuffer.unwrap()(
                self.encoder,
                self.buffer,
            ))
            .expect("Failed to destroy bitstream buffer");
        }
    }
}

impl OutputBuffer {
    pub(crate) fn new(library: Library, encoder: *mut c_void) -> Result<Self> {
        let mut args: ffi::NV_ENC_CREATE_BITSTREAM_BUFFER = unsafe { std::mem::zeroed() };
        args.version = nvenc_api_struct_version(1);

        unsafe {
            check_error(library.0.fnlist.nvEncCreateBitstreamBuffer.unwrap()(
                encoder, &mut args,
            ))?;
        }

        Ok(Self {
            library,
            encoder,
            buffer: args.bitstreamBuffer,
        })
    }

    pub(crate) fn lock(&mut self) -> Result<LockedOutputBuffer<'_>> {
        let mut args: ffi::NV_ENC_LOCK_BITSTREAM = unsafe { std::mem::zeroed() };
        args.version = nvenc_api_struct_version(1);
        args.outputBitstream = self.buffer;

        unsafe {
            check_error(self.library.0.fnlist.nvEncLockBitstream.unwrap()(
                self.encoder,
                &mut args,
            ))?;
        }

        let ptr = args.bitstreamBufferPtr as *mut u8;

        let slice = unsafe { std::slice::from_raw_parts_mut(ptr, args.bitstreamSizeInBytes as usize) };

        Ok(LockedOutputBuffer {
            buffer: self,
            data: slice,
            pts: args.outputTimeStamp,
        })
    }
}

pub(crate) struct LockedOutputBuffer<'b> {
    pub(crate) buffer: &'b mut OutputBuffer,
    data: &'b [u8],
    pts: u64
}

impl<'b> LockedOutputBuffer<'b> {
    pub(crate) fn data(&self) -> &[u8] {
        self.data
    }

    pub(crate) fn pts(&self) -> u64 {
        self.pts
    }
}

impl Drop for LockedOutputBuffer<'_> {
    fn drop(&mut self) {
        unsafe {
            check_error(
                self.buffer.library.0.fnlist.nvEncUnlockBitstream.unwrap()(
                    self.buffer.encoder,
                    self.buffer.buffer,
                ),
            )
            .expect("Failed to unlock bitstream buffer");
        }
    }
}