pub mod background;
pub mod color;
pub mod deduce;
pub mod unmix;

use anyhow::{Context, Result};
use image::{ImageBuffer, Rgba};
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::Path;

use crate::color::{Color, NormalizedColor, denormalize_color, normalize_color};
use crate::unmix::{
    DEFAULT_COLOR_CLOSENESS_THRESHOLD, compute_result_color, is_color_close_to_foreground,
    unmix_colors,
};
use nalgebra::Vector3;

/// Tunable options for [`process_image`].
///
/// Defaults: non-strict, no thresholds (existing exact-match behavior), no
/// trim, verbose progress output.
#[derive(Debug, Default, Clone)]
pub struct ProcessOptions {
    /// If true, restrict unmixing to the specified foreground colors only.
    /// Requires at least one foreground color.
    pub strict_mode: bool,
    /// Foreground color similarity threshold (0.0-1.0). `None` uses the default
    /// of [`unmix::DEFAULT_COLOR_CLOSENESS_THRESHOLD`].
    pub fg_threshold: Option<f64>,
    /// Background snap threshold (0.0-1.0). Pixels within this per-channel L∞
    /// distance of the resolved background are forced to fully transparent
    /// before unmixing. `None` (or 0.0) preserves the existing exact-match-only
    /// behavior.
    pub bg_threshold: Option<f64>,
    /// Crop the output to the bounding box of non-transparent pixels.
    pub trim: bool,
    /// Suppress per-step progress bars and status output (used in batch mode).
    pub quiet: bool,
}

/// Process an image to remove its background.
///
/// Returns `Err` if `options.strict_mode` is set but `foreground_colors` is
/// empty — strict mode requires at least one color to unmix against.
pub fn process_image<P: AsRef<Path>>(
    input_path: P,
    output_path: P,
    foreground_colors: Vec<Color>,
    background_color: Color,
    options: ProcessOptions,
) -> Result<()> {
    let input_path = input_path.as_ref();
    let output_path = output_path.as_ref();
    let ProcessOptions {
        strict_mode,
        fg_threshold,
        bg_threshold,
        trim,
        quiet,
    } = options;

    if strict_mode && foreground_colors.is_empty() {
        anyhow::bail!(
            "Strict mode requires at least one foreground color, but `foreground_colors` is empty"
        );
    }

    // Loading progress
    let load_progress = if quiet {
        ProgressBar::hidden()
    } else {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} Loading image...")
                .expect("Failed to create progress bar style"),
        );
        bar.enable_steady_tick(std::time::Duration::from_millis(100));
        bar
    };

    // Load image
    let img = image::open(input_path)
        .with_context(|| format!("Failed to open input image: {}", input_path.display()))?;

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    load_progress.finish_and_clear();
    if !quiet {
        println!(
            "✓ Loaded {} ({}x{} pixels)",
            input_path.file_name().unwrap_or_default().to_string_lossy(),
            width,
            height
        );
    }

    // Normalize colors for processing
    let fg_normalized: Vec<NormalizedColor> = foreground_colors
        .iter()
        .map(|&color| normalize_color(color))
        .collect();

    let bg_normalized = normalize_color(background_color);

    // Setup progress bar
    let progress = if quiet {
        ProgressBar::hidden()
    } else {
        create_progress_bar((width * height) as u64)?
    };

    // Process pixels in parallel. The mode dispatch is loop-invariant — the
    // branch predictor handles it for free — so a single par_iter is both
    // simpler and gives identical performance to one-iter-per-mode.
    let pixels: Vec<_> = rgba.pixels().collect();
    let bg_snap_threshold = bg_threshold.unwrap_or(0.0);
    let fg_color_threshold = fg_threshold.unwrap_or(DEFAULT_COLOR_CLOSENESS_THRESHOLD);
    let has_fg = !foreground_colors.is_empty();

    let processed_pixels: Vec<[u8; 4]> = pixels
        .par_iter()
        .progress_with(progress.clone())
        .map(|pixel| {
            // Pre-composite translucent pixels over background to get opaque color
            let observed = composite_pixel_over_background(pixel, background_color);
            if is_within_bg_threshold(observed, background_color, bg_snap_threshold) {
                return [0, 0, 0, 0];
            }
            if strict_mode {
                process_pixel_strict(observed, &fg_normalized, bg_normalized)
            } else if has_fg {
                process_pixel_non_strict_with_fg(
                    observed,
                    &fg_normalized,
                    bg_normalized,
                    fg_color_threshold,
                )
            } else {
                process_pixel_non_strict_no_fg(observed, bg_normalized)
            }
        })
        .collect();

    if quiet {
        progress.finish_and_clear();
    } else {
        progress.finish_with_message(format!("✓ Processed {} pixels", width * height));
    }

    // Create output image
    let mut output_img = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(width, height);
    for (i, pixel) in output_img.pixels_mut().enumerate() {
        *pixel = Rgba(processed_pixels[i]);
    }

    // Apply trim if requested
    let final_img = if trim {
        let trim_progress = if quiet {
            ProgressBar::hidden()
        } else {
            let bar = ProgressBar::new_spinner();
            bar.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} Trimming image...")
                    .expect("Failed to create progress bar style"),
            );
            bar.enable_steady_tick(std::time::Duration::from_millis(100));
            bar
        };

        let trimmed = trim_to_content(&output_img);
        let (new_width, new_height) = trimmed.dimensions();

        trim_progress.finish_and_clear();
        if !quiet {
            if new_width != width || new_height != height {
                println!(
                    "✓ Trimmed from {}x{} to {}x{}",
                    width, height, new_width, new_height
                );
            } else {
                println!("✓ No trimming needed (image already tight)");
            }
        }

        trimmed
    } else {
        output_img
    };

    // Save output image
    let save_progress = if quiet {
        ProgressBar::hidden()
    } else {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} Saving image...")
                .expect("Failed to create progress bar style"),
        );
        bar.enable_steady_tick(std::time::Duration::from_millis(100));
        bar
    };

    final_img
        .save(output_path)
        .with_context(|| format!("Failed to save output image: {}", output_path.display()))?;

    save_progress.finish_and_clear();
    if !quiet {
        println!(
            "✓ Saved to {}",
            output_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );
    }

    Ok(())
}

/// Trim an image by cropping to the bounding box of non-transparent pixels.
///
/// Finds the bounding box of all pixels with alpha > 0 and crops the image
/// to that region. If all pixels are transparent, returns a 1x1 transparent image.
pub fn trim_to_content(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let (width, height) = img.dimensions();

    if width == 0 || height == 0 {
        return ImageBuffer::new(1, 1);
    }

    // Find bounding box of non-transparent pixels
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0u32;
    let mut max_y = 0u32;

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            if pixel[3] > 0 {
                // Non-transparent pixel
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    // If no non-transparent pixels found, return a 1x1 transparent image
    if max_x < min_x || max_y < min_y {
        return ImageBuffer::from_pixel(1, 1, Rgba([0, 0, 0, 0]));
    }

    // Calculate new dimensions (inclusive bounds, so add 1)
    let new_width = max_x - min_x + 1;
    let new_height = max_y - min_y + 1;

    // If no trimming needed, return a clone
    if new_width == width && new_height == height {
        return img.clone();
    }

    // Create cropped image
    let mut trimmed = ImageBuffer::new(new_width, new_height);
    for y in 0..new_height {
        for x in 0..new_width {
            let src_pixel = img.get_pixel(min_x + x, min_y + y);
            trimmed.put_pixel(x, y, *src_pixel);
        }
    }

    trimmed
}

/// Create a progress bar with consistent styling
fn create_progress_bar(total: u64) -> Result<ProgressBar> {
    let progress = ProgressBar::new(total);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} pixels ({percent}%)")?
            .progress_chars("#>-")
    );
    progress.set_message("Processing pixels...");
    Ok(progress)
}

/// Whether an observed (already alpha-composited) pixel color is close enough to
/// the background to be snapped to fully transparent.
///
/// Uses an L∞ (per-channel max) distance metric in normalized [0.0, 1.0] space,
/// so a `bg_threshold` of `0.004` corresponds to ~1/255 per channel.
///
/// With `bg_threshold <= 0.0` this is a no-op: exact-background pixels are
/// still handled correctly downstream — non-strict modes have an explicit
/// 1e-6 early-return, and strict mode's unmix yields alpha ≈ 0 for them.
fn is_within_bg_threshold(observed: Color, background: Color, bg_threshold: f64) -> bool {
    if bg_threshold <= 0.0 {
        return false;
    }
    let dr = observed[0].abs_diff(background[0]);
    let dg = observed[1].abs_diff(background[1]);
    let db = observed[2].abs_diff(background[2]);
    let max_delta = dr.max(dg).max(db) as f64 / 255.0;
    max_delta <= bg_threshold
}

/// Composite a pixel over a background color to handle existing alpha channels
///
/// If the input pixel is translucent (alpha < 255), this pre-composes it over
/// the background color to produce an opaque equivalent. This allows bgone to
/// correctly process images that already have transparency.
///
/// Formula: result = foreground * alpha + background * (1 - alpha)
fn composite_pixel_over_background(pixel: &Rgba<u8>, background: Color) -> Color {
    let alpha = pixel[3] as f64 / 255.0;

    if alpha >= 1.0 {
        // Fully opaque - use as-is
        [pixel[0], pixel[1], pixel[2]]
    } else {
        // Translucent - composite over background
        let bg_norm = [
            background[0] as f64 / 255.0,
            background[1] as f64 / 255.0,
            background[2] as f64 / 255.0,
        ];
        let fg_norm = [
            pixel[0] as f64 / 255.0,
            pixel[1] as f64 / 255.0,
            pixel[2] as f64 / 255.0,
        ];

        [
            ((fg_norm[0] * alpha + bg_norm[0] * (1.0 - alpha)) * 255.0).round() as u8,
            ((fg_norm[1] * alpha + bg_norm[1] * (1.0 - alpha)) * 255.0).round() as u8,
            ((fg_norm[2] * alpha + bg_norm[2] * (1.0 - alpha)) * 255.0).round() as u8,
        ]
    }
}

/// Find the minimum alpha value that produces a valid foreground color
///
/// Given an observed color and background, this function finds the minimum alpha
/// value (between 0 and 1) such that there exists a valid foreground color
/// (all RGB components in [0, 1]) that satisfies:
/// observed = alpha * foreground + (1 - alpha) * background
///
/// Returns (foreground_color, alpha) or None if no valid solution exists
fn find_minimum_alpha_for_color(
    obs_norm: NormalizedColor,
    background: NormalizedColor,
) -> Option<(NormalizedColor, f64)> {
    let mut best_alpha = 1.0;
    let mut best_fg = obs_norm;

    // For truly minimal alpha, we need to consider different foreground colors.
    // The optimal foreground often has components at the extremes (0 or 1).
    // We'll try all 8 combinations of extreme values, plus the computed values.

    // First, let's compute the minimum alpha needed for each channel independently
    // For each channel i: observed[i] = alpha * fg[i] + (1 - alpha) * bg[i]
    // If fg[i] = 0: alpha = (bg[i] - observed[i]) / bg[i] (if bg[i] != 0)
    // If fg[i] = 1: alpha = (observed[i] - bg[i]) / (1 - bg[i]) (if bg[i] != 1)

    // Try all combinations of extreme foreground values (0 or 1 for each channel)
    for r_extreme in &[0.0, 1.0] {
        for g_extreme in &[0.0, 1.0] {
            for b_extreme in &[0.0, 1.0] {
                let fg_candidate = [*r_extreme, *g_extreme, *b_extreme];

                // Calculate required alpha for this foreground color
                // observed = alpha * foreground + (1 - alpha) * background
                // alpha = (observed - background) / (foreground - background)

                let mut alpha_needed = 0.0;
                let mut valid = true;

                let mut first_alpha_set = false;

                for i in 0..3 {
                    let denom = fg_candidate[i] - background[i];
                    if denom.abs() < 1e-10 {
                        // fg[i] ≈ bg[i], check if observed[i] ≈ bg[i] too
                        if (obs_norm[i] - background[i]).abs() > 1e-10 {
                            valid = false;
                            break;
                        }
                        // Any alpha works for this channel, continue
                    } else {
                        let alpha_i = (obs_norm[i] - background[i]) / denom;
                        if !first_alpha_set {
                            alpha_needed = alpha_i;
                            first_alpha_set = true;
                        } else if (alpha_i - alpha_needed).abs() > 1e-10 {
                            // Different channels require different alphas - invalid
                            valid = false;
                            break;
                        }
                    }
                }

                if valid
                    && first_alpha_set
                    && alpha_needed > 0.0
                    && alpha_needed <= 1.0
                    && alpha_needed < best_alpha
                {
                    // Verify the solution
                    let mut reconstructed_valid = true;
                    for i in 0..3 {
                        let reconstructed =
                            alpha_needed * fg_candidate[i] + (1.0 - alpha_needed) * background[i];
                        if (reconstructed - obs_norm[i]).abs() > 1e-10 {
                            reconstructed_valid = false;
                            break;
                        }
                    }

                    if reconstructed_valid {
                        best_alpha = alpha_needed;
                        best_fg = fg_candidate;
                    }
                }
            }
        }
    }

    // Also try the direct computation approach with fine-grained alpha search
    for alpha_int in 1..=1000 {
        let alpha = alpha_int as f64 / 1000.0;

        if alpha >= best_alpha {
            break; // No point checking higher alphas
        }

        // Calculate the required foreground color for this alpha
        let fg_r = (obs_norm[0] - (1.0 - alpha) * background[0]) / alpha;
        let fg_g = (obs_norm[1] - (1.0 - alpha) * background[1]) / alpha;
        let fg_b = (obs_norm[2] - (1.0 - alpha) * background[2]) / alpha;

        // Check if this foreground color is valid (all components in [0, 1])
        if (0.0..=1.0).contains(&fg_r) && (0.0..=1.0).contains(&fg_g) && (0.0..=1.0).contains(&fg_b)
        {
            best_alpha = alpha;
            best_fg = [fg_r, fg_g, fg_b];
            break; // This is the minimum alpha with direct computation
        }
    }

    Some((best_fg, best_alpha))
}

/// Process a pixel in strict mode: unmix against the supplied foreground
/// colors only, optimizing for maximum opacity.
fn process_pixel_strict(
    observed: Color,
    foreground_colors: &[NormalizedColor],
    background: NormalizedColor,
) -> [u8; 4] {
    let unmix_result = unmix_colors(observed, foreground_colors, background);
    let (result_color, alpha) = compute_result_color(&unmix_result, foreground_colors);
    let final_color = denormalize_color(result_color);
    [
        final_color[0],
        final_color[1],
        final_color[2],
        (alpha * 255.0).round() as u8,
    ]
}

/// Process a pixel in non-strict mode without foreground colors
///
/// In this mode, we find the optimal foreground color and alpha that produces
/// the observed color when alpha-blended with the background.
///
/// The algorithm:
/// 1. Searches for the minimum alpha value that allows a valid foreground color
/// 2. A valid foreground color has all RGB components in [0, 1] range
/// 3. Always produces perfect reconstruction of the original image
fn process_pixel_non_strict_no_fg(observed: Color, background: NormalizedColor) -> [u8; 4] {
    let obs_norm = normalize_color(observed);

    // If the observed color is exactly the background, it's fully transparent
    if (obs_norm[0] - background[0]).abs() < 1e-6
        && (obs_norm[1] - background[1]).abs() < 1e-6
        && (obs_norm[2] - background[2]).abs() < 1e-6
    {
        return [0, 0, 0, 0];
    }

    // Find the optimal alpha and foreground color
    let (best_fg, best_alpha) = find_minimum_alpha_for_color(obs_norm, background).unwrap_or({
        // If we didn't find a valid solution with alpha <= 1.0, something is wrong
        // Fall back to using alpha = 1.0
        (obs_norm, 1.0)
    });

    let final_color = denormalize_color(best_fg);
    [
        final_color[0],
        final_color[1],
        final_color[2],
        (best_alpha * 255.0).round() as u8,
    ]
}

/// Process a pixel in non-strict mode with foreground colors
///
/// This mode combines two strategies:
/// 1. For pixels "close enough" to specified foreground colors (within threshold):
///    - Uses the standard unmixing algorithm optimized for high opacity
///    - Restricts to the specified foreground colors
/// 2. For pixels NOT close to any foreground color:
///    - Allows ANY color to be used
///    - Finds the minimum alpha that produces a valid foreground color
///    - Ensures perfect reconstruction
///
/// This allows the tool to preserve colors like glows and gradients that aren't
/// close to the specified foreground colors, while still optimizing for the
/// specified colors when appropriate.
fn process_pixel_non_strict_with_fg(
    observed: Color,
    foreground_colors: &[NormalizedColor],
    background: NormalizedColor,
    threshold: f64,
) -> [u8; 4] {
    let obs_norm = normalize_color(observed);
    let obs_vec = Vector3::new(obs_norm[0] as f64, obs_norm[1] as f64, obs_norm[2] as f64);

    // If the observed color is exactly the background, it's fully transparent
    if (obs_norm[0] - background[0]).abs() < 1e-6
        && (obs_norm[1] - background[1]).abs() < 1e-6
        && (obs_norm[2] - background[2]).abs() < 1e-6
    {
        return [0, 0, 0, 0];
    }

    // Check if this pixel is close to any foreground color
    let close_to_fg =
        is_color_close_to_foreground(obs_vec, foreground_colors, background, threshold);

    if close_to_fg {
        // Use the standard unmixing algorithm optimized for high opacity
        let unmix_result = unmix_colors(observed, foreground_colors, background);
        let (result_color, alpha) = compute_result_color(&unmix_result, foreground_colors);
        let final_color = denormalize_color(result_color);
        [
            final_color[0],
            final_color[1],
            final_color[2],
            (alpha * 255.0).round() as u8,
        ]
    } else {
        // Not close to any foreground color - find ANY color that works with minimal alpha
        let obs_norm = normalize_color(observed);

        // Find the optimal alpha and foreground color
        let (best_fg, best_alpha) = find_minimum_alpha_for_color(obs_norm, background).unwrap_or({
            // If we didn't find a valid solution with alpha <= 1.0, something is wrong
            // Fall back to using alpha = 1.0
            (obs_norm, 1.0)
        });

        let final_color = denormalize_color(best_fg);
        [
            final_color[0],
            final_color[1],
            final_color[2],
            (best_alpha * 255.0).round() as u8,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn test_composite_pixel_fully_opaque() {
        let pixel = Rgba([255, 0, 0, 255]); // Opaque red
        let background = [255, 255, 255]; // White

        let result = composite_pixel_over_background(&pixel, background);
        assert_eq!(result, [255, 0, 0]); // Should stay red
    }

    #[test]
    fn test_composite_pixel_fully_transparent() {
        let pixel = Rgba([255, 0, 0, 0]); // Fully transparent red
        let background = [255, 255, 255]; // White

        let result = composite_pixel_over_background(&pixel, background);
        assert_eq!(result, [255, 255, 255]); // Should be white (background)
    }

    #[test]
    fn test_composite_pixel_semi_transparent() {
        let pixel = Rgba([255, 0, 0, 128]); // ~50% transparent red (128/255 = 0.502)
        let background = [255, 255, 255]; // White

        let result = composite_pixel_over_background(&pixel, background);
        // ~50% red + ~50% white = rgb(255, 127, 127)
        assert_eq!(result, [255, 127, 127]);
    }

    #[test]
    fn test_composite_pixel_semi_transparent_on_black() {
        let pixel = Rgba([255, 0, 0, 128]); // 50% transparent red
        let background = [0, 0, 0]; // Black

        let result = composite_pixel_over_background(&pixel, background);
        // 50% red + 50% black = rgb(128, 0, 0)
        assert_eq!(result, [128, 0, 0]);
    }

    #[test]
    fn test_composite_pixel_quarter_transparent() {
        let pixel = Rgba([200, 100, 50, 64]); // 25% transparent (64/255)
        let background = [0, 0, 0]; // Black

        let result = composite_pixel_over_background(&pixel, background);
        // Approximately 25% of the color
        assert_eq!(result, [50, 25, 13]);
    }

    #[test]
    fn test_bg_threshold_zero_is_noop() {
        let bg = [0, 0, 0];
        // Even an exact match doesn't snap at threshold 0 (existing exact-match
        // logic in the pixel processors covers that case).
        assert!(!is_within_bg_threshold([0, 0, 0], bg, 0.0));
        assert!(!is_within_bg_threshold([1, 0, 0], bg, 0.0));
    }

    #[test]
    fn test_bg_threshold_uses_l_infinity_metric() {
        let bg = [0, 0, 0];
        // 1/255 ≈ 0.00392, so threshold 0.004 should snap any single-channel delta of 1
        let t = 0.004;
        assert!(is_within_bg_threshold([1, 0, 0], bg, t));
        assert!(is_within_bg_threshold([0, 1, 0], bg, t));
        assert!(is_within_bg_threshold([0, 0, 1], bg, t));
        // And all three channels at delta 1 (L∞ = 1, NOT √3)
        assert!(is_within_bg_threshold([1, 1, 1], bg, t));
        // But delta 2 in any channel exceeds the threshold
        assert!(!is_within_bg_threshold([2, 0, 0], bg, t));
        assert!(!is_within_bg_threshold([0, 2, 0], bg, t));
    }

    #[test]
    fn test_bg_threshold_works_off_black_background() {
        let bg = [120, 200, 80];
        // ±1 per channel
        assert!(is_within_bg_threshold([121, 200, 80], bg, 0.004));
        assert!(is_within_bg_threshold([119, 201, 79], bg, 0.004));
        // ±2 per channel exceeds 0.004
        assert!(!is_within_bg_threshold([122, 200, 80], bg, 0.004));
    }

    #[test]
    fn test_bg_threshold_larger_values() {
        let bg = [0, 0, 0];
        // 5/255 ≈ 0.0196, threshold 0.02 should snap up to delta 5
        let t = 0.02;
        assert!(is_within_bg_threshold([5, 5, 5], bg, t));
        assert!(!is_within_bg_threshold([6, 0, 0], bg, t));
    }

    #[test]
    fn test_process_image_rejects_strict_mode_with_empty_fg() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let input_path = temp_dir.path().join("in.png");
        let output_path = temp_dir.path().join("out.png");

        let mut img = image::RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        image::DynamicImage::ImageRgba8(img)
            .save(&input_path)
            .unwrap();

        let err = process_image(
            &input_path,
            &output_path,
            vec![],
            [0, 0, 0],
            ProcessOptions {
                strict_mode: true,
                ..Default::default()
            },
        )
        .unwrap_err();

        let msg = format!("{:#}", err);
        assert!(
            msg.contains("Strict mode requires"),
            "expected strict-mode error, got: {}",
            msg
        );
        assert!(
            !output_path.exists(),
            "no output should be written on validation failure"
        );
    }
}
