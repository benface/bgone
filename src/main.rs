use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use bgone::{
    ProcessOptions,
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
    /// Input image path(s). Globs like 'images/*.png' are expanded automatically
    /// (quote them on Unix to prevent shell expansion). With exactly two
    /// positionals, the second is treated as the output file. With three or more,
    /// or when --out-dir is set, all positionals are inputs.
    #[arg(required = true, num_args = 1.., value_name = "PATH")]
    paths: Vec<PathBuf>,

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

    /// Foreground color similarity threshold (0.0-1.0).
    /// In non-strict mode with --fg: pixels within this threshold of a foreground color will use that color.
    /// In strict mode with 'auto': colors within this threshold are considered similar during deduction.
    /// Default: 0.05 (5%)
    #[arg(long = "fg-threshold", value_name = "FLOAT")]
    fg_threshold: Option<f64>,

    /// Background snap threshold (0.0-1.0).
    /// Pixels within this per-channel distance (L∞) of the resolved background color
    /// are forced to fully transparent before unmixing. Useful for cleaning up JPEG
    /// artifacts or noisy backgrounds. Default: 0.0 (only exact background matches snap).
    /// Example: 0.004 ≈ 1/255 per channel, 0.02 ≈ 5/255 per channel.
    #[arg(long = "bg-threshold", value_name = "FLOAT")]
    bg_threshold: Option<f64>,

    /// Trim the output image by cropping to the bounding box of non-transparent pixels.
    #[arg(long = "trim")]
    trim: bool,

    /// Output directory for batch processing.
    /// When set, every positional is treated as an input and outputs are written
    /// into this directory (created if missing) with the auto-generated -bgone suffix.
    #[arg(long = "out-dir", value_name = "PATH")]
    out_dir: Option<PathBuf>,

    /// Print version
    #[arg(short = 'v', short_alias = 'V', long = "version", action = clap::ArgAction::Version)]
    version: (),
}

/// Resolution of the positional `paths` into a concrete batch plan.
#[derive(Debug)]
struct PathPlan {
    inputs: Vec<PathBuf>,
    /// Explicit output path. Only set in single-input mode when the user wrote
    /// `bgone input.png output.png`.
    single_output: Option<PathBuf>,
}

/// Shared per-file processing settings derived once from CLI args and reused
/// for every input image. Separating this from the per-file positionals
/// (input/output path) keeps [`process_single_file`] under the clippy
/// too-many-arguments threshold without an explicit `allow`.
struct FileProcessing<'a> {
    foreground_specs: &'a [ForegroundColorSpec],
    bg_string: Option<&'a str>,
    strict: bool,
    fg_threshold: Option<f64>,
    bg_threshold: Option<f64>,
    trim: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Validate thresholds early (before any I/O)
    if let Some(t) = args.fg_threshold
        && (!(0.0..=1.0).contains(&t))
    {
        anyhow::bail!("--fg-threshold must be between 0.0 and 1.0, got: {}", t);
    }
    if let Some(t) = args.bg_threshold
        && (!(0.0..=1.0).contains(&t))
    {
        anyhow::bail!("--bg-threshold must be between 0.0 and 1.0, got: {}", t);
    }

    // In strict mode, foreground colors are required
    if args.strict && args.foreground_colors.is_empty() {
        anyhow::bail!("In strict mode, at least one foreground color must be specified with --fg");
    }

    // Parse foreground color specifications once up front so an invalid --fg fails
    // fast (instead of N times in parallel for an N-file batch).
    let foreground_specs = if args.foreground_colors.is_empty() {
        Vec::new()
    } else {
        parse_foreground_specs(&args.foreground_colors)?
    };

    // Expand any glob patterns in the positionals (handles quoted globs and Windows cmd.exe)
    let expanded_paths = expand_globs(&args.paths)?;

    // Resolve inputs + optional single-output positional
    let plan = resolve_path_plan(&expanded_paths, args.out_dir.as_deref())?;

    if plan.inputs.is_empty() {
        anyhow::bail!("No input images were provided");
    }

    // Create out_dir up front so per-file failures aren't due to a missing dir
    if let Some(out_dir) = &args.out_dir {
        std::fs::create_dir_all(out_dir)
            .with_context(|| format!("Failed to create output directory: {}", out_dir.display()))?;
    }

    // Allocate output paths sequentially. Avoids a race where two parallel tasks
    // pick the same auto-named output when batched inputs share a filename.
    let output_paths = allocate_output_paths(
        &plan.inputs,
        plan.single_output.as_deref(),
        args.out_dir.as_deref(),
    )?;

    let file_processing = FileProcessing {
        foreground_specs: &foreground_specs,
        bg_string: args.background_color.as_deref(),
        strict: args.strict,
        fg_threshold: args.fg_threshold,
        bg_threshold: args.bg_threshold,
        trim: args.trim,
    };

    if plan.inputs.len() == 1 {
        // Single-file mode: keep the original verbose output
        process_single_file(&plan.inputs[0], &output_paths[0], &file_processing, false)?;
    } else {
        // Batch mode: parallel files, quiet per-file output, single batch progress bar
        run_batch(&plan.inputs, &output_paths, &file_processing)?;
    }

    Ok(())
}

/// Drive the batch (multi-input) path: parallel processing with a single overall
/// progress bar and a per-file success/failure summary at the end.
fn run_batch(
    inputs: &[PathBuf],
    output_paths: &[PathBuf],
    file_processing: &FileProcessing<'_>,
) -> Result<()> {
    debug_assert_eq!(inputs.len(), output_paths.len());

    let total = inputs.len();
    println!("Batch mode: processing {} files in parallel...", total);

    let overall = ProgressBar::new(total as u64);
    overall.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files")
            .expect("Failed to create progress bar style")
            .progress_chars("#>-"),
    );

    let results: Vec<(PathBuf, Result<()>)> = inputs
        .par_iter()
        .zip(output_paths.par_iter())
        .map(|(input, output)| {
            let result = process_single_file(input, output, file_processing, true);
            overall.inc(1);
            (input.clone(), result)
        })
        .collect();

    overall.finish_and_clear();

    let mut succeeded = 0usize;
    let mut failed = 0usize;
    for (input, result) in &results {
        match result {
            Ok(()) => {
                succeeded += 1;
                println!("✓ {}", input.display());
            }
            Err(e) => {
                failed += 1;
                eprintln!("✗ {}: {:#}", input.display(), e);
            }
        }
    }

    println!(
        "Done: {} succeeded, {} failed (of {} total)",
        succeeded, failed, total
    );

    if failed > 0 {
        anyhow::bail!("{} file(s) failed to process", failed);
    }

    Ok(())
}

/// Process a single input image with a pre-resolved output path and the
/// shared per-file processing settings. Background detection and unknown-color
/// deduction still happen per-file (each image may have a different background
/// or palette).
fn process_single_file(
    input: &Path,
    output_path: &Path,
    fp: &FileProcessing<'_>,
    quiet: bool,
) -> Result<()> {
    let background_color = determine_background_color(input, fp.bg_string, quiet)?;

    let has_unknowns = fp
        .foreground_specs
        .iter()
        .any(|spec| matches!(spec, ForegroundColorSpec::Unknown));

    let foreground_colors = if has_unknowns {
        let img = image::open(input)
            .with_context(|| format!("Failed to open input image: {}", input.display()))?;

        let deduction_threshold = fp
            .fg_threshold
            .unwrap_or(unmix::DEFAULT_COLOR_CLOSENESS_THRESHOLD);
        deduce_unknown_colors(
            &img,
            fp.foreground_specs,
            background_color,
            deduction_threshold,
        )?
    } else {
        fp.foreground_specs
            .iter()
            .map(|spec| match spec {
                ForegroundColorSpec::Known(color) => Ok(*color),
                ForegroundColorSpec::Unknown => unreachable!("No unknowns should be present"),
            })
            .collect::<Result<Vec<_>>>()?
    };

    process_image(
        input,
        output_path,
        foreground_colors,
        background_color,
        ProcessOptions {
            strict_mode: fp.strict,
            fg_threshold: fp.fg_threshold,
            bg_threshold: fp.bg_threshold,
            trim: fp.trim,
            quiet,
        },
    )?;

    Ok(())
}

/// Expand any positionals containing glob characters (`*`, `?`, `[`) that do
/// NOT already exist as literal paths. This lets users quote globs to bypass
/// shell expansion (`bgone 'images/*.png'`) and makes bgone work on Windows
/// cmd.exe, which doesn't expand globs.
///
/// Paths without glob characters are passed through unchanged. Literal files
/// whose names happen to contain `[` etc. are detected by the existence check
/// and not re-expanded.
fn expand_globs(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut expanded = Vec::with_capacity(paths.len());

    for raw in paths {
        let raw_str = raw.to_string_lossy();
        let looks_like_glob =
            raw_str.contains('*') || raw_str.contains('?') || raw_str.contains('[');

        if !looks_like_glob || raw.exists() {
            expanded.push(raw.clone());
            continue;
        }

        let matches: Vec<PathBuf> = glob::glob(&raw_str)
            .with_context(|| format!("Invalid glob pattern: {}", raw_str))?
            .filter_map(|entry| entry.ok())
            .filter(|p| p.is_file())
            .collect();

        if matches.is_empty() {
            anyhow::bail!("No files matched glob pattern: {}", raw_str);
        }

        expanded.extend(matches);
    }

    Ok(expanded)
}

/// Decide whether the trailing positional is an output path or another input.
///
/// Rules:
/// - `--out-dir` set → every positional is an input, no single-output.
/// - 1 positional → input only.
/// - 2 positionals → backward-compatible `input → output` form. Refuses to
///   overwrite an existing file at the output position to guard against
///   `bgone images/*.png` accidentally consuming two real images.
/// - 3+ positionals → every positional is an input.
fn resolve_path_plan(paths: &[PathBuf], out_dir: Option<&Path>) -> Result<PathPlan> {
    if out_dir.is_some() {
        return Ok(PathPlan {
            inputs: paths.to_vec(),
            single_output: None,
        });
    }

    match paths.len() {
        0 => Ok(PathPlan {
            inputs: Vec::new(),
            single_output: None,
        }),
        1 => Ok(PathPlan {
            inputs: paths.to_vec(),
            single_output: None,
        }),
        2 => {
            let candidate_output = &paths[1];
            if candidate_output.exists() {
                anyhow::bail!(
                    "Refusing to overwrite existing file: {}\n\nIf you meant to process two input images in batch mode, pass --out-dir <DIR> (or quote a glob: 'images/*.png').\nIf you really want to overwrite {}, delete it first or pick a different output name.",
                    candidate_output.display(),
                    candidate_output.display()
                );
            }
            Ok(PathPlan {
                inputs: vec![paths[0].clone()],
                single_output: Some(paths[1].clone()),
            })
        }
        _ => Ok(PathPlan {
            inputs: paths.to_vec(),
            single_output: None,
        }),
    }
}

/// Sequentially allocate an output path for each input, accounting for both
/// existing files on disk AND prior allocations made in this same batch. This
/// closes the race where two parallel tasks would otherwise both pick the same
/// auto-named output (e.g. `bgone 'a/*.png' 'b/*.png' --out-dir out/` with the
/// same filename in both source directories).
///
/// If `single_output` is set, it's used verbatim for the (only) input.
fn allocate_output_paths(
    inputs: &[PathBuf],
    single_output: Option<&Path>,
    out_dir: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    let mut used: HashSet<PathBuf> = HashSet::new();
    let mut outputs = Vec::with_capacity(inputs.len());
    for input in inputs {
        let path = determine_output_path(input, single_output, out_dir, &used)?;
        used.insert(path.clone());
        outputs.push(path);
    }
    Ok(outputs)
}

/// Determine the output path for the processed image.
///
/// Precedence: explicit `single_output` > `out_dir` (auto-named inside it) >
/// auto-named next to the input.
///
/// When auto-naming, if the target already exists on disk OR is already in
/// `used`, append `-1`, `-2`, ... until an unused name is found (giving up
/// after 999).
fn determine_output_path(
    input: &Path,
    single_output: Option<&Path>,
    out_dir: Option<&Path>,
    used: &HashSet<PathBuf>,
) -> Result<PathBuf> {
    if let Some(output) = single_output {
        return Ok(output.to_path_buf());
    }

    let input_stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .context("Invalid input filename")?;

    let input_ext = input
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "png".to_string());

    // Determine output extension: use PNG for formats that don't support alpha
    let output_ext = match input_ext.as_str() {
        "png" | "webp" | "tiff" | "tif" | "gif" | "qoi" | "exr" => input_ext,
        _ => "png".to_string(),
    };

    let parent: PathBuf = match out_dir {
        Some(dir) => dir.to_path_buf(),
        None => input
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".")),
    };

    let is_available = |candidate: &Path| !candidate.exists() && !used.contains(candidate);

    // Try base name first
    let base_output = parent.join(format!("{}-bgone.{}", input_stem, output_ext));
    if is_available(&base_output) {
        return Ok(base_output);
    }

    // Otherwise append incrementing numbers
    for i in 1..1000 {
        let numbered_output = parent.join(format!("{}-bgone-{}.{}", input_stem, i, output_ext));
        if is_available(&numbered_output) {
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
fn determine_background_color(input: &Path, bg_string: Option<&str>, quiet: bool) -> Result<Color> {
    if let Some(bg_str) = bg_string {
        return parse_hex_color(bg_str).context("Invalid background color");
    }

    let detect_progress = if quiet {
        ProgressBar::hidden()
    } else {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} Auto-detecting background color...")
                .expect("Failed to create progress bar style"),
        );
        bar.enable_steady_tick(std::time::Duration::from_millis(100));
        bar
    };

    let img = image::open(input)
        .with_context(|| format!("Failed to open input image: {}", input.display()))?;

    let detected = detect_background_color(&img);

    detect_progress.finish_and_clear();
    if !quiet {
        println!(
            "✓ Auto-detected background color: #{:02x}{:02x}{:02x}",
            detected[0], detected[1], detected[2]
        );
    }

    Ok(detected)
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

        let result = determine_output_path(input, Some(output), None, &HashSet::new()).unwrap();
        assert_eq!(result, output);
    }

    #[test]
    fn test_determine_output_path_auto_base() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("test.png");

        fs::write(&input_path, b"fake image data").unwrap();

        let result = determine_output_path(&input_path, None, None, &HashSet::new()).unwrap();
        assert_eq!(result, temp_dir.path().join("test-bgone.png"));
    }

    #[test]
    fn test_determine_output_path_auto_incremental() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("test.png");

        fs::write(&input_path, b"fake image data").unwrap();
        fs::write(temp_dir.path().join("test-bgone.png"), b"existing").unwrap();

        let result = determine_output_path(&input_path, None, None, &HashSet::new()).unwrap();
        assert_eq!(result, temp_dir.path().join("test-bgone-1.png"));
    }

    #[test]
    fn test_determine_output_path_auto_multiple_increments() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("test.png");

        fs::write(&input_path, b"fake image data").unwrap();
        fs::write(temp_dir.path().join("test-bgone.png"), b"existing").unwrap();
        fs::write(temp_dir.path().join("test-bgone-1.png"), b"existing").unwrap();
        fs::write(temp_dir.path().join("test-bgone-2.png"), b"existing").unwrap();

        let result = determine_output_path(&input_path, None, None, &HashSet::new()).unwrap();
        assert_eq!(result, temp_dir.path().join("test-bgone-3.png"));
    }

    #[test]
    fn test_determine_output_path_converts_jpeg_to_png() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("image.jpg");

        fs::write(&input_path, b"fake image data").unwrap();

        let result = determine_output_path(&input_path, None, None, &HashSet::new()).unwrap();
        assert_eq!(result, temp_dir.path().join("image-bgone.png"));
    }

    #[test]
    fn test_determine_output_path_preserves_alpha_format() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("image.webp");

        fs::write(&input_path, b"fake image data").unwrap();

        let result = determine_output_path(&input_path, None, None, &HashSet::new()).unwrap();
        assert_eq!(result, temp_dir.path().join("image-bgone.webp"));
    }

    #[test]
    fn test_determine_output_path_no_extension() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("image");

        fs::write(&input_path, b"fake image data").unwrap();

        let result = determine_output_path(&input_path, None, None, &HashSet::new()).unwrap();
        assert_eq!(result, temp_dir.path().join("image-bgone.png"));
    }

    #[test]
    fn test_determine_output_path_complex_filename() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("my-image-2024.png");

        fs::write(&input_path, b"fake image data").unwrap();

        let result = determine_output_path(&input_path, None, None, &HashSet::new()).unwrap();
        assert_eq!(result, temp_dir.path().join("my-image-2024-bgone.png"));
    }

    #[test]
    fn test_determine_output_path_uses_out_dir_when_set() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("nested/test.png");
        let out_dir = temp_dir.path().join("out");
        fs::create_dir_all(input_path.parent().unwrap()).unwrap();
        fs::create_dir_all(&out_dir).unwrap();
        fs::write(&input_path, b"fake image data").unwrap();

        let result =
            determine_output_path(&input_path, None, Some(&out_dir), &HashSet::new()).unwrap();
        assert_eq!(result, out_dir.join("test-bgone.png"));
    }

    #[test]
    fn test_determine_output_path_explicit_overrides_out_dir() {
        let explicit = Path::new("/explicit/output.png");
        let out_dir = Path::new("/some/dir");
        let input = Path::new("/in/test.png");

        let result =
            determine_output_path(input, Some(explicit), Some(out_dir), &HashSet::new()).unwrap();
        assert_eq!(result, explicit);
    }

    #[test]
    fn test_resolve_path_plan_single_input() {
        let paths = vec![PathBuf::from("a.png")];
        let plan = resolve_path_plan(&paths, None).unwrap();
        assert_eq!(plan.inputs, vec![PathBuf::from("a.png")]);
        assert_eq!(plan.single_output, None);
    }

    #[test]
    fn test_resolve_path_plan_two_inputs_output_not_existing() {
        let temp_dir = TempDir::new().unwrap();
        let input = temp_dir.path().join("a.png");
        let output = temp_dir.path().join("b.png"); // does NOT exist
        fs::write(&input, b"fake").unwrap();

        let paths = vec![input.clone(), output.clone()];
        let plan = resolve_path_plan(&paths, None).unwrap();
        assert_eq!(plan.inputs, vec![input]);
        assert_eq!(plan.single_output, Some(output));
    }

    #[test]
    fn test_resolve_path_plan_two_inputs_refuses_existing_output() {
        let temp_dir = TempDir::new().unwrap();
        let a = temp_dir.path().join("a.png");
        let b = temp_dir.path().join("b.png");
        fs::write(&a, b"fake").unwrap();
        fs::write(&b, b"fake").unwrap();

        let paths = vec![a, b];
        let err = resolve_path_plan(&paths, None).unwrap_err();
        let msg = format!("{:#}", err);
        assert!(msg.contains("Refusing to overwrite"), "got: {}", msg);
        assert!(
            msg.contains("--out-dir"),
            "should suggest --out-dir, got: {}",
            msg
        );
    }

    #[test]
    fn test_resolve_path_plan_three_inputs_all_inputs() {
        let paths = vec![
            PathBuf::from("a.png"),
            PathBuf::from("b.png"),
            PathBuf::from("c.png"),
        ];
        let plan = resolve_path_plan(&paths, None).unwrap();
        assert_eq!(plan.inputs.len(), 3);
        assert_eq!(plan.single_output, None);
    }

    #[test]
    fn test_resolve_path_plan_out_dir_forces_batch() {
        let temp_dir = TempDir::new().unwrap();
        let a = temp_dir.path().join("a.png");
        let b = temp_dir.path().join("b.png");
        fs::write(&a, b"fake").unwrap();
        fs::write(&b, b"fake").unwrap();

        // With --out-dir, even when the 2nd positional exists, both are inputs
        let paths = vec![a.clone(), b.clone()];
        let plan = resolve_path_plan(&paths, Some(temp_dir.path())).unwrap();
        assert_eq!(plan.inputs, vec![a, b]);
        assert_eq!(plan.single_output, None);
    }

    #[test]
    fn test_expand_globs_passes_through_literal_paths() {
        let temp_dir = TempDir::new().unwrap();
        let a = temp_dir.path().join("a.png");
        fs::write(&a, b"fake").unwrap();

        let expanded = expand_globs(std::slice::from_ref(&a)).unwrap();
        assert_eq!(expanded, vec![a]);
    }

    #[test]
    fn test_expand_globs_expands_quoted_glob() {
        let temp_dir = TempDir::new().unwrap();
        let a = temp_dir.path().join("a.png");
        let b = temp_dir.path().join("b.png");
        let other = temp_dir.path().join("c.jpg");
        fs::write(&a, b"fake").unwrap();
        fs::write(&b, b"fake").unwrap();
        fs::write(&other, b"fake").unwrap();

        let pattern = temp_dir.path().join("*.png");
        let expanded = expand_globs(&[pattern]).unwrap();
        let mut paths: Vec<_> = expanded
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        paths.sort();
        assert_eq!(paths, vec!["a.png".to_string(), "b.png".to_string()]);
    }

    #[test]
    fn test_expand_globs_errors_on_no_matches() {
        let temp_dir = TempDir::new().unwrap();
        let pattern = temp_dir.path().join("nonexistent*.png");
        let err = expand_globs(&[pattern]).unwrap_err();
        assert!(format!("{:#}", err).contains("No files matched"));
    }

    #[test]
    fn test_determine_output_path_respects_used_set() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("test.png");
        fs::write(&input_path, b"fake image data").unwrap();

        let mut used = HashSet::new();
        used.insert(temp_dir.path().join("test-bgone.png"));

        let result = determine_output_path(&input_path, None, None, &used).unwrap();
        assert_eq!(result, temp_dir.path().join("test-bgone-1.png"));
    }

    #[test]
    fn test_allocate_output_paths_avoids_collisions_between_inputs() {
        let temp_dir = TempDir::new().unwrap();
        let dir_a = temp_dir.path().join("a");
        let dir_b = temp_dir.path().join("b");
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();

        // Two inputs with the SAME filename in different source dirs, targeting
        // a single out_dir. Without collision avoidance, both would resolve to
        // `out/image-bgone.png` and silently clobber each other.
        let a = dir_a.join("image.png");
        let b = dir_b.join("image.png");
        fs::write(&a, b"fake").unwrap();
        fs::write(&b, b"fake").unwrap();

        let out_dir = temp_dir.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();

        let outputs = allocate_output_paths(&[a, b], None, Some(&out_dir)).unwrap();
        assert_eq!(outputs.len(), 2);
        assert_ne!(outputs[0], outputs[1], "outputs must be distinct paths");
        assert_eq!(outputs[0], out_dir.join("image-bgone.png"));
        assert_eq!(outputs[1], out_dir.join("image-bgone-1.png"));
    }

    #[test]
    fn test_allocate_output_paths_uses_single_output_when_set() {
        let temp_dir = TempDir::new().unwrap();
        let input = temp_dir.path().join("a.png");
        let explicit_output = temp_dir.path().join("out.png");
        fs::write(&input, b"fake").unwrap();

        let outputs = allocate_output_paths(&[input], Some(&explicit_output), None).unwrap();
        assert_eq!(outputs, vec![explicit_output]);
    }

    #[test]
    fn test_expand_globs_skips_expansion_for_existing_paths_with_glob_chars() {
        // If a literal file named with `[` actually exists, don't try to expand it
        let temp_dir = TempDir::new().unwrap();
        let weird = temp_dir.path().join("file[1].png");
        fs::write(&weird, b"fake").unwrap();

        let expanded = expand_globs(std::slice::from_ref(&weird)).unwrap();
        assert_eq!(expanded, vec![weird]);
    }
}
