use std::env::temp_dir;


fn main() {

    let mut grabber = screen_capture::get_capture();

    let res = grabber.get_resolution();

    println!("Capture reports resolution of: {:?}", res);
    grabber.prepare_capture(0, 1920, 0, res.width - 1920, res.height);

    let mut res = grabber.capture_image();
    while !res {
        res = grabber.capture_image();
    }

    println!("Capture tried to capture image, succes? {}", res);
    let img = grabber.get_image();
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
        let buff = image::DynamicImage::ImageRgba8(img.to_image()).into_rgb8();
        println!("buf: {:?}", &buff.as_raw()[0..20]);
        buff.save("/tmp/grab.png").unwrap();
    }
    panic!();

    println!("Capture done writing");

    let buffer = img.get_data();
    if buffer.is_none() {
        panic!("image didn't provide any data");
    }

    let read_ppm = screen_capture::read_ppm(
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

    use std::time::{Instant};

    let start = Instant::now();
    let cloned_img = img.clone();
    let duration = start.elapsed();

    cloned_img.write_ppm(
        temp_dir()
            .join("cloned_img.ppm")
            .to_str()
            .expect("path must be ok"),
    )
    .unwrap();


    println!("Time elapsed in expensive_function() is: {:?}", duration);
    {
        let buff = cloned_img.clone().to_image();
        buff.save("/tmp/cloned_to_image.png").unwrap();
    }

    let cloned_buffer = cloned_img.get_data().expect("expect a data buffer to be present");
    let orig_buffer = img.get_data().expect("expect a data buffer to be present");
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
    println!("First pixel: {:#?}", img.get_pixel(0, 0));
    println!(
        "last pixel: {:#?}",
        img.get_pixel(img.width() - 1, img.height() - 1)
    );

    for _i in 0..2 {
        let res = grabber.capture_image();
        println!("Capture tried to capture image, succes? {}", res);
        let img = grabber.get_image();
        println!(
            "last pixel: {:#?}",
            img.get_pixel(img.width() - 1, img.height() - 1)
        );
    }
}
