use image::GenericImageView;
use std::env::temp_dir;
use std::time::{Duration, Instant};

use screen_capture::{CaptureConfig, ThreadedCapturer};

fn test_threaded() {
    println!("Starting default, that should be disabled.");
    let capturer = ThreadedCapturer::default();

    capturer.set_pre_callback(std::sync::Arc::new(|v| print!("Frame {v} -> ")));
    capturer.set_post_callback(std::sync::Arc::new(|v| println!("Captured {v:?}")));
    std::thread::sleep(Duration::from_millis(1000));
    println!("latest: {:?}", capturer.latest());

    println!("Switching to 5 hz now");
    capturer.set_config(CaptureConfig {
        capture: vec![],
        rate: 5.0,
    });
    std::thread::sleep(Duration::from_millis(1000));
    println!("latest: {:?}", capturer.latest());

    println!("Switching to 20 hz now");
    capturer.set_config(CaptureConfig {
        capture: vec![],
        rate: 20.0,
    });
    std::thread::sleep(Duration::from_millis(500));
    println!("Switching once in 10 seconds");
    capturer.set_config(CaptureConfig {
        capture: vec![],
        rate: 0.1,
    });
    std::thread::sleep(Duration::from_millis(5000));
    println!("Switching back to 1 second seconds");
    capturer.set_config(CaptureConfig {
        capture: vec![],
        rate: 1.0,
    });
    std::thread::sleep(Duration::from_millis(5000));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    test_threaded();

    use screen_capture::util::{read_ppm, WriteSupport};

    let mut grabber = screen_capture::capture()?;

    let res = grabber.resolution();

    println!("Capture reports resolution of: {:?}", res);
    if res.width > 1920 {
        // Use my right monitor...
        grabber.prepare_capture(0, 1920, 0, res.width - 1920, res.height)?;
    } else {
        // use left monitor only.
        grabber.prepare_capture(0, 0, 0, res.width, res.height)?;
    }

    std::thread::sleep(std::time::Duration::from_millis(1000));

    let start = Instant::now();
    grabber.capture_image();
    let duration = start.elapsed();
    println!("Capture time : {:?}", duration);

    let start = Instant::now();
    let mut res = grabber.capture_image();
    let duration = start.elapsed();
    println!("2nd capture time : {:?}", duration);

    while let Err(e) = res {
        res = grabber.capture_image();
    }

    println!("Capture tried to capture image, succes? {:?}", res);
    let img = grabber.image().expect("grab image should succeed");

    // res = grabber.capture_image();

    println!("Capture writing to temp {:?}", temp_dir());
    img.write_ppm(
        temp_dir()
            .join("grab.ppm")
            .to_str()
            .expect("path must be ok"),
    )
    .unwrap();
    img.write_bmp(
        temp_dir()
            .join("grab.bmp")
            .to_str()
            .expect("path must be ok"),
    )
    .unwrap();

    {
        let start = Instant::now();
        let img_rgba = img.to_rgba_simple();
        let duration = start.elapsed();
        println!("Time via to_rgba: {:?}", duration);
        println!("buf: {:?}", &img_rgba.as_raw()[0..20]);
        img_rgba
            .save(
                temp_dir()
                    .join("img_to_rgba.png")
                    .to_str()
                    .expect("path must be ok"),
            )
            .unwrap();
    } // 5.5ms'ish for 1080p.

    {
        let start = Instant::now();
        let img_rgba = img.to_rgba();
        let duration = start.elapsed();
        println!("Time via to_rgba_simd: {:?}", duration);
        println!("buf: {:?}", &img_rgba.as_raw()[0..20]);
        img_rgba
            .save(
                temp_dir()
                    .join("img_to_rgba_simd.png")
                    .to_str()
                    .expect("path must be ok"),
            )
            .unwrap();
    } // 4.5ms'ish for 1080p.

    {
        let img_sub = img.view(0, 0, img.width(), img.height());
        let start = Instant::now();
        let buff = img_sub.to_image();
        let duration = start.elapsed();
        println!("Time via sub and to_image: {:?}", duration);
        println!("buf: {:?}", &buff.as_raw()[0..20]);
        buff.save(
            temp_dir()
                .join("grab.png")
                .to_str()
                .expect("path must be ok"),
        )
        .unwrap();
    } // 15ms-20ms'ish for 1080p.

    {
        let start = Instant::now();
        let img_false = img.to_rgba_false();
        let duration = start.elapsed();
        println!("Time for false color image: {:?}", duration);
        let start = Instant::now();
        let img = image::DynamicImage::ImageRgba8(img_false).to_rgb8();
        let duration = start.elapsed();
        println!("Time for false color image to rgb8: {:?}", duration);
        println!("buf: {:?}", &img.as_raw()[0..20]);
        img.save(
            temp_dir()
                .join("img_false.png")
                .to_str()
                .expect("path must be ok"),
        )
        .unwrap();
    } // 5ms + 5ms 'ish for 1080p.

    {
        let start = Instant::now();
        let img_rgb = img.to_rgb();
        let duration = start.elapsed();
        println!("Time via to_rgb: {:?}", duration);
        println!("buf: {:?}", &img_rgb.as_raw()[0..20]);
        img_rgb
            .save(
                temp_dir()
                    .join("img_to_rgb.png")
                    .to_str()
                    .expect("path must be ok"),
            )
            .unwrap();
    } // 114ms'ish for 1080p.

    println!("Capture done writing");

    let read_ppm = read_ppm(
        temp_dir()
            .join("grab.ppm")
            .to_str()
            .expect("path must be ok"),
    )
    .expect("must be good");
    read_ppm
        .write_ppm(
            temp_dir()
                .join("write_read_ppm.ppm")
                .to_str()
                .expect("path must be ok"),
        )
        .unwrap();

    println!("Cloning image.");

    let start = Instant::now();
    let cloned_img = img.clone();
    let duration = start.elapsed();
    println!("Time elapsed in expensive_function() is: {:?}", duration);

    cloned_img
        .write_ppm(
            temp_dir()
                .join("cloned_img.ppm")
                .to_str()
                .expect("path must be ok"),
        )
        .unwrap();

    {
        // let buff = cloned_img.clone().to_image();
        // buff.save("/tmp/cloned_to_image.png").unwrap();
    }

    let cloned_buffer = cloned_img.data();
    let orig_buffer = img.data();
    if cloned_buffer != orig_buffer {
        println!("{:?}\n{:?}", &cloned_buffer[0..20], &orig_buffer[0..20]);
        println!("cloned_buffer: {}", cloned_buffer.len());
        println!("orig_buffer: {}", orig_buffer.len());
        panic!("data of rasterimage not equivalent to real image");
    }

    println!("Capture writing to temp.");
    cloned_img
        .write_ppm(
            temp_dir()
                .join("cloned_img_write_ppm.ppm")
                .to_str()
                .expect("path must be ok"),
        )
        .unwrap();
    cloned_img
        .write_bmp(
            temp_dir()
                .join("cloned_img_write_bmp.bmp")
                .to_str()
                .expect("path must be ok"),
        )
        .unwrap();
    println!("Capture done writing");
    println!("First pixel: {:#?}", img.pixel(0, 0));
    println!(
        "last pixel: {:#?}",
        img.pixel(img.width() - 1, img.height() - 1)
    );

    for _i in 0..2 {
        let res = grabber.capture_image();
        println!("Capture tried to capture image, succes? {:?}", res);
        let img = grabber.image().expect("should succeed");
        println!(
            "last pixel: {:#?}",
            img.pixel(img.width() - 1, img.height() - 1)
        );
    }

    Ok(())
}
