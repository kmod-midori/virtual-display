use crate::Result;
use std::{ffi::OsString, os::windows::prelude::OsStringExt, ptr::null_mut};
use windows::{
    core::{Interface, GUID},
    Win32::{
        Foundation::LUID,
        Graphics::Dxgi::{CreateDXGIFactory, IDXGIFactory},
        Media::MediaFoundation::{self, IMFAttributes, MFTEnum2, MFT_ENUM_ADAPTER_LUID},
        System::Com::CoTaskMemFree,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum VideoFormat {
    H264,
    HEVC,
}

impl VideoFormat {
    fn guid(&self) -> GUID {
        match self {
            VideoFormat::H264 => MediaFoundation::MFVideoFormat_H264,
            VideoFormat::HEVC => MediaFoundation::MFVideoFormat_HEVC,
        }
    }
}

#[derive(Debug)]
pub struct Adapter {
    /// A handle to the adapter.
    pub luid: LUID,
    /// Name of the adapter.
    pub name: OsString,
    /// PCI vendor ID of the adapter.
    pub vendoer_id: u32,
    /// PCI device ID of the adapter.
    pub device_id: u32,
}

impl Adapter {
    pub fn encoders(&self, format: VideoFormat) -> Result<()> {
        let output_type = MediaFoundation::MFT_REGISTER_TYPE_INFO {
            guidMajorType: MediaFoundation::MFMediaType_Video,
            guidSubtype: format.guid(),
        };

        let flags = MediaFoundation::MFT_ENUM_FLAG_HARDWARE
            | MediaFoundation::MFT_ENUM_FLAG_ASYNCMFT
            | MediaFoundation::MFT_ENUM_FLAG_SORTANDFILTER;

        let mut ppmftactivate = null_mut();
        let mut nummftactivate = 0;

        let transform = unsafe {
            let mut attrs = None;
            MediaFoundation::MFCreateAttributes(&mut attrs, 1)?;
            let attrs = attrs.unwrap();
            attrs.SetUINT64(&MFT_ENUM_ADAPTER_LUID, self.luid.LowPart as u64)?;

            MFTEnum2(
                MediaFoundation::MFT_CATEGORY_VIDEO_ENCODER,
                flags,
                None,
                Some(&output_type),
                Some(&attrs),
                &mut ppmftactivate,
                &mut nummftactivate,
            )?;

            let activates = std::slice::from_raw_parts_mut(ppmftactivate, nummftactivate as usize);

            let mut transform = None;

            for activate in activates {
                let activate = activate.take().unwrap();

                if transform.is_none() {
                    let attrs = activate.cast::<IMFAttributes>()?;
                    let clsid = attrs.GetGUID(&MediaFoundation::MFT_TRANSFORM_CLSID_Attribute)?;
                    dbg!(clsid);
                    // tracing::info!(?clsid, "Activating MFT");

                    match activate.ActivateObject::<MediaFoundation::IMFTransform>() {
                        Ok(a) => transform = Some(a),
                        Err(e) => {
                            dbg!(e);
                            // tracing::error!(?clsid, ?e, "Failed to activate MFT");
                            continue;
                        }
                    }
                }
            }

            CoTaskMemFree(Some(ppmftactivate as *const _));

            transform
        };

        dbg!(transform);

        Ok(())
    }
}

/// Use `IDXGIFactory.EnumAdapters` to enumerate adapters and filter out the NVIDIA ones
/// based on PCI vendor ID.
///
/// Be sure to drop the returned handles when you are done with them.
pub fn enumerate_supported_adapters() -> crate::Result<Vec<Adapter>> {
    let mut adapters = vec![];
    unsafe {
        let factory: IDXGIFactory = CreateDXGIFactory()?;
        let mut adapter_index = 0;
        while let Ok(adapter) = factory.EnumAdapters(adapter_index) {
            let mut desc = std::mem::zeroed();
            adapter.GetDesc(&mut desc).unwrap();

            let name_len = desc
                .Description
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(desc.Description.len());
            let name = OsString::from_wide(&desc.Description[..name_len]);

            adapters.push(Adapter {
                luid: desc.AdapterLuid,
                name,
                vendoer_id: desc.VendorId,
                device_id: desc.DeviceId,
            });

            adapter_index += 1;
        }
    }

    Ok(adapters)
}

#[cfg(test)]
mod test {
    use windows::Win32::{
        Media::MediaFoundation::{MFStartup, MFSTARTUP_FULL},
        System::Com::CoInitialize,
    };

    use super::*;

    fn init_mft() {
        unsafe {
            CoInitialize(None).unwrap();
            MFStartup(
                windows::Win32::Media::MediaFoundation::MF_SDK_VERSION << 16
                    | windows::Win32::Media::MediaFoundation::MF_API_VERSION,
                MFSTARTUP_FULL,
            )
            .unwrap();
        }
    }

    #[test]
    #[ignore]
    fn test_enumerate_supported_adapters() {
        init_mft();

        let adapters = enumerate_supported_adapters().unwrap();
        for adapter in adapters {
            adapter.encoders(VideoFormat::HEVC).unwrap();
        }
    }
}
