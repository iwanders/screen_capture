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

/// Configuration struct, specifying all the configurable properties of the displaylight struct..
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Config {
    /// A list of capture specifications, the first one to match will be used.
    pub capture: Vec<CaptureSpecification>,
}

/// Helper struct to use the capture object to grab according to configuration.
pub struct ConfiguredCapture {
    pub config: Config,
    pub grabber: Box<dyn Capture>,
    pub cached_resolution: Option<Resolution>,
}

impl ConfiguredCapture {
    /// Instantiate a new capture grabber with configuration.
    pub fn new(config: Config) -> ConfiguredCapture {
        ConfiguredCapture {
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
            let config = get_config(width, height, &self.config.capture);

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