use std::error::Error;
use mfx_dispatch::{Pipeline, Session};

fn main() -> Result<(), Box<dyn Error>> {
    let session = Session::new()?;
    println!("impl {:?}", session.implementation());

    let mut pipeline = Pipeline::new(session, 1920, 1080, 60)?;

    for i in 0..60 {
        let start = std::time::Instant::now();
        let (buf_idx, buf_y, buf_uv) = pipeline.get_free_surface().unwrap();
        buf_y.fill(128);
        buf_uv.fill(128);
        let _data = pipeline.encode_frame(buf_idx, false)?;
        println!("frame {} took {:?}", i, start.elapsed());
    }

    Ok(())
}
