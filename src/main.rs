use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};

use bgone::{
    background::detect_background_color,
    color::{Color, ForegroundColorSpec, parse_foreground_spec, parse_hex_color},
    deduce::deduce_unknown_colors,
    process_image, unmix,
};

#[derive(Parser, Debug)]
#[command(
    name = "bgone",
    about = "Ultra-fast CLI tool for removing solid background colors from images",
    version,
    disable_version_flag = true
)]
struct Args {
    /// Input image path
    input: PathBuf,

    /// Output image path (optional, defaults to input file with -bgone suffix)
    output: Option<PathBuf>,

    /// Foreground colors in hex format (e.g., f00, ff0000, #ff0000) or 'auto' for unknown.
    /// Multiple colors can be specified for color unmixing.
    /// Use 'auto' to let the tool deduce unknown colors (e.g., --fg ff0000 auto auto).
    /// In non-strict mode, this is optional.
    #[arg(short = 'f', long = "fg", num_args = 1.., value_name = "COLOR")]
    foreground_colors: Vec<String>,

    /// Background color in hex format (e.g., fff, ffffff, #ffffff).
    /// If not specified, the background color will be auto-detected.
    #[arg(short = 'b', long = "bg", value_name = "COLOR")]
    background_color: Option<String>,

    /// Strict mode: requires --fg and restricts unmixing to specified colors only.
    /// Without this flag, the tool can use any color for reconstruction.
    #[arg(short = 's', long = "strict")]
    strict: bool,

    /// Color similarity threshold (0.0-1.0).
    /// In non-strict mode with --fg: pixels within this threshold of a foreground color will use that color.
    /// In strict mode with 'auto': colors within this threshold are considered similar during deduction.
    /// Default: 0.05 (5%)
    #[arg(short = 't', long = "threshold", value_name = "FLOAT")]
    threshold: Option<f64>,

    /// Print version
    #[arg(short = 'v', short_alias = 'V', long = "version", action = clap::ArgAction::Version)]
    version: (),
}

fn main() -> Result<()> {
    let args = Args::parse();

    // In strict mode, foreground colors are required
    if args.strict && args.foreground_colors.is_empty() {
        anyhow::bail!("In strict mode, at least one foreground color must be specified with --fg");
    }

    // Determine output path
    let output_path = determine_output_path(&args.input, args.output.as_deref())?;

    // Parse foreground color specifications (if any)
    let foreground_specs = if args.foreground_colors.is_empty() {
        Vec::new()
    } else {
        parse_foreground_specs(&args.foreground_colors)?
    };

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

        // Use threshold for color deduction if provided, otherwise use default
        let deduction_threshold = args
            .threshold
            .unwrap_or(unmix::DEFAULT_COLOR_CLOSENESS_THRESHOLD);
        deduce_unknown_colors(
            &img,
            &foreground_specs,
            background_color,
            deduction_threshold,
        )?
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

    // Validate threshold if provided
    if let Some(threshold) = args.threshold
        && (!(0.0..=1.0).contains(&threshold))
    {
        anyhow::bail!("Threshold must be between 0.0 and 1.0, got: {}", threshold);
    }

    // Process the image
    process_image(
        &args.input,
        &output_path,
        foreground_colors,
        background_color,
        args.strict,
        args.threshold,
    )?;

    Ok(())
}

/// Determine the output path for the processed image
///
/// If output is provided, use it as-is.
/// If output is None, generate a filename based on the input with a -bgone suffix.
/// If that file already exists, append -1, -2, etc. until we find an available filename.
fn determine_output_path(input: &Path, output: Option<&Path>) -> Result<PathBuf> {
    if let Some(output) = output {
        return Ok(output.to_path_buf());
    }

    // Generate output filename from input
    let input_stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .context("Invalid input filename")?;

    let input_ext = input.extension().and_then(|s| s.to_str()).unwrap_or("png");

    let parent = input.parent().unwrap_or_else(|| Path::new("."));

    // Try base name first
    let base_output = parent.join(format!("{}-bgone.{}", input_stem, input_ext));
    if !base_output.exists() {
        return Ok(base_output);
    }

    // If base name exists, try with incrementing numbers
    for i in 1..1000 {
        let numbered_output = parent.join(format!("{}-bgone-{}.{}", input_stem, i, input_ext));
        if !numbered_output.exists() {
            return Ok(numbered_output);
        }
    }

    anyhow::bail!("Could not generate unique output filename (tried up to -bgone-999)")
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

    specs
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_determine_output_path_explicit() {
        let input = Path::new("/some/path/input.png");
        let output = Path::new("/other/path/output.png");

        let result = determine_output_path(input, Some(output)).unwrap();
        assert_eq!(result, output);
    }

    #[test]
    fn test_determine_output_path_auto_base() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("test.png");

        // Create input file
        fs::write(&input_path, b"fake image data").unwrap();

        let result = determine_output_path(&input_path, None).unwrap();
        assert_eq!(result, temp_dir.path().join("test-bgone.png"));
    }

    #[test]
    fn test_determine_output_path_auto_incremental() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("test.png");

        // Create input file and first output file
        fs::write(&input_path, b"fake image data").unwrap();
        fs::write(temp_dir.path().join("test-bgone.png"), b"existing").unwrap();

        let result = determine_output_path(&input_path, None).unwrap();
        assert_eq!(result, temp_dir.path().join("test-bgone-1.png"));
    }

    #[test]
    fn test_determine_output_path_auto_multiple_increments() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("test.png");

        // Create input file and multiple output files
        fs::write(&input_path, b"fake image data").unwrap();
        fs::write(temp_dir.path().join("test-bgone.png"), b"existing").unwrap();
        fs::write(temp_dir.path().join("test-bgone-1.png"), b"existing").unwrap();
        fs::write(temp_dir.path().join("test-bgone-2.png"), b"existing").unwrap();

        let result = determine_output_path(&input_path, None).unwrap();
        assert_eq!(result, temp_dir.path().join("test-bgone-3.png"));
    }

    #[test]
    fn test_determine_output_path_preserves_extension() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("image.jpg");

        fs::write(&input_path, b"fake image data").unwrap();

        let result = determine_output_path(&input_path, None).unwrap();
        assert_eq!(result, temp_dir.path().join("image-bgone.jpg"));
    }

    #[test]
    fn test_determine_output_path_no_extension() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("image");

        fs::write(&input_path, b"fake image data").unwrap();

        let result = determine_output_path(&input_path, None).unwrap();
        assert_eq!(result, temp_dir.path().join("image-bgone.png"));
    }

    #[test]
    fn test_determine_output_path_complex_filename() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("my-image-2024.png");

        fs::write(&input_path, b"fake image data").unwrap();

        let result = determine_output_path(&input_path, None).unwrap();
        assert_eq!(result, temp_dir.path().join("my-image-2024-bgone.png"));
    }
}
