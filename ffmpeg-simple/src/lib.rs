#![allow(clippy::new_without_default)]

use std::ptr::{null, null_mut};

use error::check_error;
pub use ffmpeg_sys as ffi;

pub mod error;
use error::Result;

pub mod codec;
pub use codec::Codec;

pub fn init_logging() {
    unsafe {
        ffi::av_log_set_callback(Some(ffi::av_log_default_callback));
        ffi::av_log_set_level(ffi::AV_LOG_VERBOSE as i32);
    }
}

pub struct Plane<'data> {
    data: &'data [u8],
    line_size: usize,
}

impl<'data> Plane<'data> {
    pub fn data(&self) -> &[u8] {
        self.data
    }

    pub fn line_size(&self) -> usize {
        self.line_size
    }
}

pub struct PlaneMut<'data> {
    data: &'data mut [u8],
    line_size: usize,
}

impl<'data> PlaneMut<'data> {
    pub fn data(&mut self) -> &mut [u8] {
        self.data
    }

    pub fn line_size(&self) -> usize {
        self.line_size
    }
}

pub struct Frame {
    raw: *mut ffi::AVFrame,
    line_sizes: [usize; 4],
    plane_sizes: [usize; 4],
}

impl Frame {
    pub fn planes(&self) -> [Option<Plane>; 4] {
        let mut planes = [None, None, None, None];

        for (i, plane) in planes.iter_mut().enumerate() {
            unsafe {
                if !(*self.raw).data[i].is_null() {
                    *plane = Some(Plane {
                        data: std::slice::from_raw_parts((*self.raw).data[i], self.plane_sizes[i]),
                        line_size: self.line_sizes[i],
                    });
                }
            }
        }

        planes
    }

    pub fn planes_mut(&mut self) -> [Option<PlaneMut>; 4] {
        let mut planes = [None, None, None, None];

        for (i, plane) in planes.iter_mut().enumerate() {
            unsafe {
                if !(*self.raw).data[i].is_null() {
                    *plane = Some(PlaneMut {
                        data: std::slice::from_raw_parts_mut(
                            (*self.raw).data[i],
                            self.plane_sizes[i],
                        ),
                        line_size: self.line_sizes[i],
                    });
                }
            }
        }

        planes
    }

    pub fn line_sizes(&self) -> [usize; 4] {
        self.line_sizes
    }

    pub fn plane_sizes(&self) -> [usize; 4] {
        self.plane_sizes
    }

    pub fn height(&self) -> usize {
        unsafe { (*self.raw).height as usize }
    }

    pub fn width(&self) -> usize {
        unsafe { (*self.raw).width as usize }
    }

    pub fn as_ptr(&self) -> *const ffi::AVFrame {
        self.raw
    }

    pub fn as_mut_ptr(&mut self) -> *mut ffi::AVFrame {
        self.raw
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        unsafe {
            ffi::av_frame_free(&mut self.raw);
        }
    }
}

#[derive(Debug)]
pub struct Packet {
    raw: *mut ffi::AVPacket,
}

impl Packet {
    pub fn pts(&self) -> i64 {
        unsafe { (*self.raw).pts }
    }

    pub fn data(&self) -> Option<&[u8]> {
        unsafe {
            if (*self.raw).data.is_null() {
                None
            } else {
                Some(std::slice::from_raw_parts(
                    (*self.raw).data,
                    (*self.raw).size as usize,
                ))
            }
        }
    }

    pub fn data_mut(&mut self) -> Option<&mut [u8]> {
        unsafe {
            if (*self.raw).data.is_null() {
                None
            } else {
                Some(std::slice::from_raw_parts_mut(
                    (*self.raw).data,
                    (*self.raw).size as usize,
                ))
            }
        }
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        unsafe {
            ffi::av_packet_free(&mut self.raw);
        }
    }
}

#[derive(Debug)]
pub struct HwDeviceContext {
    raw: *mut ffi::AVBufferRef,
}

impl HwDeviceContext {
    pub fn new(type_: ffi::AVHWDeviceType) -> Result<Self> {
        unsafe {
            let mut raw = null_mut();
            check_error(ffi::av_hwdevice_ctx_create(
                &mut raw,
                type_,
                null(),
                null_mut(),
                0,
            ))?;
            Ok(Self { raw })
        }
    }
}

impl Drop for HwDeviceContext {
    fn drop(&mut self) {
        unsafe {
            ffi::av_buffer_unref(&mut self.raw);
        }
    }
}

pub struct CodecContext {
    raw: *mut ffi::AVCodecContext,
    hw_device_ctx: Option<HwDeviceContext>,
}

impl CodecContext {
    pub fn new(codec: Codec) -> Self {
        CodecContext {
            raw: unsafe { ffi::avcodec_alloc_context3(codec.raw) },
            hw_device_ctx: None,
        }
    }

    pub fn set_hw_device_ctx(&mut self, hw_device_ctx: HwDeviceContext) -> &mut Self {
        unsafe {
            (*self.raw).hw_device_ctx = ffi::av_buffer_ref(hw_device_ctx.raw);
        }
        self.hw_device_ctx = Some(hw_device_ctx);
        self
    }

    pub fn set_size(&mut self, width: u32, height: u32) -> &mut Self {
        unsafe {
            (*self.raw).width = width as _;
            (*self.raw).height = height as _;
        }
        self
    }

    pub fn set_time_base(&mut self, num: u32, den: u32) -> &mut Self {
        unsafe {
            (*self.raw).time_base.num = num as _;
            (*self.raw).time_base.den = den as _;
        }
        self
    }

    pub fn set_framerate(&mut self, num: u32, den: u32) -> &mut Self {
        unsafe {
            (*self.raw).framerate.num = num as _;
            (*self.raw).framerate.den = den as _;
        }
        self
    }

    pub fn set_pix_fmt(&mut self, pix_fmt: ffi::AVPixelFormat) -> &mut Self {
        unsafe {
            (*self.raw).pix_fmt = pix_fmt;
        }
        self
    }

    pub fn set_option(&mut self, key: &str, value: &str) -> Result<&mut Self> {
        let key = std::ffi::CString::new(key).unwrap();
        let value = std::ffi::CString::new(value).unwrap();

        unsafe {
            check_error(ffi::av_opt_set(
                (*self.raw).priv_data,
                key.as_ptr(),
                value.as_ptr(),
                0,
            ))?;
        }

        Ok(self)
    }

    pub fn open(self) -> error::Result<OpenedCodecContext> {
        unsafe {
            check_error(ffi::avcodec_open2(self.raw, std::ptr::null(), null_mut()))?;

            let frame = ffi::av_frame_alloc();
            (*frame).width = (*self.raw).width;
            (*frame).height = (*self.raw).height;
            (*frame).format = (*self.raw).pix_fmt;

            check_error(ffi::av_frame_get_buffer(frame, 0))?;

            let mut line_sizes = [0; 4];
            let mut plane_sizes = [0; 4];

            for (i, line_size) in line_sizes.iter_mut().enumerate() {
                *line_size = (*frame).linesize[i] as usize;
            }

            ffi::av_image_fill_plane_sizes(
                plane_sizes.as_mut_ptr(),
                (*frame).format,
                (*frame).height,
                line_sizes.as_ptr() as *const _,
            );

            let packet = ffi::av_packet_alloc();

            Ok(OpenedCodecContext {
                inner: self,
                frame: Frame {
                    raw: frame,
                    line_sizes,
                    plane_sizes,
                },
                packet: Packet { raw: packet },
            })
        }
    }
}

impl Drop for CodecContext {
    fn drop(&mut self) {
        unsafe {
            ffi::avcodec_free_context(&mut self.raw);
        }
    }
}

pub struct OpenedCodecContext {
    inner: CodecContext,
    frame: Frame,
    packet: Packet,
}

impl OpenedCodecContext {
    pub fn close(self) -> Result<CodecContext> {
        unsafe {
            check_error(ffi::avcodec_close(self.inner.raw))?;
        }
        Ok(self.inner)
    }

    pub fn request_frame(&mut self) -> Result<&mut Frame> {
        unsafe {
            check_error(ffi::av_frame_make_writable(self.frame.raw))?;
        }
        Ok(&mut self.frame)
    }

    pub fn send_frame(&mut self, pts: i64) -> Result<()> {
        unsafe {
            (*self.frame.raw).pts = pts;
            check_error(ffi::avcodec_send_frame(self.inner.raw, self.frame.raw))?;
        }
        Ok(())
    }

    pub fn receive_packet(&mut self) -> Result<Option<&mut Packet>> {
        unsafe {
            // This always calls `unref` before doing anything.
            let ret = ffi::avcodec_receive_packet(self.inner.raw, self.packet.raw);

            if ret == crate::error::AVERROR_EAGAIN || ret == crate::error::AVERROR_EOF {
                Ok(None)
            } else {
                check_error(ret)?;
                Ok(Some(&mut self.packet))
            }
        }
    }
}
