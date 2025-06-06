//! Raster image, an image owning all pixels that are in it.
use crate::*;

/// Raster image, an image owning all pixels that are in it.
#[derive(Default)]
pub struct RasterImageBGR {
    width: u32,
    height: u32,
    data: Vec<BGR>,
}

impl RasterImageBGR {
    fn index(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }

    pub fn data_rgb(&self, x: u32, y: u32) -> &BGR {
        &self.data[self.index(x, y)]
    }
    pub fn data_rgb_mut(&mut self, x: u32, y: u32) -> &mut BGR {
        let index = self.index(x, y);
        &mut self.data[index]
    }

    /// Create a raster image by copying the provided image into the internal storage.
    pub fn new(img: &dyn ImageBGR) -> RasterImageBGR {
        let width = img.width();
        let height = img.height();

        // The fastest copy ever.

        RasterImageBGR {
            width,
            height,
            data: img.data().to_vec(),
        }
    }

    /// Create a new raster image of specified width and height, filled with the provided color.
    pub fn filled(width: u32, height: u32, color: BGR) -> RasterImageBGR {
        let mut res: RasterImageBGR = RasterImageBGR {
            width,
            height,
            data: vec![Default::default(); height as usize * width as usize],
        };
        for y in 0..height {
            for x in 0..width {
                *res.data_rgb_mut(x, y) = color;
            }
        }
        res
    }

    /// Fill a rectangle with a certain color.
    pub fn fill_rectangle(&mut self, x_min: u32, x_max: u32, y_min: u32, y_max: u32, color: BGR) {
        for y in y_min..y_max {
            for x in x_min..x_max {
                self.set_pixel(x, y, color);
            }
        }
    }

    /// Create a raster image from the provided two dimension vector of pixels.
    pub fn from_2d_vec(data: &[Vec<BGR>]) -> RasterImageBGR {
        let height = data.len() as u32;
        let width = data
            .first()
            .expect("image should have at least one row")
            .len() as u32;
        let mut res: RasterImageBGR = RasterImageBGR {
            width,
            height,
            data: vec![Default::default(); height as usize * width as usize],
        };
        for y in 0..height {
            for x in 0..width {
                *res.data_rgb_mut(x, y) = data[y as usize][x as usize];
            }
        }
        res
    }

    /// Set a specific pixel to the provided color.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: BGR) {
        let width = self.width();
        let height = self.height();
        if x > width || y > height {
            panic!("Trying to set out of bounds ({}, {})", x, y);
        }
        let index = self.index(x, y);
        self.data[index] = color;
    }

    /// Fill a rectangle with a gradient.
    pub fn set_gradient(&mut self, x_min: u32, x_max: u32, y_min: u32, y_max: u32) {
        let r_step = 255.0 / (x_max - x_min) as f64;
        let g_step = 255.0 / (y_max - y_min) as f64;
        for y in y_min..y_max {
            for x in x_min..x_max {
                self.set_pixel(
                    x,
                    y,
                    BGR {
                        r: (((x - x_min) as f64 * r_step) as u32 % 256) as u8,
                        g: (((y - y_min) as f64 * g_step) as u32 % 256) as u8,
                        b: 255 - (((x - x_min) as f64 * r_step) as u32 % 256) as u8,
                    },
                );
            }
        }
    }

    /// Multiply each value in the image with a float.
    pub fn scalar_multiply(&mut self, f: f32) {
        for y in 0..self.height() {
            for x in 0..self.width() {
                let old = self.pixel(x, y);
                let new = BGR {
                    r: (old.r as f32 * f) as u8,
                    g: (old.g as f32 * f) as u8,
                    b: (old.b as f32 * f) as u8,
                };
                self.set_pixel(x, y, new);
            }
        }
    }
}

impl ImageBGR for RasterImageBGR {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn pixel(&self, x: u32, y: u32) -> BGR {
        *self.data_rgb(x, y)
    }

    fn data(&self) -> &[BGR] {
        &self.data
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::util::*;
    use std::env::temp_dir;

    #[test]
    fn test_draw_gradient() {
        let mut img = RasterImageBGR::filled(100, 100, BGR { r: 0, g: 0, b: 0 });
        img.set_gradient(10, 90, 20, 80);
        img.write_bmp(
            temp_dir()
                .join("gradient.bmp")
                .to_str()
                .expect("path must be ok"),
        )
        .unwrap();

        println!("rgb sizeof: {}", std::mem::size_of::<BGR>());
    }
}
