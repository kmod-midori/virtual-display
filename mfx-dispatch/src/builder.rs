pub use mfx_dispatch_sys as ffi;

use crate::align32;

pub struct EncoderConfig {
    pub(crate) inner: ffi::mfxVideoParam,
    pub(crate) opt2: ffi::mfxExtCodingOption2,
    pub(crate) opt3: ffi::mfxExtCodingOption3,

    // pub(crate) format: InputFormat,
}

impl EncoderConfig {
    pub fn new(req: RequiredFields) -> Self {
        let mut inner: ffi::mfxVideoParam = unsafe { std::mem::zeroed() };
        req.fill_config(&mut inner);

        let opt2 = ffi::mfxExtCodingOption2 {
            Header: ffi::mfxExtBuffer {
                BufferId: ffi::MFX_EXTBUFF_CODING_OPTION2 as _,
                BufferSz: std::mem::size_of::<ffi::mfxExtCodingOption2>() as _,
            },
            BRefType: ffi::MFX_B_REF_OFF as _,
            ..unsafe { std::mem::zeroed() }
        };

        let opt3 = ffi::mfxExtCodingOption3 {
            Header: ffi::mfxExtBuffer {
                BufferId: ffi::MFX_EXTBUFF_CODING_OPTION3 as _,
                BufferSz: std::mem::size_of::<ffi::mfxExtCodingOption3>() as _,
            },
            ScenarioInfo: ffi::MFX_SCENARIO_DISPLAY_REMOTING as _,
            ContentInfo: ffi::MFX_CONTENT_NON_VIDEO_SCREEN as _,
            ..unsafe { std::mem::zeroed() }
        };

        Self {
            inner,
            opt2,
            opt3,

            // format: req.format,
        }
    }

    pub fn with_rate_control(mut self, rate_control: RateControlMethod) -> Self {
        rate_control.fill_config(&mut self.inner);
        self
    }
}

/// Fields that are required to build an encoder.
#[derive(Debug, Clone)]
pub struct RequiredFields {
    /// Width of the input frames in pixels.
    pub width: u16,
    /// Height of the input frames in pixels.
    pub height: u16,
    /// The codec to use.
    pub codec: Codec,
    /// Specified as `(numerator, denominator)`.
    pub framerate: (u16, u16),
    // /// The pixel format of the input frames.
    // pub format: InputFormat,
}

impl RequiredFields {
    pub(crate) fn fill_config(&self, config: &mut ffi::mfxVideoParam) {
        let buffer_width = align32(self.width);
        let buffer_height = align32(self.height);

        config
            .__bindgen_anon_1
            .mfx
            .FrameInfo
            .__bindgen_anon_1
            .__bindgen_anon_1
            .CropW = self.width as _;
        config
            .__bindgen_anon_1
            .mfx
            .FrameInfo
            .__bindgen_anon_1
            .__bindgen_anon_1
            .CropH = self.height as _;
        config
            .__bindgen_anon_1
            .mfx
            .FrameInfo
            .__bindgen_anon_1
            .__bindgen_anon_1
            .Width = buffer_width;
        config
            .__bindgen_anon_1
            .mfx
            .FrameInfo
            .__bindgen_anon_1
            .__bindgen_anon_1
            .Height = buffer_height;

        self.codec.fill_config(config);

        config.__bindgen_anon_1.mfx.FrameInfo.FrameRateExtN = self.framerate.0 as _;
        config.__bindgen_anon_1.mfx.FrameInfo.FrameRateExtD = self.framerate.1 as _;

        // self.format.fill_config(config);
        // Always output YUV420.
        config.__bindgen_anon_1.mfx.FrameInfo.ChromaFormat = ffi::MFX_CHROMAFORMAT_YUV420 as u16;
        // Always output progressive frames.
        config.__bindgen_anon_1.mfx.FrameInfo.PicStruct = ffi::MFX_PICSTRUCT_PROGRESSIVE as u16;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum H264Profile {
    Baseline,
    Main,
    Extended,
    High,
    High10,
    High422,
    ConstrainedBaseline,
    ConstrainedHigh,
}

impl From<H264Profile> for ffi::mfxU16 {
    fn from(value: H264Profile) -> Self {
        (match value {
            H264Profile::Baseline => ffi::MFX_PROFILE_AVC_BASELINE,
            H264Profile::Main => ffi::MFX_PROFILE_AVC_MAIN,
            H264Profile::Extended => ffi::MFX_PROFILE_AVC_EXTENDED,
            H264Profile::High => ffi::MFX_PROFILE_AVC_HIGH,
            H264Profile::High10 => ffi::MFX_PROFILE_AVC_HIGH10,
            H264Profile::High422 => ffi::MFX_PROFILE_AVC_HIGH_422,
            H264Profile::ConstrainedBaseline => ffi::MFX_PROFILE_AVC_CONSTRAINED_BASELINE,
            H264Profile::ConstrainedHigh => ffi::MFX_PROFILE_AVC_CONSTRAINED_HIGH,
        } as Self)
    }
}

#[derive(Debug, Clone)]
pub enum Codec {
    H264 {
        /// Specify the codec profile explicitly or the API functions
        /// will determine the correct profile from other sources, such as resolution and bitrate.
        profile: Option<H264Profile>,
    },
}

impl Codec {
    pub(crate) fn fill_config(&self, config: &mut ffi::mfxVideoParam) {
        match self {
            Codec::H264 { profile } => {
                config.__bindgen_anon_1.mfx.CodecId = ffi::MFX_CODEC_AVC as _;
                if let Some(profile) = profile {
                    config.__bindgen_anon_1.mfx.CodecProfile = (*profile).into();
                }
            }
        }
    }
}

/// https://spec.oneapi.io/versions/latest/elements/oneVPL/source/API_ref/VPL_enums.html#ratecontrolmethod
#[derive(Debug, Clone)]
pub enum RateControlMethod {
    /// The encoder attempts to maintain a constant bitrate.
    ConstantBitrate {
        /// Target bitrate in bits per second.
        target_bitrate: u32,
    },
    /// This algorithm improves subjective video quality of encoded stream.
    /// Depending on content, it may or may not decrease objective video quality.
    IntelligentConstantQuality {
        /// Values are in the 1 to 51 range, where 1 corresponds the best quality.
        quality: u8,
    },
}

impl RateControlMethod {
    pub(crate) fn fill_config(&self, config: &mut ffi::mfxVideoParam) {
        match self {
            RateControlMethod::ConstantBitrate { target_bitrate } => {
                config
                    .__bindgen_anon_1
                    .mfx
                    .__bindgen_anon_1
                    .__bindgen_anon_1
                    .RateControlMethod = ffi::MFX_RATECONTROL_CBR as _;
                config
                    .__bindgen_anon_1
                    .mfx
                    .__bindgen_anon_1
                    .__bindgen_anon_1
                    .__bindgen_anon_2
                    .TargetKbps = (*target_bitrate * 1000) as _;
            }
            RateControlMethod::IntelligentConstantQuality { quality } => {
                config
                    .__bindgen_anon_1
                    .mfx
                    .__bindgen_anon_1
                    .__bindgen_anon_1
                    .RateControlMethod = ffi::MFX_RATECONTROL_ICQ as _;
                config
                    .__bindgen_anon_1
                    .mfx
                    .__bindgen_anon_1
                    .__bindgen_anon_1
                    .__bindgen_anon_2
                    .ICQQuality = *quality as u16;
            }
        }
    }
}

