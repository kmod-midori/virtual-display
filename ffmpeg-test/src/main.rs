use std::io::Write;

use ffmpeg_simple::{Codec, CodecContext, HwDeviceContext};

use anyhow::Result;

fn main() -> Result<()> {
    println!("Hello, world!");

    ffmpeg_simple::init_logging();

    let mut device_context = None;
    let mut codec = Codec::find_by_name("libx264").unwrap();

    for hw_codec_name in &["h264_qsv", "h264_nvenc", "h264_amf"] {
        let hw_codec = if let Some(codec) = Codec::find_by_name(hw_codec_name) {
            codec
        } else {
            continue;
        };

        for hw_config in hw_codec.hw_configs() {
            if !hw_config
                .methods
                .contains(ffmpeg_simple::codec::HwCodecSetupMethod::HwDeviceCtx)
            {
                continue;
            }

            if let Ok(ctx) = HwDeviceContext::new(hw_config.device_type) {
                device_context = Some(ctx);
                codec = hw_codec;
                break;
            }
        }

        if device_context.is_some() {
            break;
        }
    }

    println!(
        "Using codec {} ({}) with hardware context {:?}",
        codec.name(),
        codec.long_name(),
        device_context
    );

    let mut file = std::fs::File::create("Z:/1.264")?;

    let mut ctx = CodecContext::new(codec);
    ctx.set_size(1920, 1080)
        .set_framerate(60, 1)
        .set_time_base(1, 60)
        .set_pix_fmt(ffmpeg_simple::ffi::AVPixelFormat_AV_PIX_FMT_NV12)
        .set_option("profile", "baseline")?;
    // .set_option("tune", "ll")?
    // .set_option("preset", "medium")?;
    if let Some(device_context) = device_context {
        ctx.set_hw_device_ctx(device_context);
    }

    let mut ctx = ctx.open()?;

    for i in 0..100 {
        println!("{}", i);

        let frame = ctx.request_frame()?;
        let planes = frame.planes_mut();
        for mut plane in planes.into_iter().flatten() {
            plane.data().fill(128);
        }
        ctx.send_frame(i)?;

        while let Some(packet) = ctx.receive_packet()? {
            if let Some(data) = packet.data() {
                file.write_all(data)?;
            }
        }
    }

    println!("Done");

    Ok(())
}
