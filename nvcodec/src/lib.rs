use std::{ffi::c_void, ptr::null_mut, sync::Arc};

use guid::{Codec, Preset, Profile};
use nvcodec_sys as ffi;
use windows::{
    core::{IUnknown, Interface},
    Win32::Graphics::{Direct3D, Direct3D11},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to load library: {0}")]
    LoadLibrary(#[from] libloading::Error),
    #[error("Windows API error: {0}")]
    Windows(#[from] windows::core::Error),
    #[error("Device creation failed")]
    DeviceCreationFailed,
    #[error("Unknown error: {0}")]
    Unknown(ffi::NVENCSTATUS),
}

pub type Result<T> = std::result::Result<T, Error>;

pub mod adapter;
pub mod guid;
pub mod config;

/// The `NVENCAPI_STRUCT_VERSION` macro.
pub(crate) fn nvenv_api_struct_version(ver: u32) -> u32 {
    ffi::NVENCAPI_VERSION | (ver << 16) | (0x7 << 28)
}

pub(crate) fn check_error(status: ffi::NVENCSTATUS) -> Result<()> {
    match status {
        ffi::_NVENCSTATUS_NV_ENC_SUCCESS => Ok(()),
        _ => Err(Error::Unknown(status)),
    }
}

struct LibraryInner {
    _lib: libloading::Library,
    fnlist: ffi::NV_ENCODE_API_FUNCTION_LIST,
}

/// A handle to the loaded library.
///
/// You are f
#[derive(Clone)]
pub struct Library(Arc<LibraryInner>);

impl Library {
    pub fn new() -> Result<Self> {
        #[cfg(all(windows, target_pointer_width = "64"))]
        let library_name = "nvEncodeAPI64.dll";
        #[cfg(all(windows, target_pointer_width = "32"))]
        let library_name = "nvEncodeAPI.dll";
        #[cfg(target_os = "linux")]
        let library_name = "libnvidia-encode.so.1";

        let library = unsafe { libloading::Library::new(library_name)? };
        let create_instance_fn = unsafe {
            library.get::<unsafe extern "C" fn(
                *mut ffi::NV_ENCODE_API_FUNCTION_LIST,
            ) -> ffi::NVENCSTATUS>(b"NvEncodeAPICreateInstance")?
        };

        let mut fnlist: ffi::NV_ENCODE_API_FUNCTION_LIST = unsafe { std::mem::zeroed() };
        fnlist.version = nvenv_api_struct_version(1);
        unsafe {
            check_error(create_instance_fn(&mut fnlist))?;
        }

        let inner = LibraryInner {
            _lib: library,
            fnlist,
        };

        Ok(Self(Arc::new(inner)))
    }

    /// Creates a DirectX-based encoder.
    pub fn encoder_directx(&self, adapter: adapter::Adapter) -> Result<Encoder> {
        let feature_levels = &[
            Direct3D::D3D_FEATURE_LEVEL_11_0,
            Direct3D::D3D_FEATURE_LEVEL_10_1,
            Direct3D::D3D_FEATURE_LEVEL_10_0,
            Direct3D::D3D_FEATURE_LEVEL_9_1,
        ];

        let mut device = None;

        unsafe {
            Direct3D11::D3D11CreateDevice(
                &adapter.adapter,
                Direct3D::D3D_DRIVER_TYPE_UNKNOWN,
                None,
                Direct3D11::D3D11_CREATE_DEVICE_FLAG(0),
                Some(&feature_levels[..]),
                Direct3D11::D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )?
        };

        let device = if let Some(device) = device {
            device
        } else {
            return Err(Error::DeviceCreationFailed);
        };

        let encoder_ptr = unsafe {
            let device = device.cast::<IUnknown>()?;
            let mut params: ffi::NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS = std::mem::zeroed();
            params.version = nvenv_api_struct_version(1);
            params.deviceType = ffi::_NV_ENC_DEVICE_TYPE_NV_ENC_DEVICE_TYPE_DIRECTX;
            params.device = std::mem::transmute(device);
            params.apiVersion = ffi::NVENCAPI_VERSION;

            let mut encoder = null_mut();

            match check_error(self.0.fnlist.nvEncOpenEncodeSessionEx.unwrap()(
                &mut params,
                &mut encoder,
            )) {
                Ok(_) => Ok(encoder),
                Err(e) => {
                    // If the creation of encoder session fails, the client must call ::NvEncDestroyEncoder API
                    // before exiting.
                    self.0.fnlist.nvEncDestroyEncoder.unwrap()(encoder);
                    Err(e)
                }
            }
        }?;

        Ok(Encoder {
            library: self.clone(),
            ptr: encoder_ptr,
        })
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
    fn from_ffi(format: ffi::NV_ENC_BUFFER_FORMAT) -> Option<Self> {
        match format {
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_NV12 => Some(BufferFormat::NV12),
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_YV12 => Some(BufferFormat::YV12),
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_IYUV => Some(BufferFormat::IYUV),
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_YUV444 => Some(BufferFormat::YUV444),
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_YUV420_10BIT => Some(BufferFormat::YUV420P10),
            ffi::_NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_YUV444_10BIT => Some(BufferFormat::YUV444P10),
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

pub struct Encoder {
    library: Library,
    ptr: *mut c_void,
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            self.library.0.fnlist.nvEncDestroyEncoder.unwrap()(self.ptr);
        }
    }
}

impl Encoder {
    pub fn codecs(&self) -> Result<Vec<Codec>> {
        let mut count = 0;
        unsafe {
            check_error(self.library.0.fnlist.nvEncGetEncodeGUIDCount.unwrap()(
                self.ptr, &mut count,
            ))?;
        }

        let mut guids = vec![unsafe { std::mem::zeroed() }; count as usize];
        unsafe {
            check_error(self.library.0.fnlist.nvEncGetEncodeGUIDs.unwrap()(
                self.ptr,
                guids.as_mut_ptr(),
                count,
                &mut count,
            ))?;
        }

        Ok(guids.into_iter().filter_map(Codec::from_guid).collect())
    }

    pub fn presets(&self, codec: Codec) -> Result<Vec<Preset>> {
        let mut count = 0;
        unsafe {
            check_error(self.library.0.fnlist.nvEncGetEncodePresetCount.unwrap()(
                self.ptr,
                codec.into(),
                &mut count,
            ))?;
        }

        let mut guids = vec![unsafe { std::mem::zeroed() }; count as usize];
        unsafe {
            check_error(self.library.0.fnlist.nvEncGetEncodePresetGUIDs.unwrap()(
                self.ptr,
                codec.into(),
                guids.as_mut_ptr(),
                count,
                &mut count,
            ))?;
        }

        Ok(guids.into_iter().filter_map(Preset::from_guid).collect())
    }

    pub fn profiles(&self, codec: Codec) -> Result<Vec<Profile>> {
        let mut count = 0;
        unsafe {
            check_error(self
                .library
                .0
                .fnlist
                .nvEncGetEncodeProfileGUIDCount
                .unwrap()(self.ptr, codec.into(), &mut count))?;
        }

        let mut guids = vec![unsafe { std::mem::zeroed() }; count as usize];
        unsafe {
            check_error(self.library.0.fnlist.nvEncGetEncodeProfileGUIDs.unwrap()(
                self.ptr,
                codec.into(),
                guids.as_mut_ptr(),
                count,
                &mut count,
            ))?;
        }

        Ok(guids.into_iter().filter_map(Profile::from_guid).collect())
    }

    pub fn input_formats(&self, codec: Codec) -> Result<Vec<BufferFormat>> {
        let mut count = 0;
        unsafe {
            check_error(self.library.0.fnlist.nvEncGetInputFormatCount.unwrap()(
                self.ptr,
                codec.into(),
                &mut count,
            ))?;
        }

        let mut formats = vec![unsafe { std::mem::zeroed() }; count as usize];
        unsafe {
            check_error(self.library.0.fnlist.nvEncGetInputFormats.unwrap()(
                self.ptr,
                codec.into(),
                formats.as_mut_ptr(),
                count,
                &mut count,
            ))?;
        }

        Ok(formats.into_iter().filter_map(BufferFormat::from_ffi).collect())
    }
}

#[test]
fn encoder() {
    let library = Library::new().unwrap();
    let adapter = adapter::enumerate_supported_adapters().unwrap().remove(0);
    let encoder = library.encoder_directx(adapter).unwrap();
    dbg!(encoder.codecs().unwrap());
    dbg!(encoder.presets(Codec::H264).unwrap());
    dbg!(encoder.profiles(Codec::H264).unwrap());
    dbg!(encoder.input_formats(Codec::H264).unwrap());
}
