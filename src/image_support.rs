use crate::{BGR, ImageBGR};
use image::{GenericImageView, PixelWithColorType, ExtendedColorType, Pixel, Rgba};


impl GenericImageView for dyn ImageBGR {
    type Pixel = Rgba<u8>;
    fn dimensions(&self) -> (u32, u32) {
        (self.width(), self.height())
    }
    fn get_pixel(&self, x: u32, y: u32) -> <Self as GenericImageView>::Pixel {
        let bgr = self.pixel(x, y);
        *Self::Pixel::from_slice(&[bgr.r, bgr.g, bgr.b, 255])
    }
}
