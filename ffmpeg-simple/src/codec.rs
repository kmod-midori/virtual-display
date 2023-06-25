use std::borrow::Cow;

pub use ffmpeg_sys as ffi;

#[derive(Copy, Clone)]
pub struct Codec {
    pub(crate) raw: *const ffi::AVCodec,
}

impl Codec {
    pub fn find_by_name(name: &str) -> Option<Self> {
        let name = std::ffi::CString::new(name).unwrap();

        let raw = unsafe { ffi::avcodec_find_encoder_by_name(name.as_ptr()) };

        if raw.is_null() {
            None
        } else {
            Some(Codec { raw })
        }
    }

    pub fn pixel_formats(&self) -> PixelFormats {
        PixelFormats {
            raw: self.raw,
            index: 0,
        }
    }

    /// Retrieve supported hardware configurations for a codec.
    pub fn hw_configs(&self) -> HwConfigs {
        HwConfigs {
            raw: self.raw,
            index: 0,
        }
    }

    pub fn name(&self) -> &'static str {
        unsafe { std::ffi::CStr::from_ptr((*self.raw).name).to_str().unwrap() }
    }

    pub fn long_name(&self) -> &'static str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.raw).long_name)
                .to_str()
                .unwrap()
        }
    }
}

pub struct PixelFormats {
    raw: *const ffi::AVCodec,
    index: usize,
}

impl Iterator for PixelFormats {
    type Item = ffi::AVPixelFormat;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let item = *(*self.raw).pix_fmts.add(self.index);

            if item == ffi::AVPixelFormat_AV_PIX_FMT_NONE {
                None
            } else {
                self.index += 1;
                Some(item)
            }
        }
    }
}

bitflags::bitflags! {
    /// Possible setup methods which can be used with a configuration.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct HwCodecSetupMethod: u32 {
        /// The codec supports this format via the `hw_device_ctx` interface.
        ///
        /// When selecting this format, `AVCodecContext.hw_device_ctx` should
        /// have been set to a device of the specified type before calling
        /// `avcodec_open2()`.
        const HwDeviceCtx = ffi::AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX as _;
        const HwFramesCtx = ffi::AV_CODEC_HW_CONFIG_METHOD_HW_FRAMES_CTX as _;
        /// The codec supports this format by some internal method.
        ///
        /// This format can be selected without any additional configuration - no device or frames context is required.
        const Internal = ffi::AV_CODEC_HW_CONFIG_METHOD_INTERNAL as _;
        /// The codec supports this format by some ad-hoc method.
        ///
        /// Additional settings and/or function calls are required.
        const AdHoc = ffi::AV_CODEC_HW_CONFIG_METHOD_AD_HOC as _;
    }
}

pub struct HwConfigs {
    raw: *const ffi::AVCodec,
    index: usize,
}

impl Iterator for HwConfigs {
    type Item = HwConfig;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let item = ffi::avcodec_get_hw_config(self.raw, self.index as _);

            if item.is_null() {
                None
            } else {
                self.index += 1;
                Some(HwConfig {
                    methods: HwCodecSetupMethod::from_bits_truncate((*item).methods as u32),
                    device_type: (*item).device_type,
                })
            }
        }
    }
}

#[derive(Debug)]
pub struct HwConfig {
    pub methods: HwCodecSetupMethod,
    pub device_type: ffi::AVHWDeviceType,
}

impl HwConfig {
    pub fn type_name(&self) -> Cow<'static, str> {
        unsafe {
            let s = ffi::av_hwdevice_get_type_name(self.device_type);
            let cs = std::ffi::CStr::from_ptr(s);
            cs.to_string_lossy()
        }
    }
}
