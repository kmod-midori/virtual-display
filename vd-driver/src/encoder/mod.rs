use anyhow::Result;

pub mod mft;
pub mod x264;

pub trait Encoder {
    /// Get a free surface to write a raw frame into.
    ///
    /// Returns the index of the surface and a mutable reference to the buffer of Y and UV planes.
    fn get_free_surface(&mut self) -> Option<(usize, &mut [u8], &mut [u8])>;

    /// The stride of raw frame surfaces.
    fn stride(&self) -> usize;

    /// Encode a frame.
    fn encode_frame(&mut self, surface_index: usize, force_keyframe: bool)
        -> Result<Option<&[u8]>>;
}
