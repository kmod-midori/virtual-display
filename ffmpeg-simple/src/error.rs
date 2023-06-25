use ffmpeg_sys as ffi;
use std::ffi::c_int;

pub type Result<T> = std::result::Result<T, FfmpegError>;

#[derive(Debug)]
pub enum FfmpegError {
    AvError(c_int),
    Other(String),
}

impl std::fmt::Display for FfmpegError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FfmpegError::AvError(code) => {
                let mut buf = vec![0; ffi::AV_ERROR_MAX_STRING_SIZE as usize];

                unsafe {
                    ffi::av_strerror(*code, buf.as_mut_ptr(), buf.len());
                    let err_msg = std::ffi::CStr::from_ptr(buf.as_ptr()).to_string_lossy();
                    write!(f, "FFmpeg error: {}", err_msg)
                }
            }
            FfmpegError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for FfmpegError {}

pub(crate) fn check_error(result: c_int) -> Result<c_int> {
    if result < 0 {
        Err(FfmpegError::AvError(result))
    } else {
        Ok(result)
    }
}

// Equivalent of FFmpeg's `FFERRTAG` macro to generate error codes.
#[allow(non_snake_case)]
const fn FFERRTAG(tag: &[u8; 4]) -> c_int {
    -(tag[0] as c_int | (tag[1] as c_int) << 8 | (tag[2] as c_int) << 16 | (tag[3] as c_int) << 24)
}

pub const AVERROR_EOF: c_int = FFERRTAG(b"EOF ");
pub const AVERROR_INVALIDDATA: c_int = FFERRTAG(b"INDA");
pub const AVERROR_EAGAIN: c_int = -11;
