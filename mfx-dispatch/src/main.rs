use std::error::Error;
use webrtc::{
    media::io::h264_reader::H264Reader,
    rtp::packetizer::Packetizer,
    util::{Marshal, MarshalSize},
};
use mfx_dispatch::{Pipeline, Session};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let session = Session::new()?;
    println!("impl {:?}", session.implementation());

    let clock_rate = 90000;
    let sequencer: Box<dyn webrtc::rtp::sequence::Sequencer + Send + Sync> =
        Box::new(webrtc::rtp::sequence::new_random_sequencer());
    let mut packetizer = webrtc::rtp::packetizer::new_packetizer(
        1200,
        0, // Value is handled when writing
        0, // Value is handled when writing
        Box::<webrtc::rtp::codecs::h264::H264Payloader>::default(),
        sequencer.clone(),
        clock_rate,
    );

    let mut pipeline = Pipeline::new(session, 1920, 1080, 60)?;
    dbg!(pipeline.pps());
    for i in 0..60 {
        let start = std::time::Instant::now();
        let (buf_idx, buf_y, buf_uv) = pipeline.get_free_surface().unwrap();
        buf_y.fill(128);
        buf_uv.fill(128);
        let data = pipeline.encode_frame(buf_idx, false)?;
        println!("frame {} took {:?}", i, start.elapsed());

        if let Some(data) = data {
            let mut h264 = H264Reader::new(std::io::Cursor::new(data));

            while let Ok(nal) = h264.next_nal() {
                let samples = (16.0 * clock_rate as f64) as u32;
                let packets = packetizer.packetize(&nal.data.freeze(), samples).await?;

                for packet in packets {
                    let len = packet.marshal_size();
                    let len_be = (len as u16).to_be_bytes();
                    let mut buf = vec![0; len + 4];
                    buf[0] = b'$';
                    buf[1] = 0;
                    buf[2] = len_be[0];
                    buf[3] = len_be[1];
                    packet.marshal_to(&mut buf[4..])?;
                    // conn.write_all(&buf).await?;
                }
            }
        }
    }

    Ok(())
}
