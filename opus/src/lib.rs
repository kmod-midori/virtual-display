use std::ffi::c_int;

use opus_sys as ffi;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unknown error: {0}")]
    Unknown(i32),
    #[error("One or more invalid/out of range arguments.")]
    BadArgument,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy)]
pub enum Channels {
    Mono = 1,
    Stereo = 2,
}

#[derive(Debug, Clone, Copy)]
pub enum Application {
    /// Best for most VoIP/videoconference applications where listening quality and intelligibility matter most.
    Voip = ffi::OPUS_APPLICATION_VOIP as isize,
    /// Best for broadcast/high-fidelity application where the decoded audio should be as close as possible to the input.
    Audio = ffi::OPUS_APPLICATION_AUDIO as isize,
    /// Only use when lowest-achievable latency is what matters most.
    ///
    /// Voice-optimized modes cannot be used.
    RestrictedLowDelay = ffi::OPUS_APPLICATION_RESTRICTED_LOWDELAY as isize,
}

fn check_error(code: i32) -> Result<()> {
    const OPUS_OK: i32 = ffi::OPUS_OK as i32;
    match code {
        OPUS_OK => Ok(()),
        ffi::OPUS_BAD_ARG => Err(Error::BadArgument),
        _ => Err(Error::Unknown(code)),
    }
}

pub struct Encoder {
    raw: *mut ffi::OpusEncoder,
    _sample_rate: u32,
    channels: Channels,
}

impl Encoder {
    pub fn new(sample_rate: u32, channels: Channels, application: Application) -> Result<Self> {
        let mut error = 0;
        let raw = unsafe {
            ffi::opus_encoder_create(
                sample_rate as i32,
                channels as c_int,
                application as c_int,
                &mut error,
            )
        };

        check_error(error)?;

        if raw.is_null() {
            return Err(Error::Unknown(0));
        }

        Ok(Self {
            raw,
            _sample_rate: sample_rate,
            channels,
        })
    }

    // pub fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
    //     let ret =
    //         unsafe { ffi::opus_encoder_ctl(self.raw, ffi::OPUS_SET_BITRATE_REQUEST, bitrate) };
    //     check_error(ret)
    // }

    pub fn encode(&mut self, pcm: &[i16], data: &mut [u8]) -> Result<usize> {
        let frame_size = pcm.len() / self.channels as usize;
        let len = unsafe {
            ffi::opus_encode(
                self.raw,
                // frame_size * channels * sizeof(opus_int16)
                pcm.as_ptr(),
                frame_size as c_int,
                data.as_mut_ptr(),
                data.len() as c_int,
            )
        };

        if len < 0 {
            check_error(len)?;
        }

        Ok(len as usize)
    }

    pub fn encode_f32(&mut self, pcm: &[f32], data: &mut [u8]) -> Result<usize> {
        let frame_size = pcm.len() / self.channels as usize;
        let len = unsafe {
            ffi::opus_encode_float(
                self.raw,
                // frame_size * channels * sizeof(float)
                pcm.as_ptr(),
                frame_size as c_int,
                data.as_mut_ptr(),
                data.len() as c_int,
            )
        };

        if len < 0 {
            check_error(len)?;
        }

        Ok(len as usize)
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            ffi::opus_encoder_destroy(self.raw);
        }
    }
}

// The opus encoder can be used from a single thread, but that thread may change.
unsafe impl Send for Encoder {}
