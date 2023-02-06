use nvcodec_sys as ffi;

// static const GUID\s+(NV_ENC_.*_GUID)\s+=\n{(.*?),(.*?),(.*?),\s?{(.*?)}\s};
// const $1: ffi::GUID = ffi::GUID {Data1:$2, Data2: $3, Data3: $4, Data4: [$5]};

const NV_ENC_CODEC_H264_GUID: ffi::GUID = ffi::GUID {
    Data1: 0x6bc82762,
    Data2: 0x4e63,
    Data3: 0x4ca4,
    Data4: [0xaa, 0x85, 0x1e, 0x50, 0xf3, 0x21, 0xf6, 0xbf],
};

const NV_ENC_CODEC_HEVC_GUID: ffi::GUID = ffi::GUID {
    Data1: 0x790cdc88,
    Data2: 0x4522,
    Data3: 0x4d7b,
    Data4: [0x94, 0x25, 0xbd, 0xa9, 0x97, 0x5f, 0x76, 0x3],
};

// {BFD6F8E7-233C-4341-8B3E-4818523803F4}
const NV_ENC_CODEC_PROFILE_AUTOSELECT_GUID: ffi::GUID = ffi::GUID {
    Data1: 0xbfd6f8e7,
    Data2: 0x233c,
    Data3: 0x4341,
    Data4: [0x8b, 0x3e, 0x48, 0x18, 0x52, 0x38, 0x3, 0xf4],
};

// {0727BCAA-78C4-4c83-8C2F-EF3DFF267C6A}
const NV_ENC_H264_PROFILE_BASELINE_GUID: ffi::GUID = ffi::GUID {
    Data1: 0x727bcaa,
    Data2: 0x78c4,
    Data3: 0x4c83,
    Data4: [0x8c, 0x2f, 0xef, 0x3d, 0xff, 0x26, 0x7c, 0x6a],
};

// {60B5C1D4-67FE-4790-94D5-C4726D7B6E6D}
const NV_ENC_H264_PROFILE_MAIN_GUID: ffi::GUID = ffi::GUID {
    Data1: 0x60b5c1d4,
    Data2: 0x67fe,
    Data3: 0x4790,
    Data4: [0x94, 0xd5, 0xc4, 0x72, 0x6d, 0x7b, 0x6e, 0x6d],
};

// {E7CBC309-4F7A-4b89-AF2A-D537C92BE310}
const NV_ENC_H264_PROFILE_HIGH_GUID: ffi::GUID = ffi::GUID {
    Data1: 0xe7cbc309,
    Data2: 0x4f7a,
    Data3: 0x4b89,
    Data4: [0xaf, 0x2a, 0xd5, 0x37, 0xc9, 0x2b, 0xe3, 0x10],
};

// {7AC663CB-A598-4960-B844-339B261A7D52}
const NV_ENC_H264_PROFILE_HIGH_444_GUID: ffi::GUID = ffi::GUID {
    Data1: 0x7ac663cb,
    Data2: 0xa598,
    Data3: 0x4960,
    Data4: [0xb8, 0x44, 0x33, 0x9b, 0x26, 0x1a, 0x7d, 0x52],
};

// {40847BF5-33F7-4601-9084-E8FE3C1DB8B7}
const NV_ENC_H264_PROFILE_STEREO_GUID: ffi::GUID = ffi::GUID {
    Data1: 0x40847bf5,
    Data2: 0x33f7,
    Data3: 0x4601,
    Data4: [0x90, 0x84, 0xe8, 0xfe, 0x3c, 0x1d, 0xb8, 0xb7],
};

// {B405AFAC-F32B-417B-89C4-9ABEED3E5978}
const NV_ENC_H264_PROFILE_PROGRESSIVE_HIGH_GUID: ffi::GUID = ffi::GUID {
    Data1: 0xb405afac,
    Data2: 0xf32b,
    Data3: 0x417b,
    Data4: [0x89, 0xc4, 0x9a, 0xbe, 0xed, 0x3e, 0x59, 0x78],
};

// {AEC1BD87-E85B-48f2-84C3-98BCA6285072}
const NV_ENC_H264_PROFILE_CONSTRAINED_HIGH_GUID: ffi::GUID = ffi::GUID {
    Data1: 0xaec1bd87,
    Data2: 0xe85b,
    Data3: 0x48f2,
    Data4: [0x84, 0xc3, 0x98, 0xbc, 0xa6, 0x28, 0x50, 0x72],
};

// {B514C39A-B55B-40fa-878F-F1253B4DFDEC}
const NV_ENC_HEVC_PROFILE_MAIN_GUID: ffi::GUID = ffi::GUID {
    Data1: 0xb514c39a,
    Data2: 0xb55b,
    Data3: 0x40fa,
    Data4: [0x87, 0x8f, 0xf1, 0x25, 0x3b, 0x4d, 0xfd, 0xec],
};

// {fa4d2b6c-3a5b-411a-8018-0a3f5e3c9be5}
const NV_ENC_HEVC_PROFILE_MAIN10_GUID: ffi::GUID = ffi::GUID {
    Data1: 0xfa4d2b6c,
    Data2: 0x3a5b,
    Data3: 0x411a,
    Data4: [0x80, 0x18, 0x0a, 0x3f, 0x5e, 0x3c, 0x9b, 0xe5],
};

// For HEVC Main 444 8 bit and HEVC Main 444 10 bit profiles only
// {51ec32b5-1b4c-453c-9cbd-b616bd621341}
const NV_ENC_HEVC_PROFILE_FREXT_GUID: ffi::GUID = ffi::GUID {
    Data1: 0x51ec32b5,
    Data2: 0x1b4c,
    Data3: 0x453c,
    Data4: [0x9c, 0xbd, 0xb6, 0x16, 0xbd, 0x62, 0x13, 0x41],
};

// {FC0A8D3E-45F8-4CF8-80C7-298871590EBF}
const NV_ENC_PRESET_P1_GUID: ffi::GUID = ffi::GUID {
    Data1: 0xfc0a8d3e,
    Data2: 0x45f8,
    Data3: 0x4cf8,
    Data4: [0x80, 0xc7, 0x29, 0x88, 0x71, 0x59, 0xe, 0xbf],
};

// {F581CFB8-88D6-4381-93F0-DF13F9C27DAB}
const NV_ENC_PRESET_P2_GUID: ffi::GUID = ffi::GUID {
    Data1: 0xf581cfb8,
    Data2: 0x88d6,
    Data3: 0x4381,
    Data4: [0x93, 0xf0, 0xdf, 0x13, 0xf9, 0xc2, 0x7d, 0xab],
};

// {36850110-3A07-441F-94D5-3670631F91F6}
const NV_ENC_PRESET_P3_GUID: ffi::GUID = ffi::GUID {
    Data1: 0x36850110,
    Data2: 0x3a07,
    Data3: 0x441f,
    Data4: [0x94, 0xd5, 0x36, 0x70, 0x63, 0x1f, 0x91, 0xf6],
};

// {90A7B826-DF06-4862-B9D2-CD6D73A08681}
const NV_ENC_PRESET_P4_GUID: ffi::GUID = ffi::GUID {
    Data1: 0x90a7b826,
    Data2: 0xdf06,
    Data3: 0x4862,
    Data4: [0xb9, 0xd2, 0xcd, 0x6d, 0x73, 0xa0, 0x86, 0x81],
};

// {21C6E6B4-297A-4CBA-998F-B6CBDE72ADE3}
const NV_ENC_PRESET_P5_GUID: ffi::GUID = ffi::GUID {
    Data1: 0x21c6e6b4,
    Data2: 0x297a,
    Data3: 0x4cba,
    Data4: [0x99, 0x8f, 0xb6, 0xcb, 0xde, 0x72, 0xad, 0xe3],
};

// {8E75C279-6299-4AB6-8302-0B215A335CF5}
const NV_ENC_PRESET_P6_GUID: ffi::GUID = ffi::GUID {
    Data1: 0x8e75c279,
    Data2: 0x6299,
    Data3: 0x4ab6,
    Data4: [0x83, 0x2, 0xb, 0x21, 0x5a, 0x33, 0x5c, 0xf5],
};

// {84848C12-6F71-4C13-931B-53E283F57974}
const NV_ENC_PRESET_P7_GUID: ffi::GUID = ffi::GUID {
    Data1: 0x84848c12,
    Data2: 0x6f71,
    Data3: 0x4c13,
    Data4: [0x93, 0x1b, 0x53, 0xe2, 0x83, 0xf5, 0x79, 0x74],
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    H264,
    HEVC,
}

impl Codec {
    pub(crate) fn from_guid(guid: ffi::GUID) -> Option<Self> {
        match guid {
            NV_ENC_CODEC_H264_GUID => Some(Codec::H264),
            NV_ENC_CODEC_HEVC_GUID => Some(Codec::HEVC),
            _ => None,
        }
    }
}

impl From<Codec> for ffi::GUID {
    fn from(val: Codec) -> Self {
        match val {
            Codec::H264 => NV_ENC_CODEC_H264_GUID,
            Codec::HEVC => NV_ENC_CODEC_HEVC_GUID,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Auto,
    H264Baseline,
    H264Main,
    H264High,
    H264High444,
    H264Stereo,
    H264ProgressiveHigh,
    H264ConstrainedHigh,

    HEVCMain,
    HEVCMain10,
    /// For HEVC Main 444 8 bit and HEVC Main 444 10 bit profiles only
    HEVCFRExt,
}

impl Profile {
    pub(crate) fn from_guid(guid: ffi::GUID) -> Option<Self> {
        match guid {
            NV_ENC_H264_PROFILE_BASELINE_GUID => Some(Profile::H264Baseline),
            NV_ENC_H264_PROFILE_MAIN_GUID => Some(Profile::H264Main),
            NV_ENC_H264_PROFILE_HIGH_GUID => Some(Profile::H264High),
            NV_ENC_H264_PROFILE_HIGH_444_GUID => Some(Profile::H264High444),
            NV_ENC_H264_PROFILE_STEREO_GUID => Some(Profile::H264Stereo),
            NV_ENC_H264_PROFILE_PROGRESSIVE_HIGH_GUID => Some(Profile::H264ProgressiveHigh),
            NV_ENC_H264_PROFILE_CONSTRAINED_HIGH_GUID => Some(Profile::H264ConstrainedHigh),

            NV_ENC_HEVC_PROFILE_MAIN_GUID => Some(Profile::HEVCMain),
            NV_ENC_HEVC_PROFILE_FREXT_GUID => Some(Profile::HEVCFRExt),
            NV_ENC_HEVC_PROFILE_MAIN10_GUID => Some(Profile::HEVCMain10),

            _ => None,
        }
    }
}

impl From<Profile> for ffi::GUID {
    fn from(val: Profile) -> Self {
        match val {
            Profile::H264Baseline => NV_ENC_H264_PROFILE_BASELINE_GUID,
            Profile::H264Main => NV_ENC_H264_PROFILE_MAIN_GUID,
            Profile::H264High => NV_ENC_H264_PROFILE_HIGH_GUID,
            Profile::H264High444 => NV_ENC_H264_PROFILE_HIGH_444_GUID,
            Profile::H264Stereo => NV_ENC_H264_PROFILE_STEREO_GUID,
            Profile::H264ProgressiveHigh => NV_ENC_H264_PROFILE_PROGRESSIVE_HIGH_GUID,
            Profile::H264ConstrainedHigh => NV_ENC_H264_PROFILE_CONSTRAINED_HIGH_GUID,
            Profile::HEVCMain => NV_ENC_HEVC_PROFILE_MAIN_GUID,
            Profile::HEVCMain10 => NV_ENC_HEVC_PROFILE_MAIN10_GUID,
            Profile::HEVCFRExt => NV_ENC_HEVC_PROFILE_FREXT_GUID,
            Profile::Auto => NV_ENC_CODEC_PROFILE_AUTOSELECT_GUID,
        }
    }
}

/// Performance degrades and quality improves as we move from P1 to P7.
///
/// Presets P3 to P7 for H264 and Presets P2 to P7 for HEVC have B frames enabled by default
/// for HIGH_QUALITY and LOSSLESS tuning info, and will not work with Weighted Prediction enabled.
///
/// In case Weighted Prediction is required, disable B frames by setting frameIntervalP = 1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
}

impl Preset {
    pub(crate) fn from_guid(guid: ffi::GUID) -> Option<Self> {
        match guid {
            NV_ENC_PRESET_P1_GUID => Some(Preset::P1),
            NV_ENC_PRESET_P2_GUID => Some(Preset::P2),
            NV_ENC_PRESET_P3_GUID => Some(Preset::P3),
            NV_ENC_PRESET_P4_GUID => Some(Preset::P4),
            NV_ENC_PRESET_P5_GUID => Some(Preset::P5),
            NV_ENC_PRESET_P6_GUID => Some(Preset::P6),
            NV_ENC_PRESET_P7_GUID => Some(Preset::P7),
            _ => None,
        }
    }
}

impl From<Preset> for ffi::GUID {
    fn from(val: Preset) -> Self {
        match val {
            Preset::P1 => NV_ENC_PRESET_P1_GUID,
            Preset::P2 => NV_ENC_PRESET_P2_GUID,
            Preset::P3 => NV_ENC_PRESET_P3_GUID,
            Preset::P4 => NV_ENC_PRESET_P4_GUID,
            Preset::P5 => NV_ENC_PRESET_P5_GUID,
            Preset::P6 => NV_ENC_PRESET_P6_GUID,
            Preset::P7 => NV_ENC_PRESET_P7_GUID,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuningInfo {
    HighQuality,
    LowLatency,
    UltraLowLatency,
    Lossless,
}

// impl TuningInfo {
//     pub(crate) fn from_ffi(val: ffi::NV_ENC_TUNING_INFO) -> Option<Self> {
//         match val {
//             ffi::NV_ENC_TUNING_INFO_NV_ENC_TUNING_INFO_HIGH_QUALITY => {
//                 Some(TuningInfo::HighQuality)
//             }
//             ffi::NV_ENC_TUNING_INFO_NV_ENC_TUNING_INFO_LOW_LATENCY => Some(TuningInfo::LowLatency),
//             ffi::NV_ENC_TUNING_INFO_NV_ENC_TUNING_INFO_ULTRA_LOW_LATENCY => {
//                 Some(TuningInfo::UltraLowLatency)
//             }
//             ffi::NV_ENC_TUNING_INFO_NV_ENC_TUNING_INFO_LOSSLESS => Some(TuningInfo::Lossless),
//             _ => None,
//         }
//     }
// }

impl From<TuningInfo> for ffi::NV_ENC_TUNING_INFO {
    fn from(val: TuningInfo) -> Self {
        match val {
            TuningInfo::HighQuality => ffi::NV_ENC_TUNING_INFO_NV_ENC_TUNING_INFO_HIGH_QUALITY,
            TuningInfo::LowLatency => ffi::NV_ENC_TUNING_INFO_NV_ENC_TUNING_INFO_LOW_LATENCY,
            TuningInfo::UltraLowLatency => {
                ffi::NV_ENC_TUNING_INFO_NV_ENC_TUNING_INFO_ULTRA_LOW_LATENCY
            }
            TuningInfo::Lossless => ffi::NV_ENC_TUNING_INFO_NV_ENC_TUNING_INFO_LOSSLESS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferFormat {
    // Semi-Planar YUV [Y plane followed by interleaved UV plane]
    NV12,
    // Planar YUV [Y plane followed by U and V planes]
    YV12,
    // Planar YUV [Y plane followed by V and U planes]
    IYUV,
    // Planar YUV [Y plane followed by U and V planes]
    YUV444,
    /// 10 bit Semi-Planar YUV [Y plane followed by interleaved UV plane].
    ///
    /// Each pixel of size 2 bytes. Most Significant 10 bits contain pixel data.
    YUV420P10,
    /// 10 bit Planar YUV444 [Y plane followed by U and V planes].
    ///
    /// Each pixel of size 2 bytes. Most Significant 10 bits contain pixel data.
    YUV444P10,
    /// 8 bit Packed A8R8G8B8. This is a word-ordered format
    /// where a pixel is represented by a 32-bit word with B
    /// in the lowest 8 bits, G in the next 8 bits, R in the
    /// 8 bits after that and A in the highest 8 bits.
    ARGB,
    /// 10 bit Packed A2R10G10B10. This is a word-ordered format
    /// where a pixel is represented by a 32-bit word with B
    /// in the lowest 10 bits, G in the next 10 bits, R in the
    /// 10 bits after that and A in the highest 2 bits.
    ARGB10,
    /// 8 bit Packed A8Y8U8V8. This is a word-ordered format
    /// where a pixel is represented by a 32-bit word with V
    /// in the lowest 8 bits, U in the next 8 bits, Y in the
    /// 8 bits after that and A in the highest 8 bits.
    AYUV,
    /// 8 bit Packed A8B8G8R8. This is a word-ordered format
    /// where a pixel is represented by a 32-bit word with R
    /// in the lowest 8 bits, G in the next 8 bits, B in the
    /// 8 bits after that and A in the highest 8 bits.
    ABGR,
    /// 10 bit Packed A2B10G10R10. This is a word-ordered format
    /// where a pixel is represented by a 32-bit word with R
    /// in the lowest 10 bits, G in the next 10 bits, B in the
    /// 10 bits after that and A in the highest 2 bits.
    ABGR10,
    /// Buffer format representing one-dimensional buffer.
    /// This format should be used only when registering the
    /// resource as output buffer, which will be used to write
    /// the encoded bit stream or H.264 ME only mode output.
    U8,
}

impl BufferFormat {
    pub(crate) fn from_ffi(format: ffi::NV_ENC_BUFFER_FORMAT) -> Option<Self> {
        match format {
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_NV12 => Some(BufferFormat::NV12),
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_YV12 => Some(BufferFormat::YV12),
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_IYUV => Some(BufferFormat::IYUV),
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_YUV444 => Some(BufferFormat::YUV444),
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_YUV420_10BIT => {
                Some(BufferFormat::YUV420P10)
            }
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_YUV444_10BIT => {
                Some(BufferFormat::YUV444P10)
            }
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_ARGB => Some(BufferFormat::ARGB),
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_ARGB10 => Some(BufferFormat::ARGB10),
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_AYUV => Some(BufferFormat::AYUV),
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_ABGR => Some(BufferFormat::ABGR),
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_ABGR10 => Some(BufferFormat::ABGR10),
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_U8 => Some(BufferFormat::U8),
            _ => None,
        }
    }
}

impl From<BufferFormat> for ffi::NV_ENC_BUFFER_FORMAT {
    fn from(format: BufferFormat) -> Self {
        match format {
            BufferFormat::NV12 => ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_NV12,
            BufferFormat::YV12 => ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_YV12,
            BufferFormat::IYUV => ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_IYUV,
            BufferFormat::YUV444 => ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_YUV444,
            BufferFormat::YUV420P10 => ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_YUV420_10BIT,
            BufferFormat::YUV444P10 => ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_YUV444_10BIT,
            BufferFormat::ARGB => ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_ARGB,
            BufferFormat::ARGB10 => ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_ARGB10,
            BufferFormat::AYUV => ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_AYUV,
            BufferFormat::ABGR => ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_ABGR,
            BufferFormat::ABGR10 => ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_ABGR10,
            BufferFormat::U8 => ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_U8,
        }
    }
}
