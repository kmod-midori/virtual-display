use std::ffi::c_void;

use anyhow::Result;
use windows::{
    core::{HRESULT, PCWSTR},
    Win32::{
        Devices::Enumeration::Pnp::{
            SWDeviceCapabilitiesDriverRequired, SWDeviceCapabilitiesRemovable,
            SWDeviceCapabilitiesSilentInstall, SwDeviceCreate, HSWDEVICE, SW_DEVICE_CREATE_INFO,
        },
        Foundation::HANDLE,
        System::IO::DeviceIoControl,
    },
};

unsafe extern "system" fn sw_device_create_callback(
    hswdevice: HSWDEVICE,
    createresult: HRESULT,
    pcontext: *const c_void,
    pszdeviceinstanceid: PCWSTR,
) {
    let callback_ref: &mut &mut dyn FnMut() = &mut *(pcontext as *mut _);
    callback_ref();
}

fn main() -> Result<()> {
    println!("Hello, world!");

    let create_info = SW_DEVICE_CREATE_INFO {
        cbSize: std::mem::size_of::<SW_DEVICE_CREATE_INFO>() as _,
        pszInstanceId: windows::w!("ChizukoIddDriver"),
        pszzHardwareIds: windows::w!("ChizukoIddDriver\0\0"),
        pszzCompatibleIds: windows::w!("ChizukoIddDriver\0\0"),
        pszDeviceDescription: windows::w!("Virtual Display Idd Driver"),
        CapabilityFlags: (SWDeviceCapabilitiesRemovable.0
            | SWDeviceCapabilitiesSilentInstall.0
            | SWDeviceCapabilitiesDriverRequired.0) as u32,
        ..Default::default()
    };

    let device_handle = {
        let (tx, rx) = std::sync::mpsc::channel::<()>();

        let mut callback = move || {
            let _ = tx.send(());
        };
        let mut callback_obj: &mut dyn FnMut() = &mut callback;

        let device_handle = unsafe {
            SwDeviceCreate(
                windows::w!("ChizukoIddDriver"),
                windows::w!("HTREE\\ROOT\\0"),
                &create_info,
                None,
                Some(sw_device_create_callback),
                Some(&mut callback_obj as *mut _ as _),
            )?
        };

        println!("Device handle: {:?}", device_handle);
        rx.recv()?;
        println!("Device created");
        device_handle
    };

    // std::thread::sleep(std::time::Duration::from_secs(5));

    // unsafe {
    //     DeviceIoControl(HANDLE(device_handle), 233, None, 0, None, 0, None, None);
    // }

    loop {
        std::thread::sleep(std::time::Duration::from_secs(20));
    }
}
