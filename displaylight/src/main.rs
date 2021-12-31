use displaylight::{border_detection, rectangle::Rectangle, sampler, zones};
use lights;

use std::error::Error;
use std::{thread, time};
fn main() -> Result<(), Box<dyn Error>> {
    let mut grabber = desktop_frame::get_grabber();

    let resolution = grabber.get_resolution();

    println!("Grabber reports resolution of: {:?}", resolution);
    grabber.prepare_capture(1920, 0, resolution.width - 1920, resolution.height);

    let mut control = lights::Lights::new("/dev/ttyACM0")?;

    const MAX_LEDS: usize = 228;

    let mut state: Option<(Rectangle, sampler::Sampler)> = None;
    loop {
        let res = grabber.capture_image();
        if (!res) {
            continue;
        }
        // Then, grab the image.
        let img = grabber.get_image();

        // Detect the black borders
        let borders = border_detection::find_borders(&*img, 5);

        // Border size changed, make a new sampler.
        if state.is_none() || state.as_ref().unwrap().0 != borders {
            // With the edges known, we can make the zones.
            // Zones goes bad with Rectangle { x_min: 622, x_max: 1353, y_min: 574, y_max: 384 }
            let zones = zones::Zones::make_zones(&borders, 200, 200);
            // println!("zones: {:?}", zones);
            assert_eq!(zones.len(), MAX_LEDS);

            // With the zones known, we can create the sampler.
            let sampler = sampler::Sampler::make_sampler(&zones, 15);
            state = Some((borders, sampler));
        }

        let sampler = &state.as_ref().unwrap().1;
        // With the sampler, we can now sample and get color values.
        let values = sampler.sample(&*img);
        assert_eq!(values.len(), MAX_LEDS);

        // Finally, create the lights::RGB array.
        let mut leds = [lights::RGB::default(); MAX_LEDS];
        for i in 0..MAX_LEDS {
            leds[i].r = values[i].r;
            leds[i].g = values[i].g;
            leds[i].b = values[i].b;
        }
        control.set_leds(&leds)?;
        thread::sleep(time::Duration::from_millis(1000 / 60));
    }

    Ok(())
}
