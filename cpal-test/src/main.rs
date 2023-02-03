use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};

fn main() {
    let host = cpal::default_host();
    let device = host.default_output_device().unwrap();
    let audio_cfg = device.default_output_config().unwrap();
    dbg!(&audio_cfg);

    let stream = if let cpal::SampleFormat::F32 = audio_cfg.sample_format() {
        let stream = device
            .build_input_stream(
                &audio_cfg.config(),
                move |data: &[f32], _: &_| {
                    println!("data: {:?}", data);
                },
                |err| {
                    println!("err: {:?}", err);
                },
            )
            .unwrap();
        stream.play().unwrap();
        stream
    } else {
        panic!("Unsupported sample format");
    };

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
