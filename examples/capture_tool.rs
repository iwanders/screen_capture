use image::GenericImageView;
use screen_capture::{CaptureConfig, CaptureSpecification, ThreadedCapturer};
use std::env::temp_dir;
use std::time::{Duration, Instant};

use std::path::PathBuf;

#[derive(clap::ValueEnum, Clone, Debug, Default)]
enum Area {
    #[default]
    Full,
    Left,
    Right,
}

use chrono::prelude::*;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// The output directory to save screenshots to.
    #[arg(short, long, value_name = "OUTPUT_DIR", default_value = "/tmp/")]
    output_dir: PathBuf,

    /// Sets a custom config file
    #[arg(short, long, value_name = "CONFIG_FILE")]
    config: Option<PathBuf>,

    /// The display to capture (this is only useful on windows).
    #[arg(short, long, value_name = "DISPLAY", default_value = "0")]
    display: Option<u32>,

    /// The area to capture, this is mostly useful on linux to capture either my left or right monitor.
    #[arg(short, long, value_name = "AREA", default_value = "full")]
    area: Option<Area>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Capture a single screenshot.
    Single {
        /// Delay to wait, defaults to 0.0 seconds.
        #[arg(short, long, default_value = "0.0")]
        delay: f32,
    },
    /// Capture screenshots periodic.
    Periodic {
        /// Delay to wait, defaults to 0.0 seconds.
        #[arg(short, long, default_value = "10.0")]
        interval: f32,

        /// Limit the number of captures.
        #[arg(short, long, value_name = "LIMIT")]
        limit: Option<usize>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    let mut grabber = screen_capture::capture()?;

    let res = grabber.resolution();
    println!("Capture reports resolution of: {:?}", res);

    let display = args.display.unwrap_or_default();
    let area = args.area.unwrap_or_default();

    let (x, y, width, height) = match area {
        Area::Full => (0, 0, res.width, res.height),
        Area::Left => (0, 0, res.width / 2, res.height),
        Area::Right => (res.width / 2, 0, res.width / 2, res.height),
    };

    grabber.prepare_capture(display, x, y, width, height)?;

    if let Some(config_path) = args.config {
        let config_str = std::fs::read_to_string(&config_path)?;

        let configs: Vec<CaptureSpecification> = serde_yaml::from_str(&config_str)?;
        let config = CaptureSpecification::get_config(res.width, res.height, &configs);

        if let Err(e) = grabber.prepare_capture(
            config.display,
            config.x,
            config.y,
            config.width,
            config.height,
        ) {
            println!("Failed preparing capture {e:?}");
        };
    }

    fn make_filename() -> String {
        let utc: DateTime<Utc> = Utc::now(); // e.g. `2014-11-28T12:45:59.324310806Z`
        let name = utc.format("%Y-%m-%d__%H_%M_%S.png").to_string();
        name
    }

    let output_dir = args.output_dir;
    std::fs::create_dir_all(&output_dir)?;
    match args.command {
        Commands::Single { delay } => {
            std::thread::sleep(Duration::from_secs_f32(delay));
            grabber.capture_image()?;
            let img = grabber.image()?;
            let img_rgba = img.to_rgba();

            let output_path = output_dir.join(make_filename());
            img_rgba.save(&output_path)?;
            println!("Saved {output_path:?}");
        }
        Commands::Periodic { interval, limit } => {
            for _ in 1..limit.unwrap_or(usize::MAX) {
                let start = Instant::now();
                grabber.capture_image()?;
                let img = grabber.image()?;
                let img_rgba = img.to_rgba();
                let output_path = output_dir.join(make_filename());
                img_rgba.save(&output_path)?;
                println!("Saved {output_path:?}");
                let time_taken = (Instant::now() - start).as_secs_f32();
                let remaining_sleep = (interval - time_taken).max(0.0);
                std::thread::sleep(Duration::from_secs_f32(remaining_sleep));
            }
        }
    }

    Ok(())
}
