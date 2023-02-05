use std::{ffi::OsString, os::windows::prelude::OsStringExt};

use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory, IDXGIAdapter, IDXGIFactory};

#[derive(Debug)]
pub struct Adapter {
    /// A handle to the adapter.
    pub adapter: IDXGIAdapter,
    /// Name of the adapter.
    pub name: OsString,
    /// PCI device ID of the adapter.
    pub device_id: u32,
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

            if desc.VendorId == 0x10DE {
                // NVIDIA
                let name_len = desc
                    .Description
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(desc.Description.len());
                let name = OsString::from_wide(&desc.Description[..name_len]);

                adapters.push(Adapter {
                    adapter,
                    name,
                    device_id: desc.DeviceId,
                });
            }

            adapter_index += 1;
        }
    }

    Ok(adapters)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[ignore]
    fn test_enumerate_supported_adapters() {
        let adapters = enumerate_supported_adapters().unwrap();
        dbg!(adapters);
    }
}
