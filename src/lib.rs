//! A crate to access the current image shown on the monitor.
//!
//! The entire crate is written for efficiency, on both Linux and Windows it utilises shared
//! memory functionality to avoid full copies:
//!
//!  - Using X11's [Xshm](https://en.wikipedia.org/wiki/MIT-SHM) extension for efficient retrieval on Linux.
//!  - Using Windows' [Desktop Duplication API](https://docs.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api) for efficient retrieval on Windows.
//!
//! On X11, calling [`Capture::capture_image`] while the image still exists modifies the data underneath the previously handed out image. To ensure this
//! doesn't happen, the previous images get 'poisoned' after a new call to [`Capture::capture_image`] is performed. Interacting with the old images in any way
//! results in a panic. When in doubt call [`ImageBGR::to_rgba`] immediately after [`Capture::capture_image`] and immediately
//! drop the image, keeping only the owned [`image::RgbaImage`] which one can keep around indefinitely as it owns the full content.
//!
//! On Windows, a copied image is returned, so it can be kept around indefinitely, it also means that the capture time is longer as the copy happens.
//!
//! Todo: An improvement would perhaps be to make [`Capture::capture_image`] return a reference to an image. And just panic if two calls to the capture happen.
pub mod capturer;
pub mod raster_image;
pub mod util;

pub use capturer::{CaptureConfig, CaptureSpecification, Capturer, ThreadedCapturer};

#[cfg_attr(target_os = "linux", path = "./linux/linux.rs")]
#[cfg_attr(target_os = "windows", path = "./windows/windows.rs")]
mod backend;

/// Get a new instance of the screen grabber for this platform.
pub fn capture() -> Box<dyn Capture> {
    backend::capture()
}

use crate::raster_image::RasterImageBGR;

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
#[repr(align(4))]
/// Struct to represent a single pixel in BGR(A)
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

/// Trait for something that represents an BGR image.
///
/// Both windows and linux use BGR(A), using 4 bytes per pixel, A is zero
/// on both platforms, which makes it completely translucent if converted
/// to an RGBA image.
///
/// In general, you'll want to call the [`ImageBGR::to_rgba`] method to create a standard
/// owned image you can keep around.
pub trait ImageBGR {
    /// Returns the width of the image.
    fn width(&self) -> u32;

    /// Returns the height of the image.
    fn height(&self) -> u32;

    /// Returns a specific pixel's value. The x must be less then width, y less than height.
    fn pixel(&self, x: u32, y: u32) -> BGR;

    /// Returns the raw data buffer behind this image.
    fn data(&self) -> &[BGR];

    /// False color RGBA conversion, this results in blue and red swapped, and full translucency.
    fn to_rgba_false(&self) -> image::RgbaImage {
        let data = self.data();
        let data_u8 = unsafe {
            let width = self.width() as usize;
            let height = self.height() as usize;
            assert_eq!(data.len(), width * height);
            assert_eq!(std::mem::size_of::<BGR>(), std::mem::size_of::<u8>() * 4);
            let data_u8_ptr = std::mem::transmute::<*const BGR, *const u8>(data.as_ptr());
            let len = width * height * 4;
            std::slice::from_raw_parts(data_u8_ptr, len)
        };
        image::RgbaImage::from_raw(self.width(), self.height(), data_u8.to_vec())
            .expect("must have correct dimensions")
    }

    /// Convert the the image to rgba using a for loop.
    fn to_rgba_simple(&self) -> image::RgbaImage {
        let data = self.data();
        let total_len = (self.width() * self.height() * 4) as usize;
        let mut new_data = Vec::with_capacity(total_len);
        // This minor application of unsafe to create an uninitialised vector
        // speeds things up tremendously.
        unsafe {
            new_data.set_len(total_len);
        };
        for i in 0..(self.width() * self.height()) as usize {
            let out_pos = i * 4;
            new_data[out_pos + 0] = data[i].r;
            new_data[out_pos + 1] = data[i].g;
            new_data[out_pos + 2] = data[i].b;
            new_data[out_pos + 3] = 255;
        }
        image::RgbaImage::from_raw(self.width(), self.height(), new_data)
            .expect("must have correct dimensions")
    }

    /// Convert the image to opaque rgba, using the most efficient conversion function available.
    fn to_rgba(&self) -> image::RgbaImage {
        #[cfg(all(any(target_arch = "x86_64"), target_feature = "avx2"))]
        {
            self.to_rgba_avx2()
        }

        #[cfg(not(all(any(target_arch = "x86_64"), target_feature = "avx2")))]
        {
            self.to_rgba_simple()
        }
    }

    /// An AVX2 SIMD implementation of swapping the color space in 32 byte blocks.
    #[cfg(any(doc, all(any(target_arch = "x86_64"), target_feature = "avx2")))]
    fn to_rgba_avx2(&self) -> image::RgbaImage {
        return avx2_simd_bgr_to_rgba(self.width(), self.height(), self.data());
    }

    /// Convert the image to rgb.
    fn to_rgb(&self) -> image::RgbImage {
        let data = self.data();
        let total_len = (self.width() * self.height() * 3) as usize;
        let mut new_data = Vec::with_capacity(total_len);
        // This minor application of unsafe to create an uninitialised vector
        // speeds things up tremendously.
        unsafe {
            new_data.set_len(total_len);
        };
        for i in 0..(self.width() * self.height()) as usize {
            let out_pos = i * 3;
            new_data[out_pos + 0] = data[i].r;
            new_data[out_pos + 1] = data[i].g;
            new_data[out_pos + 2] = data[i].b;
        }
        image::RgbImage::from_raw(self.width(), self.height(), new_data)
            .expect("must have correct dimensions")
    }
}

use image::{GenericImageView, Pixel, Rgba};

impl GenericImageView for Box<dyn ImageBGR> {
    type Pixel = Rgba<u8>;
    fn dimensions(&self) -> (u32, u32) {
        let img = &(**self);
        (ImageBGR::width(img), ImageBGR::height(img))
    }
    fn get_pixel(&self, x: u32, y: u32) -> <Self as GenericImageView>::Pixel {
        let bgr = self.pixel(x, y);
        *Self::Pixel::from_slice(&[bgr.r, bgr.g, bgr.b, 255])
    }
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
    fn prepare_capture(&mut self, display: u32, x: u32, y: u32, width: u32, height: u32) -> bool {
        let _ = (display, x, y, width, height);
        false
    }
}

#[cfg(any(doc, all(any(target_arch = "x86_64"), target_feature = "avx2")))]
fn avx2_simd_bgr_to_rgba(width: u32, height: u32, data: &[BGR]) -> image::RgbaImage {
    use std::arch::x86_64::*;
    const DO_PRINTS: bool = false;

    #[allow(unused_macros)]
    /// Helper print macro that can be enabled or disabled.
    macro_rules! trace {
        () => (if DO_PRINTS {println!("\n");});
        ($($arg:tt)*) => {
            if DO_PRINTS {
                println!($($arg)*);
            }
        }
    }
    #[allow(dead_code)]
    /// Print a vector of m256 type.
    unsafe fn pl(input: &__m256i) -> String {
        let v: [u8; 32] = [0; 32];
        _mm256_storeu_si256(v.as_ptr() as *mut _, *input);
        format!(
            "{} | {}",
            format!("{:02X?}", &v[0..16]),
            format!("{:02X?}", &v[16..])
        )
    }

    let new_data = unsafe {
        let data_ptr = std::mem::transmute::<*const BGR, *const u8>(data.as_ptr());
        let pixels = (width * height) as usize;
        let total_len = pixels * 4;
        let mut output: Vec<u8> = Vec::with_capacity(total_len);
        output.set_len(total_len);
        let output_ptr = output.as_mut_ptr();
        // 256  / 8 = 32 bytes, 32 / 4 = 8 blocks of BGRA fit into a vector.
        const STEP_SIZE: usize = 256 / 8;
        let chunks = total_len / STEP_SIZE;
        trace!("Chunks: {chunks}");

        let alpha_mask = _mm256_set1_epi32(i32::from_ne_bytes(0xFF000000u32.to_ne_bytes()));
        trace!(" {}", pl(&alpha_mask));
        // Okay, now we need a shuffle to swap the color channels.
        let mask = _mm256_set_epi64x(
            i64::from_ne_bytes(0x00_0c_0d_0e__00_08_09_0a_u64.to_ne_bytes()),
            i64::from_ne_bytes(0x00_04_05_06__00_00_01_02_u64.to_ne_bytes()),
            i64::from_ne_bytes(0x00_0c_0d_0e__00_08_09_0a_u64.to_ne_bytes()),
            i64::from_ne_bytes(0x00_04_05_06__00_00_01_02_u64.to_ne_bytes()),
        );
        // Handle the full chunks.
        for step in 0..chunks {
            let pos = STEP_SIZE * step;
            trace!("step: {step}, pos {pos}");
            // Load the data
            let v = _mm256_loadu_si256(std::mem::transmute::<_, *const __m256i>(
                data_ptr.offset(pos as isize),
            ));
            trace!(" {}", pl(&v));

            // Shuffle, per 128bit lane.
            let shuffled = _mm256_shuffle_epi8(v, mask);
            trace!(" {}", pl(&shuffled));

            // or that with the alpha mask to make it opaque.
            let combined = _mm256_or_si256(shuffled, alpha_mask);
            trace!(" {}", pl(&combined));

            // Write back the finished data.
            _mm256_storeu_si256(
                std::mem::transmute::<_, *mut __m256i>(output_ptr.offset(pos as isize)),
                combined,
            );
        }

        // Handle any remaining pixels manually.
        for p in (chunks * STEP_SIZE..total_len).step_by(4) {
            trace!("p: {p}");
            output[p] = data[p / 4].r;
            output[p + 1] = data[p / 4].g;
            output[p + 2] = data[p / 4].b;
            output[p + 3] = 255;
        }
        trace!("output: {output:?}");

        output
    };
    image::RgbaImage::from_raw(width, height, new_data).expect("must have correct dimensions")
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

    #[test]
    #[cfg(any(doc, all(any(target_arch = "x86_64"), target_feature = "avx2")))]
    fn test_rgb_simd() {
        // fn avx2_simd_bgr_to_rgba(width: u32, height: u32, data: &[BGR]) -> image::RgbaImage {
        use crate::util::WriteSupport;
        let mut img = RasterImageBGR::filled(45, 1, BGR { r: 0, g: 0, b: 0 });
        img.set_gradient(0, 45, 0, 1);
        img.data_rgb_mut(0, 0).b = 1;
        img.write_bmp(
            std::env::temp_dir()
                .join("simd_gradient.bmp")
                .to_str()
                .expect("path must be ok"),
        )
        .unwrap();
        let img_rgba_simd = avx2_simd_bgr_to_rgba(img.width(), img.height(), img.data());
        img_rgba_simd.save("/tmp/img_rgba_simd.png").unwrap();

        for y in 0..img.height() {
            for x in 0..img.width() {
                use image::Pixel;
                let orig = img.pixel(x, y);
                let new_pixel = img_rgba_simd.get_pixel(x, y);
                assert_eq!(orig.r, new_pixel.channels()[0]);
                assert_eq!(orig.g, new_pixel.channels()[1]);
                assert_eq!(orig.b, new_pixel.channels()[2]);
            }
        }
    }
}
