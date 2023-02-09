use std::{mem::MaybeUninit, ptr::null_mut};

use buffer::InputBuffer;
use builder::EncoderConfig;
pub use mfx_dispatch_sys as ffi;

mod error;
pub use error::{check_error, Error, Result};
mod session;
pub use session::Session;
pub mod adapter;
pub mod buffer;
pub mod builder;

pub(crate) fn align32(x: u16) -> u16 {
    ((x + 31) >> 5) << 5
}

pub struct Pipeline {
    session: Session,

    sps: Vec<u8>,
    pps: Vec<u8>,

    surfaces: Vec<InputBuffer>,

    encoded_buffer: Vec<u8>,
    encoded_bitstream: ffi::mfxBitstream,
}

unsafe impl Send for Pipeline {}

impl Pipeline {
    pub fn new(config: EncoderConfig) -> Result<Self> {
        let session = Session::new()?;

        let mut this = Self {
            session,

            sps: vec![],
            pps: vec![],

            surfaces: vec![],

            encoded_buffer: vec![],
            encoded_bitstream: unsafe { std::mem::zeroed() },
        };

        this.configure(config, ConfigureMethod::Init)?;

        Ok(this)
    }

    pub fn reset(&mut self, config: EncoderConfig) -> Result<()> {
        self.configure(config, ConfigureMethod::Reset)?;

        Ok(())
    }

    /// The encoder might no longer be usable if this call fails.
    fn configure(&mut self, config: EncoderConfig, method: ConfigureMethod) -> Result<()> {
        let mut param = config.inner;
        let mut opt2 = config.opt2;
        let mut opt3 = config.opt3;
        let format = config.format;

        param.IOPattern = ffi::MFX_IOPATTERN_IN_SYSTEM_MEMORY as u16
            | ffi::MFX_IOPATTERN_OUT_SYSTEM_MEMORY as u16;

        // TODO: Make these configurable.
        param
            .__bindgen_anon_1
            .mfx
            .__bindgen_anon_1
            .__bindgen_anon_1
            .GopRefDist = 1;
        param
            .__bindgen_anon_1
            .mfx
            .__bindgen_anon_1
            .__bindgen_anon_1
            .IdrInterval = 1;

        let ext_params = &mut [
            &mut opt2.Header as *mut ffi::mfxExtBuffer,
            &mut opt3.Header as *mut ffi::mfxExtBuffer,
        ];
        param.ExtParam = ext_params.as_mut_ptr();
        param.NumExtParam = ext_params.len() as _;

        unsafe {
            check_error(ffi::MFXVideoENCODE_Query(
                self.session.raw,
                &mut param,
                &mut param,
            ))?;
        }

        let mut alloc_request = MaybeUninit::uninit();
        unsafe {
            check_error(ffi::MFXVideoENCODE_QueryIOSurf(
                self.session.raw,
                &mut param,
                alloc_request.as_mut_ptr(),
            ))?;
        }
        let alloc_request = unsafe { alloc_request.assume_init() };

        let mut surfaces = vec![];
        for _ in 0..alloc_request.NumFrameSuggested {
            surfaces.push(InputBuffer::new(format, alloc_request.Info));
        }

        unsafe {
            match method {
                ConfigureMethod::Init => {
                    check_error(ffi::MFXVideoENCODE_Init(self.session.raw, &mut param))?;
                }
                ConfigureMethod::Reset => {
                    // Drain remaining frames
                    loop {
                        let mut sync_point = null_mut();
                        match ffi::MFXVideoENCODE_EncodeFrameAsync(
                            self.session.raw,
                            null_mut(),
                            null_mut(),
                            &mut self.encoded_bitstream,
                            &mut sync_point,
                        ) {
                            ffi::mfxStatus_MFX_ERR_MORE_DATA => {
                                // No more frames to drain
                                break;
                            }
                            ffi::mfxStatus_MFX_ERR_NONE => {
                                check_error(ffi::MFXVideoCORE_SyncOperation(
                                    self.session.raw,
                                    sync_point,
                                    ffi::MFX_INFINITE,
                                ))?;
                                // Continue draining
                            }
                            e => {
                                check_error(e)?;
                            }
                        }
                    }
                    // Reset the encoder
                    check_error(ffi::MFXVideoENCODE_Close(self.session.raw))?;
                    check_error(ffi::MFXVideoENCODE_Init(self.session.raw, &mut param))?;
                }
            }
        }

        let mut active_enc_par: ffi::mfxVideoParam = unsafe { std::mem::zeroed() };

        let mut sps_buffer = vec![0u8; 128];
        let mut pps_buffer = vec![0u8; 128];
        let mut coding_option_sps_pps = ffi::mfxExtCodingOptionSPSPPS {
            Header: ffi::mfxExtBuffer {
                BufferId: ffi::MFX_EXTBUFF_CODING_OPTION_SPSPPS as u32,
                BufferSz: std::mem::size_of::<ffi::mfxExtCodingOptionSPSPPS>() as u32,
            },
            SPSBuffer: sps_buffer.as_mut_ptr(),
            PPSBuffer: pps_buffer.as_mut_ptr(),
            SPSBufSize: sps_buffer.len() as u16,
            PPSBufSize: pps_buffer.len() as u16,
            SPSId: 0,
            PPSId: 0,
        };

        let ext_params = &mut [
            &mut coding_option_sps_pps as *mut ffi::mfxExtCodingOptionSPSPPS
                as *mut ffi::mfxExtBuffer,
        ];
        active_enc_par.ExtParam = ext_params.as_mut_ptr();
        active_enc_par.NumExtParam = ext_params.len() as u16;

        unsafe {
            check_error(ffi::MFXVideoENCODE_GetVideoParam(
                self.session.raw,
                &mut active_enc_par,
            ))?;
        }

        let encoded_buffer_size = unsafe {
            active_enc_par
                .__bindgen_anon_1
                .mfx
                .__bindgen_anon_1
                .__bindgen_anon_1
                .BufferSizeInKB as usize
                * 1024
        };
        let mut encoded_buffer = vec![0u8; encoded_buffer_size];

        let mut encoded_bitstream: ffi::mfxBitstream = unsafe { std::mem::zeroed() };
        encoded_bitstream.MaxLength = encoded_buffer_size as u32;
        encoded_bitstream.Data = encoded_buffer.as_mut_ptr();

        self.sps = sps_buffer[..coding_option_sps_pps.SPSBufSize as usize].to_vec();
        self.pps = pps_buffer[..coding_option_sps_pps.PPSBufSize as usize].to_vec();

        self.surfaces = surfaces;

        self.encoded_buffer = encoded_buffer;
        self.encoded_bitstream = encoded_bitstream;

        Ok(())
    }

    /// Get a free surface to encode a frame into.
    ///
    /// Returns the index of the surface and a mutable reference to the buffer of Y and UV planes.
    pub fn get_free_surface(&mut self) -> Option<(usize, &mut InputBuffer)> {
        for (i, buffer) in self.surfaces.iter_mut().enumerate() {
            if !buffer.locked() {
                return Some((i, buffer));
            }
        }

        None
    }

    pub fn sps(&self) -> &[u8] {
        &self.sps
    }

    pub fn pps(&self) -> &[u8] {
        &self.pps
    }

    pub fn encode_frame(
        &mut self,
        surface_index: usize,
        force_keyframe: bool,
    ) -> Result<Option<&[u8]>> {
        let surface = self.surfaces[surface_index].surface_mut();

        let mut sync_point = null_mut();

        let start_instant = std::time::Instant::now();

        let mut ctrl: ffi::mfxEncodeCtrl = unsafe { std::mem::zeroed() };
        if force_keyframe {
            ctrl.FrameType = (ffi::MFX_FRAMETYPE_I | ffi::MFX_FRAMETYPE_IDR) as u16;
        }

        let status = unsafe {
            ffi::MFXVideoENCODE_EncodeFrameAsync(
                self.session.raw,
                &mut ctrl,
                surface,
                &mut self.encoded_bitstream,
                &mut sync_point,
            )
        };

        match status {
            ffi::mfxStatus_MFX_ERR_MORE_DATA => Ok(None),
            ffi::mfxStatus_MFX_ERR_NONE => unsafe {
                check_error(ffi::MFXVideoCORE_SyncOperation(
                    self.session.raw,
                    sync_point,
                    ffi::MFX_INFINITE,
                ))?;

                tracing::trace!("encode_frame took {:?}", start_instant.elapsed());

                let start = self.encoded_bitstream.DataOffset as usize;
                let end = start + self.encoded_bitstream.DataLength as usize;

                let s = &self.encoded_buffer[start..end];
                self.encoded_bitstream.DataLength = 0; // Reset to 0
                Ok(Some(s))
            },
            status => {
                check_error(status)?;
                unreachable!("check_error should have returned an error");
            }
        }
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        unsafe {
            ffi::MFXVideoENCODE_Close(self.session.raw);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigureMethod {
    Init,
    Reset,
}

#[cfg(test)]
mod test {
    use crate::builder::{Codec, H264Profile, RateControlMethod, RequiredFields};

    use super::*;

    #[test]
    #[ignore]
    fn encode() {
        let mut pipeline = Pipeline::new(
            EncoderConfig::new(RequiredFields {
                width: 1920,
                height: 1080,
                codec: Codec::H264 {
                    profile: Some(H264Profile::Baseline),
                },
                framerate: (60, 1),
                format: buffer::InputFormat::RGB4,
            })
            .with_rate_control(RateControlMethod::IntelligentConstantQuality { quality: 10 }),
        )
        .unwrap();

        std::thread::spawn(move || {
            for i in 0..30 {
                let start = std::time::Instant::now();
                let (buf_idx, buf) = pipeline.get_free_surface().unwrap();
                buf.data_mut().fill(128);
                let _data = pipeline.encode_frame(buf_idx, false).unwrap();
                println!("frame {} took {:?}", i, start.elapsed());
            }
        })
        .join()
        .unwrap();
    }
}
