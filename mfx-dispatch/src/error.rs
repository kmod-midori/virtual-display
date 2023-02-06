use mfx_dispatch_sys as ffi;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "The function cannot find the desired legacy Intel(r) Media SDK implementation or version."
    )]
    Unsupported,
    #[error("Invalid video parameters")]
    InvalidVideoParam,
    #[error("Incompatible video parameters")]
    IncompatibleVideoParam,
    #[error("Unknown error: {0}")]
    Unknown(i32),
}

pub fn check_error(err: i32) -> Result<()> {
    match err {
        ffi::mfxStatus_MFX_ERR_NONE
        | ffi::mfxStatus_MFX_WRN_PARTIAL_ACCELERATION
        | ffi::mfxStatus_MFX_WRN_INCOMPATIBLE_VIDEO_PARAM => Ok(()),
        ffi::mfxStatus_MFX_ERR_UNSUPPORTED => Err(Error::Unsupported),
        ffi::mfxStatus_MFX_ERR_INVALID_VIDEO_PARAM => Err(Error::InvalidVideoParam),
        ffi::mfxStatus_MFX_ERR_INCOMPATIBLE_VIDEO_PARAM => Err(Error::IncompatibleVideoParam),
        err => Err(Error::Unknown(err)),
    }
}

pub type Result<T> = std::result::Result<T, Error>;