use std::env::temp_dir;
use std::time::{Instant};

fn main() {
    use screen_capture::util::{WriteSupport, read_ppm};

    let mut grabber = screen_capture::capture();

    let res = grabber.resolution();

    println!("Capture reports resolution of: {:?}", res);
    if res.width > 1920 {
        // Use my right monitor...
        grabber.prepare_capture(0, 1920, 0, res.width - 1920, res.height);
    } else {
        // use left monitor only.
        grabber.prepare_capture(0, 0, 0, res.width, res.height);
    }

    let mut res = grabber.capture_image();
    while !res {
        res = grabber.capture_image();
    }

    println!("Capture tried to capture image, succes? {}", res);
    let img = grabber.image().expect("grab image should succeed");
    println!("Capture writing to temp {:?}", temp_dir());
    img.write_ppm(
        temp_dir()
            .join("grab.ppm")
            .to_str()
            .expect("path must be ok"),
    )
    .unwrap();
    img.write_bmp(temp_dir().join("grab.bmp").to_str().expect("path must be ok"))
        .unwrap();

    {
        use image::GenericImageView;
        let img_sub = img.view(0,0, img.width(), img.height());
        let start = Instant::now();
        let buff = img_sub.to_image();
        let duration = start.elapsed();
        println!("Time to go to buffer: {:?}", duration);
        println!("buf: {:?}", &buff.as_raw()[0..20]);
        buff.save("/tmp/grab.png").unwrap();
    } // 15ms-20ms'ish for 1080p.

    println!("Capture done writing");

    let buffer = img.data();


    let read_ppm = read_ppm(
        temp_dir()
            .join("grab.ppm")
            .to_str()
            .expect("path must be ok"),
    )
    .expect("must be good");
    read_ppm.write_ppm(
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

    cloned_img.write_ppm(
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
    cloned_img.write_ppm(temp_dir().join("cloned_img_write_ppm.ppm").to_str().expect("path must be ok"))
        .unwrap();
    cloned_img.write_bmp(temp_dir().join("cloned_img_write_bmp.bmp").to_str().expect("path must be ok"))
        .unwrap();
    println!("Capture done writing");
    println!("First pixel: {:#?}", img.pixel(0, 0));
    println!(
        "last pixel: {:#?}",
        img.pixel(img.width() - 1, img.height() - 1)
    );

    for _i in 0..2 {
        let res = grabber.capture_image();
        println!("Capture tried to capture image, succes? {}", res);
        let img = grabber.image().expect("should succeed");
        println!(
            "last pixel: {:#?}",
            img.pixel(img.width() - 1, img.height() - 1)
        );
    }
}
