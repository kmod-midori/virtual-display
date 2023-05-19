use std::{ffi::c_void, ptr::null_mut, sync::Arc};

use buffers::{InputBuffer, LockedInputBuffer, OutputBuffer};
use config::EncoderInitializeParams;
use guid::{BufferFormat, Codec, Preset, Profile};
use nvcodec_sys as ffi;
use windows::{
    core::{IUnknown, Interface},
    Win32::Graphics::{Direct3D, Direct3D11},
};

use crate::{config::EncodeConfig, guid::TuningInfo};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to load library: {0}")]
    LoadLibrary(#[from] libloading::Error),
    #[error("Windows API error: {0}")]
    Windows(#[from] windows::core::Error),
    #[error("Device creation failed")]
    DeviceCreationFailed,
    #[error("NVENC error {0}: {1}")]
    NvCodec(ffi::NVENCSTATUS, String),
    #[error("Invalid parameter")]
    InvalidParam,
    #[error("Unsupported parameter")]
    UnsupportedParam,
    #[error("Not enough buffer, please allocate more input/output buffers")]
    NotEnoughBuffer,
    #[error("Unknown error: {0}")]
    Unknown(ffi::NVENCSTATUS),
}

pub type Result<T> = std::result::Result<T, Error>;

pub mod adapter;
pub mod buffers;
pub mod config;
pub mod guid;

/// The `NVENCAPI_STRUCT_VERSION` macro.
pub(crate) fn nvenc_api_struct_version(ver: u32) -> u32 {
    ffi::NVENCAPI_VERSION | (ver << 16) | (0x7 << 28)
}

pub(crate) fn check_error(status: ffi::NVENCSTATUS) -> Result<()> {
    if status == ffi::_NVENCSTATUS_NV_ENC_SUCCESS {
        return Ok(());
    }

    match status {
        ffi::_NVENCSTATUS_NV_ENC_SUCCESS => Ok(()),
        ffi::_NVENCSTATUS_NV_ENC_ERR_INVALID_PARAM => Err(Error::InvalidParam),
        ffi::_NVENCSTATUS_NV_ENC_ERR_UNSUPPORTED_PARAM => Err(Error::UnsupportedParam),
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
        fnlist.version = nvenc_api_struct_version(1);
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
            params.version = nvenc_api_struct_version(1);
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
            ptr: Some(encoder_ptr),
        })
    }
}

pub struct Encoder {
    library: Library,
    /// `None` denotes that the encoder has been initialized and moved
    /// to another struct.
    ptr: Option<*mut c_void>,
}

impl Drop for Encoder {
    fn drop(&mut self) {
        if let Some(ptr) = self.ptr {
            unsafe {
                self.library.0.fnlist.nvEncDestroyEncoder.unwrap()(ptr);
            }
        }
    }
}

impl Encoder {
    fn ptr(&self) -> *mut c_void {
        self.ptr.unwrap()
    }

    pub fn codecs(&self) -> Result<Vec<Codec>> {
        let mut count = 0;
        unsafe {
            check_error(self.library.0.fnlist.nvEncGetEncodeGUIDCount.unwrap()(
                self.ptr(),
                &mut count,
            ))?;
        }

        let mut guids = vec![unsafe { std::mem::zeroed() }; count as usize];
        unsafe {
            check_error(self.library.0.fnlist.nvEncGetEncodeGUIDs.unwrap()(
                self.ptr(),
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
                self.ptr(),
                codec.into(),
                &mut count,
            ))?;
        }

        let mut guids = vec![unsafe { std::mem::zeroed() }; count as usize];
        unsafe {
            check_error(self.library.0.fnlist.nvEncGetEncodePresetGUIDs.unwrap()(
                self.ptr(),
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
                .unwrap()(
                self.ptr(), codec.into(), &mut count
            ))?;
        }

        let mut guids = vec![unsafe { std::mem::zeroed() }; count as usize];
        unsafe {
            check_error(self.library.0.fnlist.nvEncGetEncodeProfileGUIDs.unwrap()(
                self.ptr(),
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
                self.ptr(),
                codec.into(),
                &mut count,
            ))?;
        }

        let mut formats = vec![unsafe { std::mem::zeroed() }; count as usize];
        unsafe {
            check_error(self.library.0.fnlist.nvEncGetInputFormats.unwrap()(
                self.ptr(),
                codec.into(),
                formats.as_mut_ptr(),
                count,
                &mut count,
            ))?;
        }

        Ok(formats
            .into_iter()
            .filter_map(BufferFormat::from_ffi)
            .collect())
    }

    pub fn preset_config(
        &self,
        codec: Codec,
        preset: Preset,
        tuning_info: TuningInfo,
    ) -> Result<EncodeConfig> {
        let mut config: ffi::NV_ENC_PRESET_CONFIG = unsafe { std::mem::zeroed() };
        config.version = nvenc_api_struct_version(4) | (1 << 31);
        config.presetCfg.version = nvenc_api_struct_version(7) | (1 << 31);

        unsafe {
            check_error(self.library.0.fnlist.nvEncGetEncodePresetConfigEx.unwrap()(
                self.ptr(),
                codec.into(),
                preset.into(),
                tuning_info.into(),
                &mut config,
            ))?;
        }

        Ok(EncodeConfig {
            inner: Box::new(config.presetCfg),
        })
    }

    pub fn configure(mut self, mut params: EncoderInitializeParams) -> Result<InitializedEncoder> {
        if let Some(c) = params.encode_config.as_mut() {
            params.inner.encodeConfig = c.inner.as_mut();
        }

        let width = params.inner.encodeWidth;
        let height = params.inner.encodeHeight;

        unsafe {
            check_error(self.library.0.fnlist.nvEncInitializeEncoder.unwrap()(
                self.ptr(),
                params.inner.as_mut(),
            ))?;
        }

        let input_buffer = InputBuffer::new(
            self.library.clone(),
            self.ptr(),
            width,
            height,
            params.buffer_format,
        )?;

        let mut output_buffers = vec![];
        let mut cpu_output_buffers = vec![];
        for _ in 0..4 {
            output_buffers.push(OutputBuffer::new(self.library.clone(), self.ptr())?);
            cpu_output_buffers.push(vec![]);
        }

        Ok(InitializedEncoder {
            library: self.library.clone(),
            ptr: self.ptr.take().unwrap(),

            input_buffer: Some(input_buffer),
            output_buffers,
            submitted_output_buffers: vec![],
            cpu_output_buffers,

            width,
            height,
        })
    }
}

pub struct InitializedEncoder {
    library: Library,
    ptr: *mut c_void,

    input_buffer: Option<InputBuffer>,
    output_buffers: Vec<OutputBuffer>,
    submitted_output_buffers: Vec<OutputBuffer>,
    cpu_output_buffers: Vec<Vec<u8>>,

    width: u32,
    height: u32,
}

impl InitializedEncoder {
    pub fn spspps(&self) -> Result<Vec<u8>> {
        let mut buffer = [0u8; ffi::NV_MAX_SEQ_HDR_LEN as usize];
        let mut out_size = 0;

        let mut arg: ffi::NV_ENC_SEQUENCE_PARAM_PAYLOAD = unsafe { std::mem::zeroed() };
        arg.version = nvenc_api_struct_version(1);
        arg.inBufferSize = buffer.len() as u32;
        arg.spsppsBuffer = buffer.as_mut_ptr() as *mut c_void;
        arg.outSPSPPSPayloadSize = &mut out_size;

        unsafe {
            check_error(self.library.0.fnlist.nvEncGetSequenceParams.unwrap()(
                self.ptr, &mut arg,
            ))?;
        }

        Ok(buffer[..out_size as usize].to_vec())
    }

    /// Locks an input buffer for writing
    pub fn lock_input_buffer(&mut self) -> Result<LockedInputBuffer<'_>> {
        self.input_buffer.as_mut().unwrap().lock()
    }

    pub fn encode(&mut self, pts: u64, force_idr: bool) -> Result<Vec<(u64, &[u8])>> {
        let input_buffer = self.input_buffer.as_mut().unwrap();

        let mut args: ffi::NV_ENC_PIC_PARAMS = unsafe { std::mem::zeroed() };
        args.version = nvenc_api_struct_version(4) | (1 << 31);
        args.inputBuffer = input_buffer.buffer;
        args.inputWidth = self.width;
        args.inputHeight = self.height;
        args.inputPitch = self.width;
        args.bufferFmt = input_buffer.format().into();
        args.inputTimeStamp = pts;
        args.pictureStruct = ffi::_NV_ENC_PIC_STRUCT_NV_ENC_PIC_STRUCT_FRAME;
        if force_idr {
            args.encodePicFlags |= ffi::_NV_ENC_PIC_FLAGS_NV_ENC_PIC_FLAG_FORCEIDR as u32;
        }

        let output_buffer = self.output_buffers.pop().ok_or(Error::NotEnoughBuffer)?;
        args.outputBitstream = output_buffer.buffer;
        self.submitted_output_buffers.push(output_buffer);

        let result =
            unsafe { self.library.0.fnlist.nvEncEncodePicture.unwrap()(self.ptr, &mut args) };

        match result {
            ffi::_NVENCSTATUS_NV_ENC_SUCCESS => {
                let mut out_buffers = vec![];

                let it = self
                    .submitted_output_buffers
                    .drain(..)
                    .zip(self.cpu_output_buffers.iter_mut());

                for (mut gpu_buffer, cpu_buffer) in it {
                    let locked = gpu_buffer.lock()?;
                    cpu_buffer.clear();
                    cpu_buffer.extend_from_slice(locked.data());

                    out_buffers.push((locked.pts(), cpu_buffer.as_slice()));

                    drop(locked);

                    self.output_buffers.push(gpu_buffer);
                }

                Ok(out_buffers)
            }
            ffi::_NVENCSTATUS_NV_ENC_ERR_NEED_MORE_INPUT => {
                // This frame has been buffered internally
                Ok(vec![])
            }
            result => {
                check_error(result)?;
                unreachable!();
            }
        }
    }
}

impl Drop for InitializedEncoder {
    fn drop(&mut self) {
        // Destroy all buffers before destroying the encoder
        self.input_buffer.take();
        self.submitted_output_buffers.clear();
        self.output_buffers.clear();

        unsafe {
            check_error(self.library.0.fnlist.nvEncDestroyEncoder.unwrap()(self.ptr)).unwrap();
        }
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

    let encode_config = encoder
        .preset_config(Codec::H264, Preset::P4, TuningInfo::LowLatency)
        .unwrap()
        .with_rate_control_mode(config::RateControlMode::TargetQuality {
            quality: Some(10),
            max: None,
        });

    let config = EncoderInitializeParams::new(Codec::H264, 1920, 1080, BufferFormat::ARGB)
        .with_frame_rate(60, 1)
        .with_preset(Preset::P4)
        .with_tuning_info(TuningInfo::LowLatency)
        .with_encode_config(encode_config);
    let mut encoder = encoder.configure(config).unwrap();

    for _ in 0..30 {
        let mut b = encoder.lock_input_buffer().unwrap();
        b.data().fill(128);
        drop(b);
        dbg!(encoder.encode(0, false).unwrap());
    }
}
