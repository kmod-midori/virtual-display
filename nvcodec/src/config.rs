use nvcodec_sys as ffi;

use crate::guid::{Codec, Profile, TuningInfo, Preset, BufferFormat};

#[derive(Debug, Clone)]
pub enum RateControlMode {
    ConstantBitrate {
        average: u32,
    },
    VariableBitrate {
        average: u32,
        /// If `None`, NVENC will set it to an internally determined default value.
        ///
        /// It is recommended that the client specify both parameters for better control.
        max: Option<u32>,
    },
    ConstantQp {
        inter_p: u32,
        inter_b: u32,
        intra: u32,
    },
    TargetQuality {
        /// Target quality level in the range [1, 51]. 1 is the highest quality and 51 is the lowest quality.
        ///
        /// Leave this field as None to let the driver determine.
        quality: Option<u8>,
        /// Maximum bitrate in bits per second.
        max: Option<u32>,
    },
}

pub struct EncodeConfig {
    pub(crate) inner: Box<ffi::NV_ENC_CONFIG>,
}

impl EncodeConfig {
    pub fn new() -> Self {
        let mut encode_config: Box<ffi::NV_ENC_CONFIG> = Box::new(unsafe { std::mem::zeroed() });
        encode_config.version = crate::nvenc_api_struct_version(7) | (1 << 31);

        Self {
            inner: encode_config,
        }
    }

    pub fn profile(&self) -> Option<Profile> {
        Profile::from_guid(self.inner.profileGUID)
    }

    pub fn with_profile(mut self, profile: Profile) -> Self {
        self.inner.profileGUID = profile.into();
        self
    }

    pub fn with_rate_control_mode(mut self, mode: RateControlMode) -> Self {
        let inner = &mut self.inner.rcParams;
        inner.version = crate::nvenc_api_struct_version(1);

        match mode {
            RateControlMode::ConstantBitrate { average } => {
                inner.rateControlMode = ffi::_NV_ENC_PARAMS_RC_MODE_NV_ENC_PARAMS_RC_CBR;
                inner.averageBitRate = average;
            }
            RateControlMode::VariableBitrate { average, max } => {
                inner.rateControlMode = ffi::_NV_ENC_PARAMS_RC_MODE_NV_ENC_PARAMS_RC_VBR;
                inner.averageBitRate = average;
                if let Some(max) = max {
                    inner.maxBitRate = max;
                }
            }
            RateControlMode::ConstantQp {
                inter_p,
                inter_b,
                intra,
            } => {
                inner.rateControlMode = ffi::_NV_ENC_PARAMS_RC_MODE_NV_ENC_PARAMS_RC_CONSTQP;
                inner.constQP.qpInterB = inter_b;
                inner.constQP.qpInterP = inter_p;
                inner.constQP.qpIntra = intra;
            }
            RateControlMode::TargetQuality { quality, max } => {
                inner.rateControlMode = ffi::_NV_ENC_PARAMS_RC_MODE_NV_ENC_PARAMS_RC_VBR;
                if let Some(quality) = quality {
                    inner.targetQuality = quality;
                } else {
                    inner.targetQuality = 0;
                }
                if let Some(max) = max {
                    inner.maxBitRate = max;
                }
            }
        }

        self
    }
}

impl Default for EncodeConfig {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EncoderInitializeParams {
    pub(crate) inner: Box<ffi::NV_ENC_INITIALIZE_PARAMS>,
    pub(crate) encode_config: Option<EncodeConfig>,
    pub(crate) buffer_format: BufferFormat,
}

impl EncoderInitializeParams {
    pub fn new(codec: Codec, width: u32, height: u32, format: BufferFormat) -> Self {
        let mut inner: Box<ffi::NV_ENC_INITIALIZE_PARAMS> = Box::new(unsafe { std::mem::zeroed() });
        inner.version = crate::nvenc_api_struct_version(5) | (1 << 31);
        inner.encodeGUID = codec.into();
        inner.encodeWidth = width;
        inner.encodeHeight = height;
        // Only support display order for now
        inner.enablePTD = 1;

        Self {
            inner,
            encode_config: None,
            buffer_format: format,
        }
    }

    pub fn with_preset(mut self, preset: Preset) -> Self {
        self.inner.presetGUID = preset.into();
        self
    }

    pub fn with_frame_rate(mut self, numerator: u32, denominator: u32) -> Self {
        self.inner.frameRateNum = numerator;
        self.inner.frameRateDen = denominator;
        self
    }

    pub fn with_encode_config(mut self, encode_config: EncodeConfig) -> Self {
        self.encode_config = Some(encode_config);
        self
    }

    pub fn with_tuning_info(mut self, tuning_info: TuningInfo) -> Self {
        self.inner.tuningInfo = tuning_info.into();
        self
    }
}
