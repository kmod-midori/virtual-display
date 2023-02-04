use std::{any::Any, mem::MaybeUninit, ptr::null_mut, sync::Arc, time::Duration};

use crossbeam::channel::{Receiver, RecvTimeoutError, Sender};
pub use mfx_dispatch_sys as ffi;

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
    #[error("Device failed")]
    DeviceFailed,
    #[error("Unknown error: {0}")]
    Unknown(i32),
}

pub type Result<T> = std::result::Result<T, Error>;

pub type UserData = Box<dyn Any + Send>;

fn check_error(err: i32) -> Result<()> {
    match err {
        ffi::mfxStatus_MFX_ERR_NONE
        | ffi::mfxStatus_MFX_WRN_PARTIAL_ACCELERATION
        | ffi::mfxStatus_MFX_WRN_INCOMPATIBLE_VIDEO_PARAM => Ok(()),
        ffi::mfxStatus_MFX_ERR_UNSUPPORTED => Err(Error::Unsupported),
        ffi::mfxStatus_MFX_ERR_INVALID_VIDEO_PARAM => Err(Error::InvalidVideoParam),
        ffi::mfxStatus_MFX_ERR_INCOMPATIBLE_VIDEO_PARAM => Err(Error::IncompatibleVideoParam),
        ffi::mfxStatus_MFX_ERR_DEVICE_FAILED => Err(Error::DeviceFailed),
        err => Err(Error::Unknown(err)),
    }
}

/// A session with the Intel Media SDK.
#[derive(Debug)]
struct Session {
    raw: ffi::mfxSession,
    impl_: i32,
}

impl Session {
    pub fn new() -> Result<Self> {
        let mut version = ffi::mfxVersion {
            __bindgen_anon_1: ffi::mfxVersion__bindgen_ty_1 { Major: 1, Minor: 0 },
        };
        let mut session = null_mut();

        unsafe {
            check_error(ffi::MFXInit(
                ffi::MFX_IMPL_HARDWARE_ANY,
                &mut version,
                &mut session,
            ))?;
        };

        let mut impl_ = MaybeUninit::uninit();

        unsafe {
            check_error(ffi::MFXQueryIMPL(session, impl_.as_mut_ptr()))?;
        }

        Ok(Self {
            raw: session,
            impl_: unsafe { impl_.assume_init() },
        })
    }

    fn implementation(&self) -> i32 {
        self.impl_
    }
}

unsafe impl Send for Session {}

unsafe impl Sync for Session {}

pub struct InputBuffer {
    data: Vec<u8>,
    user_data: Option<UserData>,
    surface: Box<ffi::mfxFrameSurface1>,
    stride: usize,
}

impl InputBuffer {
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn stride(&self) -> usize {
        self.stride
    }

    pub fn set_user_data(&mut self, user_data: UserData) {
        self.user_data = Some(user_data);
    }
}

unsafe impl Send for InputBuffer {}

impl InputBuffer {
    fn locked(&self) -> bool {
        self.surface.Data.Locked != 0
    }
}

pub struct OutputBuffer {
    data: Vec<u8>,
    bitstream: Box<ffi::mfxBitstream>,
}

unsafe impl Send for OutputBuffer {}

/// Reference to an [`OutputBuffer`].
///
/// Drop this to release the buffer back to the encoder.
pub struct OutputBufferRef {
    /// Should always be `Some`, but we need to be able to take it out
    inner: Option<OutputBuffer>,
    user_data: Option<UserData>,
    tx: Sender<OutputBuffer>,
}

impl OutputBufferRef {
    pub fn data(&self) -> &[u8] {
        &self.inner.as_ref().unwrap().data
    }

    pub fn take_user_data(&mut self) -> Option<UserData> {
        self.user_data.take()
    }
}

impl Drop for OutputBufferRef {
    fn drop(&mut self) {
        if let Some(buffer) = self.inner.take() {
            // Release the buffer back to the encoder
            self.tx.send(buffer).ok();
        }
    }
}

struct SyncPoint(ffi::mfxSyncPoint);
unsafe impl Send for SyncPoint {}

struct SubmittedBuffers {
    input: InputBuffer,
    output: OutputBuffer,
    sync_point: SyncPoint,
    error: Option<Error>,
}

pub struct Encoder {
    input_buffer_release_rx: Receiver<InputBuffer>,
    input_buffer_submit_tx: Sender<InputBuffer>,

    sps: Vec<u8>,
    pps: Vec<u8>,
}

impl Encoder {
    /// Callbacks will be called on the thread that receives output from the hardware encoder,
    /// do not block for too long.
    pub fn new(
        width: u16,
        height: u16,
        framerate: u16,
        output_callback: Box<dyn FnMut(OutputBufferRef) + Send>,
        error_callback: Box<dyn FnMut(Error) + Send>,
    ) -> Result<Self> {
        let session = Session::new()?;

        let mut enc_par: ffi::mfxVideoParam = unsafe { std::mem::zeroed() };
        enc_par.IOPattern = ffi::MFX_IOPATTERN_IN_SYSTEM_MEMORY as u16
            | ffi::MFX_IOPATTERN_OUT_SYSTEM_MEMORY as u16;

        enc_par
            .__bindgen_anon_1
            .mfx
            .__bindgen_anon_1
            .__bindgen_anon_1
            .GopRefDist = 1;
        enc_par
            .__bindgen_anon_1
            .mfx
            .__bindgen_anon_1
            .__bindgen_anon_1
            .IdrInterval = 1;

        enc_par.__bindgen_anon_1.mfx.CodecId = ffi::MFX_CODEC_AVC as u32;
        enc_par.__bindgen_anon_1.mfx.CodecProfile = ffi::MFX_PROFILE_AVC_BASELINE as u16;
        enc_par
            .__bindgen_anon_1
            .mfx
            .__bindgen_anon_1
            .__bindgen_anon_1
            .TargetUsage = ffi::MFX_TARGETUSAGE_BEST_SPEED as u16;
        enc_par
            .__bindgen_anon_1
            .mfx
            .__bindgen_anon_1
            .__bindgen_anon_1
            .RateControlMethod = ffi::MFX_RATECONTROL_ICQ as u16;
        enc_par
            .__bindgen_anon_1
            .mfx
            .__bindgen_anon_1
            .__bindgen_anon_1
            .__bindgen_anon_2
            .ICQQuality = 27;

        enc_par.__bindgen_anon_1.mfx.FrameInfo.FrameRateExtN = framerate as u32;
        enc_par.__bindgen_anon_1.mfx.FrameInfo.FrameRateExtD = 1;
        enc_par.__bindgen_anon_1.mfx.FrameInfo.FourCC = ffi::MFX_FOURCC_NV12 as u32;
        enc_par.__bindgen_anon_1.mfx.FrameInfo.ChromaFormat = ffi::MFX_CHROMAFORMAT_YUV420 as u16;
        enc_par.__bindgen_anon_1.mfx.FrameInfo.PicStruct = ffi::MFX_PICSTRUCT_PROGRESSIVE as u16;

        let buffer_width = align32(width);
        let buffer_height = align32(height);

        enc_par
            .__bindgen_anon_1
            .mfx
            .FrameInfo
            .__bindgen_anon_1
            .__bindgen_anon_1
            .CropW = width;
        enc_par
            .__bindgen_anon_1
            .mfx
            .FrameInfo
            .__bindgen_anon_1
            .__bindgen_anon_1
            .CropH = height;

        enc_par
            .__bindgen_anon_1
            .mfx
            .FrameInfo
            .__bindgen_anon_1
            .__bindgen_anon_1
            .Width = buffer_width;
        enc_par
            .__bindgen_anon_1
            .mfx
            .FrameInfo
            .__bindgen_anon_1
            .__bindgen_anon_1
            .Height = buffer_height;

        let mut coding_option_2: ffi::mfxExtCodingOption2 = unsafe { std::mem::zeroed() };
        coding_option_2.Header = ffi::mfxExtBuffer {
            BufferId: ffi::MFX_EXTBUFF_CODING_OPTION2 as u32,
            BufferSz: std::mem::size_of::<ffi::mfxExtCodingOption2>() as u32,
        };
        coding_option_2.BRefType = ffi::MFX_B_REF_OFF as u16; // Disable B-frame

        let mut coding_option_3: ffi::mfxExtCodingOption3 = unsafe { std::mem::zeroed() };
        coding_option_3.Header = ffi::mfxExtBuffer {
            BufferId: ffi::MFX_EXTBUFF_CODING_OPTION3 as u32,
            BufferSz: std::mem::size_of::<ffi::mfxExtCodingOption3>() as u32,
        };
        coding_option_3.ScenarioInfo = ffi::MFX_SCENARIO_DISPLAY_REMOTING as u16;
        coding_option_3.ContentInfo = ffi::MFX_CONTENT_NON_VIDEO_SCREEN as u16;

        let ext_params =
            &mut [&mut coding_option_2 as *mut ffi::mfxExtCodingOption2 as *mut ffi::mfxExtBuffer];

        enc_par.ExtParam = ext_params.as_mut_ptr();
        enc_par.NumExtParam = ext_params.len() as u16;

        unsafe {
            check_error(ffi::MFXVideoENCODE_Query(
                session.raw,
                &mut enc_par,
                &mut enc_par,
            ))?;
        }

        let mut alloc_request = MaybeUninit::uninit();
        unsafe {
            check_error(ffi::MFXVideoENCODE_QueryIOSurf(
                session.raw,
                &mut enc_par,
                alloc_request.as_mut_ptr(),
            ))?;
        }
        let alloc_request = unsafe { alloc_request.assume_init() };

        let buffer_width =
            unsafe { alloc_request.Info.__bindgen_anon_1.__bindgen_anon_1.Width } as usize;
        let buffer_height =
            unsafe { alloc_request.Info.__bindgen_anon_1.__bindgen_anon_1.Height } as usize;

        let buffer_count = alloc_request.NumFrameSuggested as usize;
        let buffer_size = buffer_width * buffer_height * 3 / 2; // NV12

        let (input_buffer_release_tx, input_buffer_release_rx) =
            crossbeam::channel::bounded(buffer_count);
        for _ in 0..buffer_count {
            let mut buffer = vec![0u8; buffer_size];
            let buffer_y_ptr = buffer.as_mut_ptr();
            let buffer_uv_ptr = unsafe { buffer_y_ptr.add(buffer_width * buffer_height) };

            let mut surface: ffi::mfxFrameSurface1 = unsafe { std::mem::zeroed() };
            surface.Info = alloc_request.Info;
            surface.Data.__bindgen_anon_3.Y = buffer_y_ptr;
            surface.Data.__bindgen_anon_4.UV = buffer_uv_ptr;
            surface.Data.__bindgen_anon_2.PitchLow = buffer_width as u16;

            input_buffer_release_tx
                .send(InputBuffer {
                    data: buffer,
                    user_data: None,
                    surface: Box::new(surface),
                    stride: buffer_width,
                })
                .expect("The channel should not be disconnected");
        }

        unsafe {
            check_error(ffi::MFXVideoENCODE_Init(session.raw, &mut enc_par))?;
        }

        let mut active_enc_par: ffi::mfxVideoParam = unsafe { std::mem::zeroed() };

        let mut sps_buffer = vec![0u8; 128];
        let mut pps_buffer = vec![0u8; 128];
        let mut coding_option_sps_pps = ffi::mfxExtCodingOptionSPSPPS {
            Header: ffi::mfxExtBuffer {
                BufferId: ffi::MFX_EXTBUFF_CODING_OPTION_SPSPPS as u32,
                BufferSz: std::mem::size_of::<ffi::mfxExtCodingOptionSPSPPS>() as u32,
            },
            SPSBuffer: sps_buffer.as_mut_ptr(),
            PPSBuffer: pps_buffer.as_mut_ptr(),
            SPSBufSize: sps_buffer.len() as u16,
            PPSBufSize: pps_buffer.len() as u16,
            SPSId: 0,
            PPSId: 0,
        };

        let ext_params = &mut [
            &mut coding_option_sps_pps as *mut ffi::mfxExtCodingOptionSPSPPS
                as *mut ffi::mfxExtBuffer,
        ];
        active_enc_par.ExtParam = ext_params.as_mut_ptr();
        active_enc_par.NumExtParam = ext_params.len() as u16;

        unsafe {
            check_error(ffi::MFXVideoENCODE_GetVideoParam(
                session.raw,
                &mut active_enc_par,
            ))?;
        }

        let output_buffer_size = unsafe {
            active_enc_par
                .__bindgen_anon_1
                .mfx
                .__bindgen_anon_1
                .__bindgen_anon_1
                .BufferSizeInKB as usize
                * 1024
        };
        let (output_buffer_release_tx, output_buffer_release_rx) =
            crossbeam::channel::bounded(buffer_count);
        for _ in 0..buffer_count {
            let mut output_buffer = vec![0u8; output_buffer_size];

            let mut bitstream: ffi::mfxBitstream = unsafe { std::mem::zeroed() };
            bitstream.MaxLength = output_buffer_size as u32;
            bitstream.Data = output_buffer.as_mut_ptr();

            output_buffer_release_tx
                .send(OutputBuffer {
                    data: output_buffer,
                    bitstream: Box::new(bitstream),
                })
                .expect("The channel should not be disconnected");
        }

        let (input_buffer_submit_tx, input_buffer_submit_rx) =
            crossbeam::channel::bounded(buffer_count);

        let (submitted_buffers_tx, submitted_buffers_rx) =
            crossbeam::channel::bounded(buffer_count);

        let shared_session = Arc::new(session);
        let session = shared_session.clone();
        std::thread::spawn(move || {
            producer_thread(
                session,
                input_buffer_submit_rx,
                output_buffer_release_rx,
                submitted_buffers_tx,
            );
        });

        let session = shared_session;
        std::thread::spawn(move || {
            consumer_thread(
                session,
                submitted_buffers_rx,
                output_buffer_release_tx,
                input_buffer_release_tx,
                output_callback,
                error_callback,
            );
        });

        Ok(Self {
            input_buffer_submit_tx,
            input_buffer_release_rx,

            sps: sps_buffer[..coding_option_sps_pps.SPSBufSize as usize].to_vec(),
            pps: pps_buffer[..coding_option_sps_pps.PPSBufSize as usize].to_vec(),
        })
    }

    pub fn sps(&self) -> &[u8] {
        &self.sps
    }

    pub fn pps(&self) -> &[u8] {
        &self.pps
    }

    pub fn recv_input_buffer(&self) -> InputBuffer {
        self.input_buffer_release_rx.recv().unwrap()
    }

    pub fn recv_input_buffer_timeout(&self, timeout: Duration) -> Option<InputBuffer> {
        match self.input_buffer_release_rx.recv_timeout(timeout) {
            Ok(surface) => Some(surface),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => {
                panic!("Channel disconnected")
            }
        }
    }

    pub fn submit_input_buffer(&self, buffer: InputBuffer) {
        self.input_buffer_submit_tx
            .send(buffer)
            .expect("Channel disconnected");
    }

    pub fn submit_input_buffer_timeout(
        &self,
        buffer: InputBuffer,
        timeout: Duration,
    ) -> std::result::Result<(), InputBuffer> {
        match self.input_buffer_submit_tx.send_timeout(buffer, timeout) {
            Ok(_) => Ok(()),
            Err(e) => match e {
                crossbeam::channel::SendTimeoutError::Timeout(buffer) => Err(buffer),
                crossbeam::channel::SendTimeoutError::Disconnected(_) => {
                    panic!("Channel disconnected")
                }
            },
        }
    }
}

/// This function stops when all senders of `input_buffer_submit_rx`
/// and all [`OutputBufferRef`] are dropped.
fn producer_thread(
    session: Arc<Session>,
    input_buffer_submit_rx: Receiver<InputBuffer>,
    output_buffer_release_rx: Receiver<OutputBuffer>,
    submitted_buffers_tx: Sender<SubmittedBuffers>,
) {
    let mut last_output_buffer = None;

    while let Ok(mut buffer) = input_buffer_submit_rx.recv() {
        let mut output_buffer = if let Some(b) = last_output_buffer.take() {
            b
        } else if let Ok(buffer) = output_buffer_release_rx.recv() {
            buffer
        } else {
            break;
        };

        let (sync_point, status) = loop {
            let mut sync_point = null_mut();

            let status = unsafe {
                ffi::MFXVideoENCODE_EncodeFrameAsync(
                    session.raw,
                    null_mut(),
                    buffer.surface.as_mut(),
                    output_buffer.bitstream.as_mut(),
                    &mut sync_point,
                )
            };

            if status == ffi::mfxStatus_MFX_WRN_DEVICE_BUSY {
                // The device is busy, wait a bit and try again.
                std::thread::sleep(Duration::from_millis(1));
            } else {
                break (sync_point, status);
            }
        };

        let error = match status {
            ffi::mfxStatus_MFX_ERR_MORE_DATA => {
                // No output produced, reuse the buffer for next operation.
                last_output_buffer = Some(output_buffer);
                continue;
            }
            ffi::mfxStatus_MFX_ERR_NONE => {
                // Output produced
                None
            }
            status => {
                // Should not be `Ok(())`, as `MFXVideoENCODE_EncodeFrameAsync` does not return other warnings.
                // https://spec.oneapi.io/versions/latest/elements/oneVPL/source/API_ref/VPL_func_vid_encode.html#mfxvideoencode-encodeframeasync
                check_error(status).err()
            }
        };

        let submitted_buffers = SubmittedBuffers {
            input: buffer,
            output: output_buffer,
            sync_point: SyncPoint(sync_point),
            error,
        };

        submitted_buffers_tx.send(submitted_buffers).unwrap();
    }
}

/// This thread stops when the sender of `submitted_buffers_rx` ([`producer_thread`]) is dropped.
fn consumer_thread(
    session: Arc<Session>,
    submitted_buffers_rx: Receiver<SubmittedBuffers>,
    output_buffer_release_tx: Sender<OutputBuffer>,
    input_buffer_release_tx: Sender<InputBuffer>,
    mut output_callback: Box<dyn FnMut(OutputBufferRef)>,
    mut error_callback: Box<dyn FnMut(Error)>,
) {
    let mut input_buffers = Vec::new();

    while let Ok(mut submitted_buffers) = submitted_buffers_rx.recv() {
        let error = if let Some(err) = submitted_buffers.error {
            Some(err)
        } else {
            let status = unsafe {
                ffi::MFXVideoCORE_SyncOperation(
                    session.raw,
                    submitted_buffers.sync_point.0,
                    ffi::MFX_INFINITE,
                )
            };

            check_error(status).err()
        };

        if let Some(err) = error {
            // Error occurred, release the output buffer back to the encoder
            error_callback(err);
            output_buffer_release_tx
                .send(submitted_buffers.output)
                .unwrap();
        } else {
            // Output produced
            let output_buffer_ref = OutputBufferRef {
                inner: Some(submitted_buffers.output),
                tx: output_buffer_release_tx.clone(),
                user_data: submitted_buffers.input.user_data.take(),
            };
            output_callback(output_buffer_ref);
        }

        // Release the input buffer back to the encoder
        if submitted_buffers.input.locked() {
            // Release immediately
            input_buffer_release_tx
                .send(submitted_buffers.input)
                .unwrap();
        } else {
            // Schedule for release later
            input_buffers.push(Some(submitted_buffers.input));
        }

        // Release all input buffers that are not locked
        // `drain_filter` is not stable yet, so we do this for now.
        input_buffers.retain_mut(|buffer| {
            if let Some(buffer_ref) = buffer {
                if buffer_ref.locked() {
                    true
                } else {
                    input_buffer_release_tx
                        .send(buffer.take().unwrap())
                        .unwrap();
                    false
                }
            } else {
                false
            }
        });
    }
}

fn align32(x: u16) -> u16 {
    ((x + 31) >> 5) << 5
}

#[test]
fn encode() {
    let encoder = Encoder::new(
        1920,
        1080,
        60,
        Box::new(|s| {}),
        Box::new(|e| {
            eprintln!("Error: {:?}", e);
        }),
    )
    .unwrap();

    for i in 0..30 {
        let start = std::time::Instant::now();
        let mut buffer = encoder.recv_input_buffer();
        buffer.data_mut().fill(128);
        encoder.submit_input_buffer(buffer);
    }
}
