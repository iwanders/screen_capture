//! A crate to access the current image shown on the monitor.
//!  - Using X11's [Xshm](https://en.wikipedia.org/wiki/MIT-SHM) extension for efficient retrieval on Linux.
//!  - Using Windows' [Desktop Duplication API](https://docs.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api) for efficient retrieval on Windows.
pub mod raster_image;
pub mod util;
mod image_support;
pub use image_support::*;


#[cfg_attr(target_os = "linux", path = "./linux/linux.rs")]
#[cfg_attr(target_os = "windows", path = "./windows/windows.rs")]
mod backend;

/// Get a new instance of the desktop frame grabber for this platform.
pub fn capture() -> Box<dyn Capture> {
    backend::capture()
}

use crate::raster_image::RasterImageBGR;

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
#[repr(align(4))]
/// Struct to represent a single pixel.
pub struct BGR {
    pub b: u8,
    pub g: u8,
    pub r: u8,
}

impl BGR {
    pub fn from_i32(v: i32) -> Self {
        // Checked godbolt, this evaporates to a single 'mov' and 'and' instruction.
        BGR {
            r: ((v >> 16) & 0xFF) as u8,
            g: ((v >> 8) & 0xFF) as u8,
            b: (v & 0xFF) as u8,
        }
    }
}

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
/// Struct to represent the resolution.
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

/// Trait for something that represents an image.
pub trait ImageBGR {
    /// Returns the width of the image.
    fn width(&self) -> u32;

    /// Returns the height of the image.
    fn height(&self) -> u32;

    /// Returns a specific pixel's value. The x must be less then width, y less than height.
    fn pixel(&self, x: u32, y: u32) -> BGR;

    /// Returns the raw data buffer behind this image.
    fn data(&self) -> &[BGR];
}



// Implementation for cloning a boxed image, this always makes a true copy to a raster image.
impl Clone for Box<dyn ImageBGR> {
    fn clone(&self) -> Self {
        return Box::new(RasterImageBGR::new(self.as_ref()));
    }
}

/// Trait to which the desktop frame grabbers adhere.
pub trait Capture {
    /// Capture the frame into an internal buffer, creating a 'snapshot'
    fn capture_image(&mut self) -> bool;

    /// Retrieve the image for access. By default this may be backed by the internal buffer
    /// created by capture_image.
    fn image(&mut self) -> Result<Box<dyn ImageBGR>, ()>;

    /// Retrieve the current full desktop resolution.
    fn resolution(&mut self) -> Resolution;

    /// Attempt to prepare capture for a subsection of the entire desktop.
    /// This is implementation defined and not guaranteed to do anything. It MUST be called before
    /// trying to capture an image, as setup may happen here.
    fn prepare_capture(
        &mut self,
        display: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> bool {
        let _ = (display, x, y, width, height);
        false
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_rgb_order() {
        // Both X11 and Windows use the following to convert from the bytes behind the pointer to
        // the actual pixel values.
        /*
        let masked = as_integer & 0x00FFFFFF;
        BGR {
            r: ((masked >> 16) & 0xFF) as u8,
            g: ((masked >> 8) & 0xFF) as u8,
            b: (masked & 0xFF) as u8,
        }*/
        // Lets make the BGR struct follow that order.
        let as_integer = 0x00112233;
        let masked = as_integer & 0x00FFFFFF;
        let p = BGR {
            r: ((masked >> 16) & 0xFF) as u8,
            g: ((masked >> 8) & 0xFF) as u8,
            b: (masked & 0xFF) as u8,
        };
        assert_eq!(p.r, 0x11);
        assert_eq!(p.g, 0x22);
        assert_eq!(p.b, 0x33);

        // So                 xxRRGGBB
        // let as_integer = 0x00112233;

        // now, we can make an integer, reinterpret cast the thing and check that.
        unsafe {
            let rgb_from_integer =
                std::mem::transmute::<*const i32, *const BGR>(&as_integer as *const i32);
            assert_eq!((*rgb_from_integer).r, 0x11);
            assert_eq!((*rgb_from_integer).g, 0x22);
            assert_eq!((*rgb_from_integer).b, 0x33);
        }
        assert_eq!(std::mem::size_of::<BGR>(), std::mem::size_of::<u32>());
    }
}
