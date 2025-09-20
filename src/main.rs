use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;

use bgone::{
    background::detect_background_color,
    color::{parse_hex_color, Color},
    process_image,
};

#[derive(Parser, Debug)]
#[command(
    name = "bgone",
    about = "Ultra-fast CLI tool for removing solid background colors from images",
    version,
    author
)]
struct Args {
    /// Input image path
    input: PathBuf,

    /// Output image path
    output: PathBuf,

    /// Foreground colors in hex format (e.g., f00, ff0000, #ff0000)
    /// Multiple colors can be specified for color unmixing
    #[arg(long = "fg", required = true, num_args = 1.., value_name = "COLOR")]
    foreground_colors: Vec<String>,

    /// Background color in hex format (e.g., fff, ffffff, #ffffff)
    /// If not specified, the background color will be auto-detected
    #[arg(long = "bg", value_name = "COLOR")]
    background_color: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Parse foreground colors
    let foreground_colors = parse_foreground_colors(&args.foreground_colors)?;

    // Determine background color
    let background_color = determine_background_color(&args)?;

    // Process the image
    process_image(
        &args.input,
        &args.output,
        foreground_colors,
        background_color,
    )?;

    Ok(())
}

/// Parse and validate foreground colors from command line arguments
fn parse_foreground_colors(color_strings: &[String]) -> Result<Vec<Color>> {
    let colors: Result<Vec<Color>> = color_strings
        .iter()
        .enumerate()
        .map(|(i, color_str)| {
            parse_hex_color(color_str)
                .with_context(|| format!("Invalid foreground color #{}: {}", i + 1, color_str))
        })
        .collect();

    let colors = colors?;

    if colors.is_empty() {
        anyhow::bail!("At least one foreground color must be specified");
    }

    Ok(colors)
}

/// Determine background color either from user input or auto-detection
fn determine_background_color(args: &Args) -> Result<Color> {
    if let Some(bg_str) = &args.background_color {
        parse_hex_color(bg_str).context("Invalid background color")
    } else {
        // Auto-detect background color
        let detect_progress = ProgressBar::new_spinner();
        detect_progress.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} Auto-detecting background color...")
                .expect("Failed to create progress bar style"),
        );
        detect_progress.enable_steady_tick(std::time::Duration::from_millis(100));

        let img = image::open(&args.input)
            .with_context(|| format!("Failed to open input image: {}", args.input.display()))?;

        let detected = detect_background_color(&img);

        detect_progress.finish_and_clear();
        println!(
            "✓ Auto-detected background color: #{:02x}{:02x}{:02x}",
            detected[0], detected[1], detected[2]
        );

        Ok(detected)
    }
}
