use crate::{Data, Encoding, Error, Image, Picture, Result, Setup};
use core::{mem, ptr};
use x264_sys::*;

/// Encodes video.
pub struct Encoder {
    raw: *mut x264_t,
    params: x264_param_t,
}

impl Encoder {
    /// Creates a new builder with default options.
    ///
    /// For more options see `Setup::new`.
    pub fn builder() -> Setup {
        Setup::default()
    }

    #[doc(hidden)]
    pub unsafe fn from_raw(raw: *mut x264_t) -> Self {
        let mut params = mem::MaybeUninit::uninit().assume_init();
        x264_encoder_parameters(raw, &mut params);
        Self { raw, params }
    }

    /// Feeds a frame to the encoder.
    ///
    /// # Panics
    ///
    /// Panics if there is a mismatch between the image and the encoder
    /// regarding width, height or colorspace.
    pub fn encode(&mut self, pts: i64, picture: &mut Picture, image: Image) -> Result<Data> {
        assert_eq!(image.width(), self.width());
        assert_eq!(image.height(), self.height());
        assert_eq!(image.encoding(), self.encoding());
        unsafe { self.encode_unchecked(pts, picture, image) }
    }

    /// Feeds a frame to the encoder.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the width, height *and* colorspace
    /// of the image are the same as that of the encoder.
    pub unsafe fn encode_unchecked(
        &mut self,
        pts: i64,
        picture: &mut Picture,
        image: Image,
    ) -> Result<Data> {
        let image = image.raw();

        picture.raw.i_pts = pts;
        picture.raw.img = image;

        let mut len = 0;
        let mut stuff = mem::MaybeUninit::uninit();
        let mut raw = mem::MaybeUninit::uninit();

        let err = x264_encoder_encode(
            self.raw,
            stuff.as_mut_ptr(),
            &mut len,
            &mut picture.raw,
            raw.as_mut_ptr(),
        );

        let stuff = stuff.assume_init();
        // let raw = raw.assume_init();

        if err < 0 {
            Err(Error)
        } else {
            let data = Data::from_raw_parts(stuff, len as usize);
            Ok(data)
        }
    }

    /// Gets the video headers, which should be sent first.
    pub fn headers(&mut self) -> Result<Data> {
        let mut len = 0;
        let mut stuff = unsafe { mem::MaybeUninit::uninit().assume_init() };

        let err = unsafe { x264_encoder_headers(self.raw, &mut stuff, &mut len) };

        if err < 0 {
            Err(Error)
        } else {
            Ok(unsafe { Data::from_raw_parts(stuff, len as usize) })
        }
    }

    /// Begins flushing the encoder, to handle any delayed frames.
    ///
    /// ```rust
    /// # use x264::{Colorspace, Setup};
    /// # let encoder = Setup::default().build(Colorspace::RGB, 1920, 1080).unwrap();
    /// #
    /// let mut flush = encoder.flush();
    ///
    /// while let Some(result) = flush.next() {
    ///     if let Ok((data, picture)) = result {
    ///         // Handle data.
    ///     }
    /// }
    /// ```
    pub fn flush(self) -> Flush {
        Flush { encoder: self }
    }

    /// The width required of any input images.
    pub fn width(&self) -> i32 {
        self.params.i_width
    }
    /// The height required of any input images.
    pub fn height(&self) -> i32 {
        self.params.i_height
    }
    /// The encoding required of any input images.
    pub fn encoding(&self) -> Encoding {
        unsafe { Encoding::from_raw(self.params.i_csp) }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            x264_encoder_close(self.raw);
        }
    }
}

/// Iterate through any delayed frames.
pub struct Flush {
    encoder: Encoder,
}

impl Flush {
    /// Keeps flushing.
    pub fn next(&mut self) -> Option<Result<(Data, Picture)>> {
        let enc = self.encoder.raw;

        if unsafe { x264_encoder_delayed_frames(enc) } == 0 {
            return None;
        }

        let mut len = 0;
        let mut stuff = unsafe { mem::MaybeUninit::uninit().assume_init() };
        let mut raw = unsafe { mem::MaybeUninit::uninit().assume_init() };

        let err =
            unsafe { x264_encoder_encode(enc, &mut stuff, &mut len, ptr::null_mut(), &mut raw) };

        Some(if err < 0 {
            Err(Error)
        } else {
            Ok(unsafe {
                (
                    Data::from_raw_parts(stuff, len as usize),
                    Picture::from_raw(raw),
                )
            })
        })
    }
}
