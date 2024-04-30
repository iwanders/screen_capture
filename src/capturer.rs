//! Helpers to select a configuration based on the resolution.

use serde::{Deserialize, Serialize};
use crate::{Capture, Resolution, ImageBGR};

/// Capture specification that conditionally applies.
///
/// If `match_*` is populated and matches the resolution's value it will be
/// considered to match and the capture will be setup according to the other fields.
#[derive(Debug, PartialEq, Serialize, Deserialize, Default, Copy, Clone)]
pub struct CaptureSpecification {
    /// The resolution's width to match to.
    pub match_width: Option<u32>,

    /// The resolution's height to match to.
    pub match_height: Option<u32>,

    #[serde(default)]
    /// The x offset to apply for this specification.
    pub x: u32,
    /// The y offset to apply for this specification.
    #[serde(default)]
    pub y: u32,

    /// The width to apply for this specification, set to the resolutions' width - x if zero.
    #[serde(default)]
    pub width: u32,
    /// The height to apply for this specification, set to the resolutions' height - y if zero.
    #[serde(default)]
    pub height: u32,

    /// The display to set the capture setup to.
    #[serde(default)]
    pub display: u32,
}

impl CaptureSpecification {
    /// Iterates through the specs to find the best one, augmends the missing or 0 values and returns it.
    /// See the documentation of [`CaptureSpecification`] for further information.
    pub fn get_config(width: u32, height: u32, specs: &[CaptureSpecification]) -> CaptureSpecification {
        for spec in specs.iter() {
            let mut matches = true;
            if let Some(match_width) = spec.match_width {
                matches &= match_width == width;
            }
            if let Some(match_height) = spec.match_height {
                matches &= match_height == height;
            }
            if !matches {
                continue;
            }

            // We found the best match, copy this and populate it as best we can.
            let mut populated: CaptureSpecification = *spec;
            populated.width = if populated.width == 0 {
                width - populated.x
            } else {
                populated.width
            };
            populated.height = if populated.height == 0 {
                height - populated.y
            } else {
                populated.height
            };
            return populated;
        }

        // No capture match found... well, return some sane default then.
        CaptureSpecification {
            width,
            height,
            ..Default::default()
        }
    }
}

/// Configuration struct, specifying all the configurable properties of the displaylight struct..
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CaptureConfig {
    /// A list of capture specifications, the first one to match will be used.
    pub capture: Vec<CaptureSpecification>,

    /// A rate, used only if [`ThreadedCapturer`] is used.
    pub rate: f32,
}




/// Helper struct to use the capture object to grab according to configuration.
pub struct Capturer {
    pub config: CaptureConfig,
    pub grabber: Box<dyn Capture>,
    pub cached_resolution: Option<Resolution>,
}

impl Capturer {
    /// Instantiate a new capture grabber with configuration.
    pub fn new(config: CaptureConfig) -> Capturer {
        Self {
            config,
            grabber: crate::capture(),
            cached_resolution: None,
        }
    }

    /// Update the capture configuration according to the latest resolution.
    ///
    /// Returns true if the configuration changed.
    pub fn update_resolution(&mut self) -> bool {
        // First, check if the resolution of the desktop environment has changed, if so, act.
        let current_resolution = self.grabber.resolution();
        let old_resolution = self.cached_resolution;

        if self.cached_resolution.is_none()
            || *self.cached_resolution.as_ref().unwrap() != current_resolution
        {
            let width = current_resolution.width;
            let height = current_resolution.height;

            // Resolution has changed, figure out the best match in our configurations and
            // prepare the capture accordingly.
            let config = CaptureSpecification::get_config(width, height, &self.config.capture);

            self.grabber.prepare_capture(
                config.display,
                config.x,
                config.y,
                config.width,
                config.height,
            );
            // Store the current resolution.
            self.cached_resolution = Some(current_resolution);
        }
        old_resolution != self.cached_resolution
    }

    /// Set the configuration and re-initialise appropriately.
    pub fn set_config(&mut self, config: CaptureConfig) {
        self.cached_resolution = None; // force reinitialisation.
        self.config = config;
    }

    /// Get the current config.
    pub fn config(&self) -> CaptureConfig {
        self.config.clone()
    }

    /// Update the resolution and capture a new image.
    pub fn capture(&mut self) -> Result<Box<dyn ImageBGR>, ()> {
        self.update_resolution();

        // Now, we are ready to try and get the image:
        let res = self.grabber.capture_image();

        if !res {
            return Err(());
        }

        // Then, we can grab the actual image.
        self.grabber.image()
    }
}


use std::sync::mpsc::{Sender, channel};
use std::sync::atomic::{AtomicBool};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex};
pub struct ThreadedCapturer {
    thread: Option<std::thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
    latest: Arc<Mutex<(Result<Arc<image::RgbaImage>, ()>, std::time::Instant)>>,
    sender: Sender<CaptureConfig>,
    /// Pointer to the current config.
    config: Arc<Mutex<CaptureConfig>>,
}

impl Drop for ThreadedCapturer {
    fn drop(&mut self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let t = self.thread.take();
        t.unwrap().join().expect("join should succeed");
    }
}

impl ThreadedCapturer {
    /// Instantiate a new capture grabber with configuration.
    pub fn new(config: CaptureConfig) -> ThreadedCapturer {
        let running : Arc<AtomicBool> = Arc::new(true.into());
        let latest = Arc::new(Mutex::new((Err(()), std::time::Instant::now())));
        let running_t = Arc::clone(&running);
        let latest_t = Arc::clone(&latest);
        let config_initial = config.clone();
        let config = Arc::new(Mutex::new(config));
        let config_t = Arc::clone(&config);
        let (sender, receiver) = channel::<CaptureConfig>();
        let thread = std::thread::spawn(move || {
            use std::time::{Duration, Instant};
            const DEBUG_PRINT: bool = true;

            let epoch = Instant::now();
            let mut capturer =  Capturer::new(config_initial);
            let latest = latest_t;
            let config = config_t;

            let mut last_duration = std::time::Duration::new(0, 0);
            let mut last_end = Instant::now();
            while running_t.load(Relaxed) {
                // First, check for new configs, if so consume them.
                for new_config in receiver.try_iter() {
                    capturer.set_config(new_config.clone());
                    {
                        let mut locked = config.lock().unwrap();
                        *locked = new_config;
                    }
                }
                
                let rate_valid = capturer.config.rate >= 0.0;
                if !rate_valid {
                    // Rate is negative or zero, can be used to disable, block on config updates for 100ms.
                    if let Ok(new_config) = receiver.recv_timeout(Duration::from_millis(100)) {
                        capturer.set_config(new_config.clone());
                        {
                            let mut locked = config.lock().unwrap();
                            *locked = new_config;
                        }
                    }
                    continue;
                }

                // Next, calculate the desired interval and point in time to start.
                let interval = Duration::from_secs_f32(1.0 / capturer.config.rate);
                let start_timepoint = last_end + interval - last_duration;
                if DEBUG_PRINT {
                    println!("current:   {: >16.6?} start_timepoint: {: >12.6?}", Instant::now().duration_since(epoch) , start_timepoint.duration_since(epoch) );
                }
                let now = Instant::now();
                if now <= start_timepoint {
                    // Still have to wait, limit the wait to 100ms.
                    let to_wait = start_timepoint - now;
                    let limited = to_wait.min(Duration::from_millis(100));
                    if DEBUG_PRINT {
                        println!("sleeping for: {:?}", limited);
                    }
                    std::thread::sleep(limited);
                    // Quick check if we still have to wait more.
                    if Instant::now() <= start_timepoint {
                        continue;
                    }
                }

                let start = Instant::now();
                let img = capturer.capture();
                let img = img.map(|v| v.to_rgba());
                {
                    let mut locked = latest.lock().unwrap();
                    if DEBUG_PRINT {
                        println!("capture at {: >16.6?} ", start.duration_since(epoch));
                    }
                    *locked = (img.map(|v|Arc::new(v)), start);
                }
                std::thread::sleep(Duration::from_millis(100) - (std::time::Instant::now() - start));

                let end = std::time::Instant::now();
                last_duration = end - start;
                last_end = end;
                if DEBUG_PRINT {
                    println!("Duration was {: >13.6?} at {: >12.6?}", last_duration.as_secs_f64(), Instant::now().duration_since(epoch));
                }
            }
            if DEBUG_PRINT {
                println!("Broke from loop, thread closing");
            }
        });
        Self {
            config,
            running,
            latest,
            sender,
            thread: Some(thread)
        }
    }

    /// Set the configuration and re-initialise appropriately.
    pub fn set_config(&self, config: CaptureConfig) {
        let _ = self.sender.send(config);
    }

    /// Get the current config.
    pub fn config(&self) -> CaptureConfig {
        let locked = self.config.lock().unwrap();
        locked.clone()
    }

    /// Obtain the latest image.
    pub fn latest_image(&self) -> Result<Arc<image::RgbaImage>, ()> {
        let lock = self.latest.lock().unwrap();
        lock.0.clone()
    }

    /// Obtain the latest image and its capture time.
    pub fn latest(&self) -> (Result<Arc<image::RgbaImage>, ()>, std::time::Instant) {
        let lock = self.latest.lock().unwrap();
        lock.clone()
    }
}
