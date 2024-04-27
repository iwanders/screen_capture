use crate::raster_image;
use crate::{ImageBGR, BGR};

/// Reads a ppm image from disk. (or rather ppms written by [`write_ppm`]).
pub fn read_ppm(filename: &str) -> Result<Box<dyn ImageBGR>, Box<dyn std::error::Error>> {
    use std::fs::File;
    let file = File::open(filename)?;
    use std::io::{BufRead, BufReader};
    let br = BufReader::new(file);
    let mut lines = br.lines();
    let width: u32;
    let height: u32;
    fn make_error(v: &str) -> Box<dyn std::error::Error> {
        Box::new(std::io::Error::new(std::io::ErrorKind::Other, v))
    }

    // First, read the type, this must be P3
    let l = lines
        .next()
        .ok_or_else(|| make_error("Not enough lines"))??;
    if l != "P3" {
        return Err(make_error("Input format not supported."));
    }

    // This is where we get the resolution.
    let l = lines
        .next()
        .ok_or_else(|| make_error("Not enough lines"))??;
    let mut values = l.trim().split(' ').map(|x| str::parse::<u32>(x));
    width = values
        .next()
        .ok_or_else(|| make_error("Could not parse width."))??;
    height = values
        .next()
        .ok_or_else(|| make_error("Could not parse height."))??;

    // And check the scaling.
    let l = lines
        .next()
        .ok_or_else(|| make_error("Not enough lines"))??;
    if l != "255" {
        return Err(make_error("Scaling not supported, only 255 supported"));
    }

    let mut img: Vec<Vec<BGR>> = Default::default();
    img.resize(height as usize, vec![]);

    // Now, we iterate over the remaining lines, each holds a row for the image.
    for (li, l) in lines.enumerate() {
        let l = l?;
        // Allocate this row.
        img[li].resize(width as usize, Default::default());
        // Finally, parse the row.
        // https://doc.rust-lang.org/rust-by-example/error/iter_result.html
        let split = l.trim().split(' ').map(|x| str::parse::<u32>(x));
        let numbers: Result<Vec<_>, _> = split.collect();
        let numbers = numbers?;
        // Cool, now we have a bunch of numbers, verify the width.
        if numbers.len() / 3 != width as usize {
            return Err(make_error(
                format!("Width is incorrect, got {}", numbers.len() / 3).as_str(),
            ));
        }

        // Finally, we can convert the bytes.
        for i in 0..width as usize {
            let r = u8::try_from(numbers[i * 3])?;
            let g = u8::try_from(numbers[i * 3 + 1])?;
            let b = u8::try_from(numbers[i * 3 + 2])?;
            img[li][i] = BGR { r, g, b };
        }
    }

    Ok(Box::new(raster_image::RasterImageBGR::from_2d_vec(&img)))
}



/// Dump a ppm file to disk.
pub fn write_ppm(img: &dyn ImageBGR, filename: &str) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::prelude::*;
    let mut file = File::create(filename)?;
    file.write_all(b"P3\n")?;
    let width = img.width();
    let height = img.height();
    file.write_all(format!("{} {}\n", width, height).as_ref())?;
    file.write_all(b"255\n")?;
    for y in 0..height {
        let mut v: String = Default::default();
        v.reserve(4 * 3 * width as usize);
        for x in 0..width {
            let color = img.pixel(x, y);
            use std::fmt::Write;
            write!(v, "{} {} {} ", color.r, color.g, color.b).unwrap();
        }
        file.write_all(v.as_ref())?;
        file.write_all(b"\n")?;
    }
    Ok(())
}

/// Dump a bmp file to disk, mostly because windows can't open ppm.
pub fn write_bmp(img: &dyn ImageBGR, filename: &str) -> std::io::Result<()> {
    // Adopted from https://stackoverflow.com/a/62946358
    use std::fs::File;
    use std::io::prelude::*;
    let mut file = File::create(filename)?;
    let width = img.width();
    let height = img.height();
    let pad = (((width as i32) * -3) & 3) as u32;
    let total = 54 + 3 * width * height + pad * height;
    let head: [u32; 7] = [total, 0, 54, 40, width, height, (24 << 16) | 1];
    let head_left = [0u32; 13 - 7];

    file.write_all(b"BM")?;
    file.write_all(
        &head
            .iter()
            .map(|x| x.to_le_bytes())
            .collect::<Vec<[u8; 4]>>()
            .concat(),
    )?;
    file.write_all(
        &head_left
            .iter()
            .map(|x| x.to_le_bytes())
            .collect::<Vec<[u8; 4]>>()
            .concat(),
    )?;
    // And now, we go into writing rows.
    let mut row: Vec<u8> = Default::default();
    row.resize((width * 3 + pad) as usize, 0);
    for y in 0..height {
        // populate the row
        for x in 0..width {
            let color = img.pixel(x, height - y - 1);
            row[(x * 3) as usize] = color.b;
            row[(x * 3 + 1) as usize] = color.g;
            row[(x * 3 + 2) as usize] = color.r;
        }
        // And write the row.
        file.write_all(&row)?;
    }
    Ok(())
}

pub trait WriteSupport {
    fn write_ppm(&self, filename: &str) -> std::io::Result<()>;
    fn write_bmp(&self, filename: &str) -> std::io::Result<()>;
}
impl WriteSupport for dyn ImageBGR {
    fn write_ppm(&self, filename: &str) -> std::io::Result<()>{
        write_ppm(self, filename)
    }
    fn write_bmp(&self, filename: &str) -> std::io::Result<()>{
        write_bmp(self, filename)
    }
}

impl WriteSupport for crate::raster_image::RasterImageBGR {
    fn write_ppm(&self, filename: &str) -> std::io::Result<()>{
        write_ppm(self, filename)
    }
    fn write_bmp(&self, filename: &str) -> std::io::Result<()>{
        write_bmp(self, filename)
    }
}
