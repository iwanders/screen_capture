use desktop_frame;
fn main() {
    let mut grabber = desktop_frame::get_grabber();

    let res = grabber.capture_image();
    println!("Hello, world! {}", res);
    let img = grabber.get_image();
    img.write_pnm("/tmp/foo.pnm").unwrap();

    let z = img.clone();
    z.write_pnm("/tmp/z.pnm").unwrap();
    println!("First pixel: {:#?}", img.get_pixel(0, 0));
}