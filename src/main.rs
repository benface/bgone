use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;

use bgone::{
    background::detect_background_color,
    color::{parse_foreground_spec, parse_hex_color, Color, ForegroundColorSpec},
    deduce::deduce_unknown_colors,
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

    /// Foreground colors in hex format (e.g., f00, ff0000, #ff0000) or 'auto' for unknown
    /// Multiple colors can be specified for color unmixing
    /// Use 'auto' to let the tool deduce unknown colors (e.g., --fg ff0000 auto auto)
    #[arg(long = "fg", required = true, num_args = 1.., value_name = "COLOR")]
    foreground_colors: Vec<String>,

    /// Background color in hex format (e.g., fff, ffffff, #ffffff)
    /// If not specified, the background color will be auto-detected
    #[arg(long = "bg", value_name = "COLOR")]
    background_color: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Parse foreground color specifications
    let foreground_specs = parse_foreground_specs(&args.foreground_colors)?;

    // Determine background color
    let background_color = determine_background_color(&args)?;

    // Check if we have any unknown colors to deduce
    let has_unknowns = foreground_specs
        .iter()
        .any(|spec| matches!(spec, ForegroundColorSpec::Unknown));

    let foreground_colors = if has_unknowns {
        // Load the image for color deduction
        let img = image::open(&args.input)
            .with_context(|| format!("Failed to open input image: {}", args.input.display()))?;

        deduce_unknown_colors(&img, &foreground_specs, background_color)?
    } else {
        // All colors are known, just extract them
        foreground_specs
            .iter()
            .map(|spec| match spec {
                ForegroundColorSpec::Known(color) => Ok(*color),
                ForegroundColorSpec::Unknown => unreachable!("No unknowns should be present"),
            })
            .collect::<Result<Vec<_>>>()?
    };

    // Process the image
    process_image(
        &args.input,
        &args.output,
        foreground_colors,
        background_color,
    )?;

    Ok(())
}

/// Parse and validate foreground color specifications from command line arguments
fn parse_foreground_specs(color_strings: &[String]) -> Result<Vec<ForegroundColorSpec>> {
    let specs: Result<Vec<ForegroundColorSpec>> = color_strings
        .iter()
        .enumerate()
        .map(|(i, spec_str)| {
            parse_foreground_spec(spec_str).with_context(|| {
                format!(
                    "Invalid foreground color specification #{}: {}",
                    i + 1,
                    spec_str
                )
            })
        })
        .collect();

    let specs = specs?;

    if specs.is_empty() {
        anyhow::bail!("At least one foreground color must be specified");
    }

    Ok(specs)
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
