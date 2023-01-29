use std::ptr::null_mut;

use anyhow::Result;

use variant::Variant;
use windows::{
    core::{IUnknown, Interface, Vtable},
    Win32::{
        Graphics::{
            Direct3D, Direct3D11,
            Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC},
        },
        Media::MediaFoundation::{self, MFTEnumEx},
        System::Com::{CoInitialize, CoTaskMemFree},
    },
};

mod variant;

fn main() -> Result<()> {
    unsafe {
        CoInitialize(None)?;
    }

    // let driver_types = &[
    //     Direct3D::D3D_DRIVER_TYPE_HARDWARE,
    //     Direct3D::D3D_DRIVER_TYPE_WARP,
    //     Direct3D::D3D_DRIVER_TYPE_REFERENCE,
    // ];

    // let feature_levels = &[
    //     Direct3D::D3D_FEATURE_LEVEL_11_0,
    //     Direct3D::D3D_FEATURE_LEVEL_10_1,
    //     Direct3D::D3D_FEATURE_LEVEL_10_0,
    //     Direct3D::D3D_FEATURE_LEVEL_9_1,
    // ];

    // let mut device = None;
    // let mut context = None;

    // for driver_type in driver_types {
    //     let flags =
    //         Direct3D11::D3D11_CREATE_DEVICE_DEBUG | Direct3D11::D3D11_CREATE_DEVICE_VIDEO_SUPPORT;

    //     let result = unsafe {
    //         Direct3D11::D3D11CreateDevice(
    //             None,
    //             *driver_type,
    //             None,
    //             flags,
    //             Some(&feature_levels[..]),
    //             Direct3D11::D3D11_SDK_VERSION,
    //             Some(&mut device),
    //             None,
    //             Some(&mut context),
    //         )
    //     };

    //     if result.is_ok() && device.is_some() && context.is_some() {
    //         break;
    //     }
    // }

    // let (device, context) = match (device, context) {
    //     (Some(device), Some(context)) => (device, context),
    //     _ => return Err(anyhow::anyhow!("Failed to create D3D11 device")),
    // };

    // let mut reset_token = 0;
    // let mut device_manager = None;
    // unsafe {
    //     MediaFoundation::MFCreateDXGIDeviceManager(&mut reset_token, &mut device_manager)?;
    // }
    // let device_manager = device_manager.unwrap();
    // unsafe {
    //     device_manager.ResetDevice(&device, reset_token)?;
    // }

    let transform = unsafe {
        let output_type = MediaFoundation::MFT_REGISTER_TYPE_INFO {
            guidMajorType: MediaFoundation::MFMediaType_Video,
            guidSubtype: MediaFoundation::MFVideoFormat_H264,
        };

        let flags = MediaFoundation::MFT_ENUM_FLAG_HARDWARE
            | MediaFoundation::MFT_ENUM_FLAG_SORTANDFILTER;

        let mut ppmftactivate = null_mut();
        let mut nummftactivate = 0;

        MFTEnumEx(
            MediaFoundation::MFT_CATEGORY_VIDEO_ENCODER,
            flags,
            None,
            Some(&output_type),
            &mut ppmftactivate,
            &mut nummftactivate,
        )?;

        let activates = std::slice::from_raw_parts_mut(ppmftactivate, nummftactivate as usize);

        let transform = if let Some(activate) = activates.first() {
            activate
                .as_ref()
                .unwrap()
                .ActivateObject::<MediaFoundation::IMFTransform>()?
        } else {
            return Err(anyhow::anyhow!("No hardware encoder found"));
        };

        for activate in activates {
            activate.take();
        }

        CoTaskMemFree(Some(ppmftactivate as *const _));

        transform
    };

    let encoder_attrs = unsafe { transform.GetAttributes()? };

    unsafe {
        // Unlock so that this MFT can be used asynchronously.
        encoder_attrs.SetUINT32(&MediaFoundation::MF_TRANSFORM_ASYNC_UNLOCK, 1)?;
        // Enable low-latency
        encoder_attrs.SetUINT32(&MediaFoundation::MF_LOW_LATENCY, 1)?;
    }

    let (input_stream_id, output_stream_id) = unsafe {
        let input_id = &mut [0];
        let output_id = &mut [0];
        transform.GetStreamIDs(input_id, output_id)?;
        (input_id[0], output_id[0])
    };

    let bitrate_bps = 12_000_000;

    let size_u64 = (1920 as u64) << 32 | (1080 as u64);
    let framerate_u64 = (60 as u64) << 32 | 1;

    unsafe {
        let codec_api = transform.cast::<MediaFoundation::ICodecAPI>()?;
        let val = Variant::from(MediaFoundation::eAVEncCommonRateControlMode_Quality.0 as u32);
        codec_api.SetValue(
            &MediaFoundation::CODECAPI_AVEncCommonRateControlMode,
            val.as_ptr(),
        )?;
        let val = Variant::from(70u32);
        codec_api.SetValue(&MediaFoundation::CODECAPI_AVEncCommonQuality, val.as_ptr())?;
        let val = Variant::from(true);
        codec_api.SetValue(&MediaFoundation::CODECAPI_AVLowLatencyMode, val.as_ptr())?;
        let val = Variant::from(0u32);
        codec_api.SetValue(
            &MediaFoundation::CODECAPI_AVEncCommonQualityVsSpeed,
            val.as_ptr(),
        )?;
    }

    unsafe {
        let output_type = MediaFoundation::MFCreateMediaType()?;
        output_type.SetGUID(
            &MediaFoundation::MF_MT_MAJOR_TYPE,
            &MediaFoundation::MFMediaType_Video,
        )?;
        output_type.SetGUID(
            &MediaFoundation::MF_MT_SUBTYPE,
            &MediaFoundation::MFVideoFormat_H264,
        )?;
        output_type.SetUINT32(&MediaFoundation::MF_MT_AVG_BITRATE, bitrate_bps)?;
        output_type.SetUINT64(&MediaFoundation::MF_MT_FRAME_SIZE, size_u64)?;
        output_type.SetUINT64(&MediaFoundation::MF_MT_FRAME_RATE, framerate_u64)?;
        output_type.SetUINT32(
            &MediaFoundation::MF_MT_INTERLACE_MODE,
            MediaFoundation::MFVideoInterlace_Progressive.0 as u32,
        )?;
        // Set this attribute to TRUE for all uncompressed media types.
        output_type.SetUINT32(&MediaFoundation::MF_MT_ALL_SAMPLES_INDEPENDENT, 1)?;
        output_type.SetUINT32(
            &MediaFoundation::MF_MT_MPEG2_PROFILE,
            MediaFoundation::eAVEncH264VProfile_High.0 as u32,
        )?; // eAVEncH264VProfile_High
        transform.SetOutputType(output_stream_id, &output_type, 0)?;
    }

    unsafe {
        let input_type = transform.GetInputAvailableType(input_stream_id, 0)?;
        input_type.SetGUID(
            &MediaFoundation::MF_MT_MAJOR_TYPE,
            &MediaFoundation::MFMediaType_Video,
        )?;
        input_type.SetGUID(
            &MediaFoundation::MF_MT_SUBTYPE,
            &MediaFoundation::MFVideoFormat_NV12,
        )?;
        input_type.SetUINT64(&MediaFoundation::MF_MT_FRAME_SIZE, size_u64)?;
        input_type.SetUINT64(&MediaFoundation::MF_MT_FRAME_RATE, framerate_u64)?;
        // Set this attribute to TRUE for all uncompressed media types.
        input_type.SetUINT32(&MediaFoundation::MF_MT_ALL_SAMPLES_INDEPENDENT, 1)?;
        transform.SetInputType(input_stream_id, &input_type, 0)?;
    }
    Ok(())
}
