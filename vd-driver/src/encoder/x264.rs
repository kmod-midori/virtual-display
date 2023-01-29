use anyhow::Result;
use dcv_color_primitives as dcp;
use x264::Colorspace;

pub struct Encoder {
    encoder: x264::Encoder,
    picture: x264::Picture,
    width: u32,
    height: u32,
    framerate: u32,
    pts: i64,
    nv12_buffer_y: Vec<u8>,
    nv12_buffer_u: Vec<u8>,
    nv12_buffer_v: Vec<u8>,
}

impl Encoder {
    pub fn new(width: u32, height: u32, framerate: u32) -> x264::Result<Self> {
        let encoder = Self::new_encoder(width, height, framerate)?;
        let mut this = Self {
            encoder,
            picture: x264::Picture::new(),
            width,
            height,
            framerate,
            pts: 0,
            nv12_buffer_y: vec![],
            nv12_buffer_u: vec![],
            nv12_buffer_v: vec![],
        };
        this.resize_buffers();

        Ok(this)
    }

    fn new_encoder(width: u32, height: u32, framerate: u32) -> x264::Result<x264::Encoder> {
        x264::Setup::preset(x264::Preset::Veryfast, x264::Tune::None, true, true)
            .fps(framerate, 1)
            .build(Colorspace::I420, width as i32, height as i32)
    }

    fn resize_buffers(&mut self) {
        self.nv12_buffer_y
            .resize((self.width * self.height) as usize, 0);
        self.nv12_buffer_u
            .resize(((self.width * self.height) / 4) as usize, 0);
        self.nv12_buffer_v
            .resize(((self.width * self.height) / 4) as usize, 0);
    }

    pub fn reconfigure(&mut self, width: u32, height: u32, framerate: u32) -> x264::Result<()> {
        self.width = width;
        self.height = height;
        self.framerate = framerate;
        self.pts = 0;

        self.resize_buffers();
        self.encoder = Self::new_encoder(width, height, framerate)?;

        Ok(())
    }

    pub fn headers(&mut self) -> x264::Result<x264::Data> {
        self.encoder.headers()
    }

    /// Encodes a frame from BGRA buffer and increments the pts.
    pub fn encode_frame<'a>(&'a mut self, buffer: &[u8]) -> Result<x264::Data<'a>> {
        if buffer.len() != (self.width * self.height * 4) as usize {
            anyhow::bail!("buffer size mismatch");
        }

        let dcp_src_format = dcp::ImageFormat {
            pixel_format: dcp::PixelFormat::Bgra,
            color_space: dcp::ColorSpace::Rgb,
            num_planes: 1,
        };

        let dcp_dst_format = dcp::ImageFormat {
            pixel_format: dcp::PixelFormat::I420,
            color_space: dcp::ColorSpace::Bt709,
            num_planes: 3,
        };

        dcp::convert_image(
            self.width,
            self.height,
            &dcp_src_format,
            None,
            &[buffer],
            &dcp_dst_format,
            None,
            &mut [
                &mut self.nv12_buffer_y,
                &mut self.nv12_buffer_u,
                &mut self.nv12_buffer_v,
            ],
        )?;

        let plane_y = x264::Plane {
            stride: self.width as i32,
            data: &self.nv12_buffer_y,
        };
        let plane_u = x264::Plane {
            stride: self.width as i32 / 2,
            data: &self.nv12_buffer_u,
        };
        let plane_v = x264::Plane {
            stride: self.width as i32 / 2,
            data: &self.nv12_buffer_v,
        };
        let image = unsafe {
            x264::Image::new_unchecked(
                x264::Colorspace::I420.into(),
                self.width as i32,
                self.height as i32,
                &[plane_y, plane_u, plane_v],
            )
        };

        let data = if let Ok(data) = self.encoder.encode(self.pts, &mut self.picture, image) {
            data
        } else {
            anyhow::bail!("Failed to encode frame");
        };

        self.pts += 1;

        Ok(data)
    }
}
