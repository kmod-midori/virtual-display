use std::{
    ptr::null_mut,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use crossbeam::channel;
use dcv_color_primitives as dcp;
use windows::{
    core::{AgileReference, Interface, ManuallyDrop},
    Win32::{
        Media::MediaFoundation::{
            self, IMFAttributes, IMFMediaEventGenerator, IMFMediaType, IMFTransform, MFTEnumEx,
        },
        System::Com::CoTaskMemFree,
    },
};

use crate::win32::Variant;

pub struct EncoderHandle {
    frame_tx: channel::Sender<Arc<Mutex<Vec<u8>>>>,
    data_rx: channel::Receiver<Arc<Mutex<Vec<u8>>>>,
}

impl EncoderHandle {
    pub fn encode_frame(&mut self, buffer: Arc<Mutex<Vec<u8>>>) -> Result<Arc<Mutex<Vec<u8>>>> {
        let strat = std::time::Instant::now();
        self.frame_tx.send(buffer)?;
        let res = self.data_rx.recv()?;
        tracing::trace!("encode_frame took {} ms", strat.elapsed().as_millis());
        Ok(res)
    }
}

pub struct Encoder {
    transform: AgileReference<IMFTransform>,
    event_generator: AgileReference<IMFMediaEventGenerator>,

    frame_rx: channel::Receiver<Arc<Mutex<Vec<u8>>>>,
    data_tx: channel::Sender<Arc<Mutex<Vec<u8>>>>,

    input_stream_id: u32,
    output_stream_id: u32,
    width: u32,
    height: u32,
    framerate: u32,

    pts: i64,
}

impl Encoder {
    fn create_transform() -> Result<IMFTransform> {
        let transform = unsafe {
            let output_type = MediaFoundation::MFT_REGISTER_TYPE_INFO {
                guidMajorType: MediaFoundation::MFMediaType_Video,
                guidSubtype: MediaFoundation::MFVideoFormat_H264,
            };

            let flags = MediaFoundation::MFT_ENUM_FLAG_HARDWARE
                | MediaFoundation::MFT_ENUM_FLAG_ASYNCMFT
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

            tracing::info!("Found {} H.264 MFTs", nummftactivate);

            let activates = std::slice::from_raw_parts_mut(ppmftactivate, nummftactivate as usize);

            let mut transform = None;

            for activate in activates {
                let activate = activate.take().unwrap();

                if transform.is_none() {
                    let transform_attributes = activate.cast::<IMFAttributes>()?;

                    let clsid = transform_attributes
                        .GetGUID(&MediaFoundation::MFT_TRANSFORM_CLSID_Attribute)?;
                    tracing::info!(?clsid, "Activating MFT");

                    match activate.ActivateObject::<MediaFoundation::IMFTransform>() {
                        Ok(a) => transform = Some(a),
                        Err(e) => {
                            tracing::error!(?clsid, ?e, "Failed to activate MFT");
                            continue;
                        }
                    }
                }
            }

            CoTaskMemFree(Some(ppmftactivate as *const _));

            match transform {
                Some(t) => t,
                None => anyhow::bail!("Failed to find a suitable H.264 MFT"),
            }
        };

        Ok(transform)
    }

    fn setup_input_type(
        input_type: IMFMediaType,
        width: u32,
        height: u32,
        framerate: u32,
    ) -> Result<IMFMediaType> {
        unsafe {
            input_type.SetGUID(
                &MediaFoundation::MF_MT_MAJOR_TYPE,
                &MediaFoundation::MFMediaType_Video,
            )?;
            input_type.SetGUID(
                &MediaFoundation::MF_MT_SUBTYPE,
                &MediaFoundation::MFVideoFormat_NV12,
            )?;
            input_type.SetUINT64(
                &MediaFoundation::MF_MT_FRAME_SIZE,
                (width as u64) << 32 | (height as u64),
            )?;
            input_type.SetUINT64(
                &MediaFoundation::MF_MT_FRAME_RATE,
                (framerate as u64) << 32 | 1,
            )?;
            // Set this attribute to TRUE for all uncompressed media types.
            input_type.SetUINT32(&MediaFoundation::MF_MT_ALL_SAMPLES_INDEPENDENT, 1)?;
        }

        Ok(input_type)
    }

    fn setup_output_type(
        output_type: IMFMediaType,
        width: u32,
        height: u32,
        framerate: u32,
    ) -> Result<IMFMediaType> {
        unsafe {
            output_type.SetGUID(
                &MediaFoundation::MF_MT_MAJOR_TYPE,
                &MediaFoundation::MFMediaType_Video,
            )?;
            output_type.SetGUID(
                &MediaFoundation::MF_MT_SUBTYPE,
                &MediaFoundation::MFVideoFormat_H264,
            )?;
            // output_type.SetUINT32(&MediaFoundation::MF_MT_AVG_BITRATE, bitrate_bps)?;
            output_type.SetUINT64(
                &MediaFoundation::MF_MT_FRAME_SIZE,
                (width as u64) << 32 | (height as u64),
            )?;
            output_type.SetUINT64(
                &MediaFoundation::MF_MT_FRAME_RATE,
                (framerate as u64) << 32 | 1,
            )?;
            output_type.SetUINT32(
                &MediaFoundation::MF_MT_INTERLACE_MODE,
                MediaFoundation::MFVideoInterlace_Progressive.0 as u32,
            )?;
            output_type.SetUINT32(&MediaFoundation::MF_MT_ALL_SAMPLES_INDEPENDENT, 0)?;
            output_type.SetUINT32(
                &MediaFoundation::MF_MT_MPEG2_PROFILE,
                MediaFoundation::eAVEncH264VProfile_Base.0 as u32,
            )?;
        }

        Ok(output_type)
    }

    pub fn new_handle(width: u32, height: u32, framerate: u32) -> Result<EncoderHandle> {
        let transform = Self::create_transform()?;

        tracing::info!("Created transform");

        let encoder_attrs = unsafe { transform.GetAttributes()? };

        tracing::info!("Set transform attributes");

        unsafe {
            // Unlock so that this MFT can be used asynchronously.
            encoder_attrs.SetUINT32(&MediaFoundation::MF_TRANSFORM_ASYNC_UNLOCK, 1)?;
            // Enable low-latency
            encoder_attrs.SetUINT32(&MediaFoundation::MF_LOW_LATENCY, 1)?;
        }

        let (input_stream_id, output_stream_id) = unsafe {
            let input_id = &mut [0];
            let output_id = &mut [0];
            if let Err(e) = transform.GetStreamIDs(input_id, output_id) {
                tracing::warn!(?e, "Failed to get stream IDs, falling back to 0, 0");
                input_id[0] = 0;
                output_id[0] = 0;
            }
            (input_id[0], output_id[0])
        };

        tracing::info!("Set codec API values");

        unsafe {
            let codec_api = transform.cast::<MediaFoundation::ICodecAPI>()?;

            let val = Variant::from(MediaFoundation::eAVEncCommonRateControlMode_Quality.0 as u32);
            codec_api.SetValue(
                &MediaFoundation::CODECAPI_AVEncCommonRateControlMode,
                val.as_ptr(),
            )?;
            let val = Variant::from(50u32);
            codec_api.SetValue(&MediaFoundation::CODECAPI_AVEncCommonQuality, val.as_ptr())?;

            // let val = Variant::from(20_000_000_u32);
            // codec_api.SetValue(
            //     &MediaFoundation::CODECAPI_AVEncCommonMeanBitRate,
            //     val.as_ptr(),
            // )?;
            let val = Variant::from(true);
            codec_api
                .SetValue(&MediaFoundation::CODECAPI_AVEncCommonRealTime, val.as_ptr())
                .ok();
            let val = Variant::from(true);
            codec_api.SetValue(&MediaFoundation::CODECAPI_AVLowLatencyMode, val.as_ptr())?;
            let val = Variant::from(0u32); // Speed first
            codec_api.SetValue(
                &MediaFoundation::CODECAPI_AVEncCommonQualityVsSpeed,
                val.as_ptr(),
            )?;
            // let val = Variant::from(2u32);
            // codec_api.SetValue(
            //     &MediaFoundation::CODECAPI_AVEncMPVDefaultBPictureCount,
            //     val.as_ptr(),
            // )?;
            // let val = Variant::from(16384u32);
            // codec_api.SetValue(&MediaFoundation::CODECAPI_AVEncMPVGOPSize, val.as_ptr())?;
        }

        tracing::info!("Set output type");

        unsafe {
            let output_type = Self::setup_output_type(
                transform.GetOutputAvailableType(output_stream_id, 0)?,
                width,
                height,
                framerate,
            )?;

            transform.SetOutputType(output_stream_id, &output_type, 0)?;
        }

        tracing::info!("Set input type");

        unsafe {
            let input_type = Self::setup_input_type(
                transform.GetInputAvailableType(input_stream_id, 0)?,
                width,
                height,
                framerate,
            )?;
            transform.SetInputType(input_stream_id, &input_type, 0)?;
        }

        let (frame_tx, frame_rx) = channel::bounded(0);
        let (data_tx, data_rx) = channel::bounded(0);

        let this = Self {
            event_generator: AgileReference::new(&transform.cast()?)?,
            transform: AgileReference::new(&transform)?,

            frame_rx,
            data_tx,

            input_stream_id,
            output_stream_id,
            width,
            height,
            framerate,
            pts: 0,
        };

        std::thread::spawn(move || {
            if let Err(e) = this.run() {
                tracing::error!(?e, "Encoder thread error");
            }
        });

        Ok(EncoderHandle { frame_tx, data_rx })
    }

    fn run(mut self) -> Result<()> {
        let input_buffer = unsafe {
            MediaFoundation::MFCreate2DMediaBuffer(
                self.width,
                self.height,
                0x3231564e, /* 'NV12' */
                false,
            )?
        };
        let input_buffer_2d = input_buffer.cast::<MediaFoundation::IMF2DBuffer>()?;

        let output_buffer = Arc::new(Mutex::new(Vec::new()));

        let transform = self.transform.resolve()?;
        let event_generator = self.event_generator.resolve()?;

        unsafe {
            transform.ProcessMessage(MediaFoundation::MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            transform.ProcessMessage(MediaFoundation::MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
        }

        loop {
            let event_type = unsafe {
                event_generator
                    .GetEvent(MediaFoundation::MF_EVENT_FLAG_NONE)?
                    .GetType()?
            };

            match MediaFoundation::MF_EVENT_TYPE(event_type as i32) {
                MediaFoundation::METransformNeedInput => {
                    let frame = if let Ok(frame) = self.frame_rx.recv() {
                        frame
                    } else {
                        break;
                    };

                    tracing::trace!("Got new frame");

                    let dcp_src_format = dcp::ImageFormat {
                        pixel_format: dcp::PixelFormat::Bgra,
                        color_space: dcp::ColorSpace::Rgb,
                        num_planes: 1,
                    };

                    let dcp_dst_format = dcp::ImageFormat {
                        pixel_format: dcp::PixelFormat::Nv12,
                        color_space: dcp::ColorSpace::Bt709,
                        num_planes: 1,
                    };

                    let mut input_buffer_raw = std::ptr::null_mut();
                    let mut input_buffer_raw_pitch = 0;
                    unsafe {
                        input_buffer_2d
                            .Lock2D(&mut input_buffer_raw, &mut input_buffer_raw_pitch)?;

                        // Stride = pitch - width (extra bytes per row)
                        let dst_strides = [(input_buffer_raw_pitch as u32 - self.width) as usize];
                        let mut dst_buffers_size = [0];

                        dcp::get_buffers_size(
                            self.width,
                            self.height,
                            &dcp_dst_format,
                            Some(&dst_strides),
                            &mut dst_buffers_size,
                        )?;

                        let dst_buffer =
                            std::slice::from_raw_parts_mut(input_buffer_raw, dst_buffers_size[0]);

                        {
                            let frame_guard = frame.lock().unwrap();

                            dcp::convert_image(
                                self.width,
                                self.height,
                                &dcp_src_format,
                                None,
                                &[&frame_guard],
                                &dcp_dst_format,
                                Some(&dst_strides),
                                &mut [dst_buffer],
                            )?;
                        }

                        input_buffer_2d.Unlock2D()?;

                        let input_sample = MediaFoundation::MFCreateSample()?;
                        input_sample.AddBuffer(&input_buffer)?;
                        input_sample.SetSampleTime(self.pts)?;
                        transform.ProcessInput(self.input_stream_id, &input_sample, 0)?;

                        self.pts += 1;

                        tracing::trace!("transform.ProcessInput");
                    }
                }
                MediaFoundation::METransformHaveOutput => {
                    tracing::trace!("METransformHaveOutput");

                    let mut v = [MediaFoundation::MFT_OUTPUT_DATA_BUFFER {
                        dwStreamID: self.output_stream_id,
                        pSample: ManuallyDrop::none(),
                        dwStatus: 0,
                        pEvents: ManuallyDrop::none(),
                    }];

                    let mut status = 0;
                    unsafe {
                        let res = transform.ProcessOutput(
                            MediaFoundation::MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32,
                            &mut v,
                            &mut status,
                        );

                        match res {
                            Ok(_) => {
                                let b = v[0].pSample.unwrap().ConvertToContiguousBuffer()?;
                                let mut data = null_mut();
                                let mut data_len = 0;
                                b.Lock(&mut data, None, Some(&mut data_len))?;

                                let slice = std::slice::from_raw_parts(
                                    data as *const u8,
                                    data_len as usize,
                                );

                                {
                                    let mut output_buffer = output_buffer.lock().unwrap();
                                    output_buffer.clear();
                                    output_buffer.extend_from_slice(slice);
                                }

                                b.Unlock()?;

                                if self.data_tx.send(output_buffer.clone()).is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                if e.code() == MediaFoundation::MF_E_TRANSFORM_STREAM_CHANGE {
                                    tracing::info!("Renegotiating output stream");

                                    let output_type = Self::setup_output_type(
                                        transform
                                            .GetOutputAvailableType(self.output_stream_id, 0)?,
                                        self.width,
                                        self.height,
                                        self.framerate,
                                    )?;

                                    transform.SetOutputType(
                                        self.output_stream_id,
                                        &output_type,
                                        0,
                                    )?;
                                } else {
                                    return Err(e.into());
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        unsafe {
            transform.ProcessMessage(MediaFoundation::MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0)?;
            transform.ProcessMessage(MediaFoundation::MFT_MESSAGE_NOTIFY_END_STREAMING, 0)?;
        }

        Ok(())
    }
}
