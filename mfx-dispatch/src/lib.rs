use std::{mem::MaybeUninit, ptr::null_mut};

pub use mfx_dispatch_sys as ffi;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "The function cannot find the desired legacy Intel(r) Media SDK implementation or version."
    )]
    Unsupported,
    #[error("Invalid video parameters")]
    InvalidVideoParam,
    #[error("Incompatible video parameters")]
    IncompatibleVideoParam,
    #[error("Unknown error: {0:x}")]
    Unknown(i32),
}

fn check_error(err: i32) -> Result<()> {
    match err {
        ffi::mfxStatus_MFX_ERR_NONE
        | ffi::mfxStatus_MFX_WRN_PARTIAL_ACCELERATION
        | ffi::mfxStatus_MFX_WRN_INCOMPATIBLE_VIDEO_PARAM => Ok(()),
        ffi::mfxStatus_MFX_ERR_UNSUPPORTED => Err(Error::Unsupported),
        ffi::mfxStatus_MFX_ERR_INVALID_VIDEO_PARAM => Err(Error::InvalidVideoParam),
        ffi::mfxStatus_MFX_ERR_INCOMPATIBLE_VIDEO_PARAM => Err(Error::IncompatibleVideoParam),
        err => Err(Error::Unknown(err)),
    }
}

fn align32(x: u16) -> u16 {
    ((x + 31) >> 5) << 5
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Session {
    raw: ffi::mfxSession,
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

pub struct Pipeline {
    session: Session,

    _width: u16,
    buffer_width: u16,
    _height: u16,
    buffer_height: u16,
    _framerate: u16,

    surfaces: Vec<(Vec<u8>, ffi::mfxFrameSurface1)>,

    encoded_buffer: Vec<u8>,
    encoded_bitstream: ffi::mfxBitstream,
}

impl Pipeline {
    pub fn new(session: Session, width: u16, height: u16, framerate: u16) -> Result<Self> {
        let mut enc_par: ffi::mfxVideoParam = unsafe { std::mem::zeroed() };
        enc_par.IOPattern = ffi::MFX_IOPATTERN_IN_SYSTEM_MEMORY as u16
            | ffi::MFX_IOPATTERN_OUT_SYSTEM_MEMORY as u16;

        enc_par.AsyncDepth = 1;

        enc_par
            .__bindgen_anon_1
            .mfx
            .__bindgen_anon_1
            .__bindgen_anon_1
            .GopRefDist = 1;
        enc_par
            .__bindgen_anon_1
            .mfx
            .__bindgen_anon_1
            .__bindgen_anon_1
            .IdrInterval = 1;

        enc_par.__bindgen_anon_1.mfx.CodecId = ffi::MFX_CODEC_AVC as u32;
        // enc_par.__bindgen_anon_1.mfx.CodecProfile = ffi::MFX_PROFILE_AVC_BASELINE as u16;
        enc_par
            .__bindgen_anon_1
            .mfx
            .__bindgen_anon_1
            .__bindgen_anon_1
            .TargetUsage = ffi::MFX_TARGETUSAGE_BEST_SPEED as u16;
        enc_par
            .__bindgen_anon_1
            .mfx
            .__bindgen_anon_1
            .__bindgen_anon_1
            .RateControlMethod = ffi::MFX_RATECONTROL_ICQ as u16;
        // enc_par
        //     .__bindgen_anon_1
        //     .mfx
        //     .__bindgen_anon_1
        //     .__bindgen_anon_1
        //     .__bindgen_anon_2
        //     .TargetKbps = 12000;
        enc_par
            .__bindgen_anon_1
            .mfx
            .__bindgen_anon_1
            .__bindgen_anon_1
            .__bindgen_anon_2
            .ICQQuality = 27;

        enc_par.__bindgen_anon_1.mfx.FrameInfo.FrameRateExtN = framerate as u32;
        enc_par.__bindgen_anon_1.mfx.FrameInfo.FrameRateExtD = 1;
        enc_par.__bindgen_anon_1.mfx.FrameInfo.FourCC = ffi::MFX_FOURCC_NV12 as u32;
        enc_par.__bindgen_anon_1.mfx.FrameInfo.ChromaFormat = ffi::MFX_CHROMAFORMAT_YUV420 as u16;
        enc_par.__bindgen_anon_1.mfx.FrameInfo.PicStruct = ffi::MFX_PICSTRUCT_PROGRESSIVE as u16;

        let buffer_width = align32(width);
        let buffer_height = align32(height);

        enc_par
            .__bindgen_anon_1
            .mfx
            .FrameInfo
            .__bindgen_anon_1
            .__bindgen_anon_1
            .CropW = width;
        enc_par
            .__bindgen_anon_1
            .mfx
            .FrameInfo
            .__bindgen_anon_1
            .__bindgen_anon_1
            .CropH = height;

        enc_par
            .__bindgen_anon_1
            .mfx
            .FrameInfo
            .__bindgen_anon_1
            .__bindgen_anon_1
            .Width = buffer_width;
        enc_par
            .__bindgen_anon_1
            .mfx
            .FrameInfo
            .__bindgen_anon_1
            .__bindgen_anon_1
            .Height = buffer_height;

        let mut coding_option_2: ffi::mfxExtCodingOption2 = unsafe { std::mem::zeroed() };
        coding_option_2.Header = ffi::mfxExtBuffer {
            BufferId: ffi::MFX_EXTBUFF_CODING_OPTION2 as u32,
            BufferSz: std::mem::size_of::<ffi::mfxExtCodingOption2>() as u32,
        };
        coding_option_2.BRefType = ffi::MFX_B_REF_OFF as u16; // Disable B-frame

        let mut coding_option_3: ffi::mfxExtCodingOption3 = unsafe { std::mem::zeroed() };
        coding_option_3.Header = ffi::mfxExtBuffer {
            BufferId: ffi::MFX_EXTBUFF_CODING_OPTION3 as u32,
            BufferSz: std::mem::size_of::<ffi::mfxExtCodingOption3>() as u32,
        };
        coding_option_3.ScenarioInfo = ffi::MFX_SCENARIO_DISPLAY_REMOTING as u16;
        coding_option_3.ContentInfo = ffi::MFX_CONTENT_NON_VIDEO_SCREEN as u16;

        let ext_params =
            &mut [&mut coding_option_2 as *mut ffi::mfxExtCodingOption2 as *mut ffi::mfxExtBuffer];

        enc_par.ExtParam = ext_params.as_mut_ptr();
        enc_par.NumExtParam = ext_params.len() as u16;

        unsafe {
            check_error(ffi::MFXVideoENCODE_Query(
                session.raw,
                &mut enc_par,
                &mut enc_par,
            ))?;
        }

        let mut alloc_request = MaybeUninit::uninit();
        unsafe {
            check_error(ffi::MFXVideoENCODE_QueryIOSurf(
                session.raw,
                &mut enc_par,
                alloc_request.as_mut_ptr(),
            ))?;
        }
        let alloc_request = unsafe { alloc_request.assume_init() };

        let buffer_width =
            unsafe { alloc_request.Info.__bindgen_anon_1.__bindgen_anon_1.Width } as usize;
        let buffer_height =
            unsafe { alloc_request.Info.__bindgen_anon_1.__bindgen_anon_1.Height } as usize;

        let buffer_size = buffer_width * buffer_height * 3 / 2; // NV12

        let mut surfaces = vec![];
        for _ in 0..alloc_request.NumFrameSuggested {
            let mut buffer = vec![0u8; buffer_size];
            let buffer_y_ptr = buffer.as_mut_ptr();
            let buffer_uv_ptr = unsafe { buffer_y_ptr.add(buffer_width * buffer_height) };

            let mut surface: ffi::mfxFrameSurface1 = unsafe { std::mem::zeroed() };
            surface.Info = alloc_request.Info;
            surface.Data.__bindgen_anon_3.Y = buffer_y_ptr;
            surface.Data.__bindgen_anon_4.UV = buffer_uv_ptr;
            surface.Data.__bindgen_anon_2.PitchLow = buffer_width as u16;

            surfaces.push((buffer, surface));
        }

        unsafe {
            check_error(ffi::MFXVideoENCODE_Init(session.raw, &mut enc_par))?;
        }

        let mut active_enc_par: ffi::mfxVideoParam = unsafe { std::mem::zeroed() };
        unsafe {
            check_error(ffi::MFXVideoENCODE_GetVideoParam(
                session.raw,
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

        Ok(Self {
            session,
            _width: width,
            _height: height,
            buffer_width: buffer_width as u16,
            buffer_height: buffer_height as u16,
            _framerate: framerate,

            surfaces,

            encoded_buffer,
            encoded_bitstream,
        })
    }

    /// Get a free surface to encode a frame into.
    ///
    /// Returns the index of the surface and a mutable reference to the buffer of Y and UV planes.
    pub fn get_free_surface(&mut self) -> Option<(usize, &mut [u8], &mut [u8])> {
        for (i, (buffer, surface)) in self.surfaces.iter_mut().enumerate() {
            if surface.Data.Locked == 0 {
                let uv_offset = self.buffer_width as usize * self.buffer_height as usize;
                let (buffer_y, buffer_uv) = buffer.split_at_mut(uv_offset);
                return Some((i, buffer_y, buffer_uv));
            }
        }

        None
    }

    pub fn stride(&self) -> usize {
        self.buffer_width as usize
    }

    pub fn encode_frame(
        &mut self,
        surface_index: usize,
        force_keyframe: bool,
    ) -> Result<Option<&[u8]>> {
        let (_, surface) = &mut self.surfaces[surface_index];

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

    pub fn close(self) -> Session {
        Session {
            raw: self.session.raw,
            impl_: self.session.impl_,
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
