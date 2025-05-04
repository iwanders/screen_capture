use crate::BGR;

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
    format!("{:02X?} | {:02X?}", &v[0..16], &v[16..])
}

/// An SIMD based avx2 implementation to convert BGR structs into RgbaImage.
///
/// This only works with avx2 instructions, BGR must be aligned on 4 byte boundaries (unused alpha byte).
/// A load is issued to put 8 BGR pixels from the source into an SIMD vector.
/// Then a single shuffle operation is performed to swap the channels appropriately.
/// The alpha channel is bitwise OR'd to ensure the data is opaque
/// A store is executed to move the corrected 32 bytes to the destination image.
pub fn avx2_simd_bgr_to_rgba(width: u32, height: u32, data: &[BGR]) -> image::RgbaImage {
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
            i64::from_ne_bytes(0x00_0c_0d_0e_00_08_09_0a_u64.to_ne_bytes()),
            i64::from_ne_bytes(0x00_04_05_06_00_00_01_02_u64.to_ne_bytes()),
            i64::from_ne_bytes(0x00_0c_0d_0e_00_08_09_0a_u64.to_ne_bytes()),
            i64::from_ne_bytes(0x00_04_05_06_00_00_01_02_u64.to_ne_bytes()),
        );
        // Handle the full chunks.
        for step in 0..chunks {
            let pos = STEP_SIZE * step;
            trace!("step: {step}, pos {pos}");
            // Load the data
            let v = _mm256_loadu_si256(std::mem::transmute::<*const u8, *const __m256i>(
                data_ptr.add(pos),
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
                std::mem::transmute::<*const u8, *mut __m256i>(output_ptr.add(pos)),
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
    use crate::{ImageBGR, RasterImageBGR};

    #[test]
    fn test_rgb_simd() {
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
