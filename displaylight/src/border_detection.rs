// Roughly the same architecture as the C++ project.
// Analyser does bisection to find the black borders.
// Then sample in each led's rectangle.

use crate::rectangle::Rectangle;
use desktop_frame::{Image, RGB};

// This bespoke bisection procedure to find the presumably single transition in a 1d search.
// This bails out if lower and upper are identical, so if the return of f at start min and max
// is identical, it will return max if f(max) was true, else it returns min.
fn bisect(f: &dyn Fn(u32) -> bool, min: u32, max: u32) -> u32 {
    let mut min = min;
    let mut max = max;
    let mut upper = f(min);
    let mut lower = f(max);
    while ((max - min) > 1) && (upper != lower) {
        upper = f(max);
        lower = f(min);
        let midpoint = (max + min) / 2;
        let center = f(midpoint);
        if center != lower {
            max = midpoint
        } else {
            min = midpoint
        }
    }

    if upper {
        return max;
    }

    min
}

pub fn find_borders(image: &dyn Image, bisections_per_side: u32) -> Rectangle {
    let mut b: Rectangle = Default::default();
    use std::cmp::{max, min};

    // No idea if this is the fastest way to write it... but it is cool with the reduce.
    let bounds = (0..bisections_per_side)
        .map(|i| {
            let mut bisection_res: [u32; 4] = [0, 0, 0, 0];
            let max_x = image.get_width() - 1;
            let max_y = image.get_height() - 1;
            let center_x = max_x / 2;
            let center_y = max_y / 2;
            let mid_x = max_x / (bisections_per_side + 1) * (i + 1);
            let mid_y = max_y / (bisections_per_side + 1) * (i + 1);

            // Perform left bound, find x_min
            bisection_res[0] = bisect(&|x| image.get_pixel(x, mid_y) == RGB::black(), 0, center_x);

            // Perform right bound, find x_max
            bisection_res[1] = bisect(
                &|x| image.get_pixel(x, mid_y) != RGB::black(),
                center_x,
                max_x,
            );

            // Perform lower bound, find y_min
            bisection_res[2] = bisect(&|y| image.get_pixel(mid_x, y) == RGB::black(), 0, center_y);

            // Perform upper bound, find y_max
            bisection_res[3] = bisect(
                &|y| image.get_pixel(mid_x, y) != RGB::black(),
                center_y,
                max_y,
            );

            // println!("Bisection res: {:?}", bisection_res);
            return bisection_res;
        })
        .reduce(|a, b| {
            [
                min(a[0], b[0]),
                max(a[1], b[1]),
                min(a[2], b[2]),
                max(a[3], b[3]),
            ]
        });

    let bounds = bounds.expect("Will always have a result.");
    b.x_min = bounds[0];
    b.x_max = bounds[1];
    b.y_min = bounds[2];
    b.y_max = bounds[3];
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop_frame::raster_image::RasterImage;
    use desktop_frame::Image;
    use std::env::temp_dir;

    fn tmp_file(name: &str) -> String {
        temp_dir()
            .join(name)
            .to_str()
            .expect("path must be ok")
            .to_owned()
    }

    #[test]
    fn test_bisect() {
        //       0     1  2  3  4  5  6  7  8  9
        let z = [0u32, 0, 0, 0, 0, 1, 1, 1, 1, 1];
        let len = z.len() as u32 - 1;
        // Bisect to find the first index that is leq 5.
        let res = bisect(&|i| z[i as usize] == 0, 0u32, len);
        assert_eq!(res, 4);

        let res = bisect(&|i| z[i as usize] != 0, 0u32, len);
        assert_eq!(res, 5);

        let rz = z.iter().rev().collect::<Vec<&u32>>();
        let res = bisect(&|i| *rz[i as usize] == 0, 0u32, len);
        assert_eq!(res, 5);
        let res = bisect(&|i| *rz[i as usize] != 0, 0u32, len);
        assert_eq!(res, 4);

        // Completely black.
        let v = [0u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        // Bias low
        let res = bisect(&|i| v[i as usize] != 0, 0u32, len);
        assert_eq!(res, 0);

        // Bias high.
        let res = bisect(&|i| v[i as usize] == 0, 0u32, len);
        assert_eq!(res, len);

        // Completely white.
        let v = [1u32, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];

        // Bias low
        let res = bisect(&|i| v[i as usize] == 0, 0u32, len);
        assert_eq!(res, 0);

        // Bias high.
        let res = bisect(&|i| v[i as usize] != 0, 0u32, len);
        assert_eq!(res, len);
    }

    #[test]
    fn test_fully_white() {
        let img = RasterImage::filled(100, 100, RGB::white());
        let b = find_borders(&img, 5);

        assert_eq!(b.x_min, 0);
        assert_eq!(b.y_min, 0);
        assert_eq!(b.x_max, 99);
        assert_eq!(b.y_max, 99);
    }

    #[test]
    fn test_free_floating_rect() {
        let mut img = RasterImage::filled(100, 100, RGB { r: 0, g: 0, b: 0 });
        img.fill_rectangle(30, 80, 20, 70, RGB::yellow());
        let mut tracked = desktop_frame::tracked_image::TrackedImage::new(Box::new(img));
        let b = find_borders(&tracked, 10);
        let mut track_results = tracked.draw_access(0.5);
        track_results.set_pixel(b.x_min, b.y_min, RGB::cyan());
        track_results.set_pixel(b.x_max, b.y_max, RGB::white());
        track_results
            .write_ppm(&tmp_file("free_floating.ppm"))
            .expect("Should succeed.");

        assert_eq!(b.x_min, 29); // last index that is black
        assert_eq!(b.y_min, 19); // last index that is black.
        assert_eq!(b.x_max, 79); // last index that is not black.
        assert_eq!(b.y_max, 69); // last index that is not black.
    }

    #[test]
    fn test_horizontal_borders() {
        let mut img = RasterImage::filled(100, 100, RGB { r: 0, g: 0, b: 0 });
        img.fill_rectangle(0, 100, 20, 70, RGB::yellow());
        let mut tracked = desktop_frame::tracked_image::TrackedImage::new(Box::new(img));
        let b = find_borders(&tracked, 10);
        let mut track_results = tracked.draw_access(0.5);
        track_results.set_pixel(b.x_min, b.y_min, RGB::cyan());
        track_results.set_pixel(b.x_max, b.y_max, RGB::white());
        track_results
            .write_ppm(&tmp_file("test_horizontal_borders.ppm"))
            .expect("Should succeed.");

        assert_eq!(b.x_min, 0); // last index that is black
        assert_eq!(b.y_min, 19); // last index that is black.
        assert_eq!(b.x_max, 99); // last index that is not black.
        assert_eq!(b.y_max, 69); // last index that is not black.
    }

    #[test]
    fn test_vertical_borders() {
        let mut img = RasterImage::filled(100, 100, RGB { r: 0, g: 0, b: 0 });
        img.fill_rectangle(30, 80, 0, 100, RGB::yellow());
        let mut tracked = desktop_frame::tracked_image::TrackedImage::new(Box::new(img));
        let b = find_borders(&tracked, 10);
        let mut track_results = tracked.draw_access(0.5);
        track_results.set_pixel(b.x_min, b.y_min, RGB::cyan());
        track_results.set_pixel(b.x_max, b.y_max, RGB::white());
        track_results
            .write_ppm(&tmp_file("test_vertical_borders.ppm"))
            .expect("Should succeed.");

        assert_eq!(b.x_min, 29); // last index that is black
        assert_eq!(b.y_min, 0); // last index that is black.
        assert_eq!(b.x_max, 79); // last index that is not black.
        assert_eq!(b.y_max, 99); // last index that is not black.
    }
}
