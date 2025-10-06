pub mod background;
pub mod color;
pub mod deduce;
pub mod unmix;

use anyhow::{Context, Result};
use image::{ImageBuffer, Rgba};
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::Path;

use crate::color::{denormalize_color, normalize_color, Color, NormalizedColor};
use crate::unmix::{
    compute_result_color, is_color_close_to_foreground, unmix_colors,
    DEFAULT_COLOR_CLOSENESS_THRESHOLD,
};
use nalgebra::Vector3;

/// Process an image to remove its background
pub fn process_image<P: AsRef<Path>>(
    input_path: P,
    output_path: P,
    foreground_colors: Vec<Color>,
    background_color: Color,
    strict_mode: bool,
    threshold: Option<f64>,
) -> Result<()> {
    let input_path = input_path.as_ref();
    let output_path = output_path.as_ref();

    // Loading progress
    let load_progress = ProgressBar::new_spinner();
    load_progress.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} Loading image...")
            .expect("Failed to create progress bar style"),
    );
    load_progress.enable_steady_tick(std::time::Duration::from_millis(100));

    // Load image
    let img = image::open(input_path)
        .with_context(|| format!("Failed to open input image: {}", input_path.display()))?;

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    load_progress.finish_and_clear();
    println!(
        "✓ Loaded {} ({}x{} pixels)",
        input_path.file_name().unwrap_or_default().to_string_lossy(),
        width,
        height
    );

    // Normalize colors for processing
    let fg_normalized: Vec<NormalizedColor> = foreground_colors
        .iter()
        .map(|&color| normalize_color(color))
        .collect();

    let bg_normalized = normalize_color(background_color);

    // Setup progress bar
    let progress = create_progress_bar((width * height) as u64)?;

    // Process pixels in parallel
    let pixels: Vec<_> = rgba.pixels().collect();
    let processed_pixels: Vec<[u8; 4]> = if !strict_mode && foreground_colors.is_empty() {
        // Non-strict mode without foreground colors
        pixels
            .par_iter()
            .progress_with(progress.clone())
            .map(|pixel| {
                let observed = [pixel[0], pixel[1], pixel[2]];
                process_pixel_non_strict_no_fg(observed, bg_normalized)
            })
            .collect()
    } else if !strict_mode {
        // Non-strict mode WITH foreground colors
        let color_threshold = threshold.unwrap_or(DEFAULT_COLOR_CLOSENESS_THRESHOLD);
        pixels
            .par_iter()
            .progress_with(progress.clone())
            .map(|pixel| {
                let observed = [pixel[0], pixel[1], pixel[2]];
                process_pixel_non_strict_with_fg(
                    observed,
                    &fg_normalized,
                    bg_normalized,
                    color_threshold,
                )
            })
            .collect()
    } else {
        // Strict mode
        pixels
            .par_iter()
            .progress_with(progress.clone())
            .map(|pixel| {
                let observed = [pixel[0], pixel[1], pixel[2]];
                let unmix_result = unmix_colors(observed, &fg_normalized, bg_normalized);
                let (result_color, alpha) = compute_result_color(&unmix_result, &fg_normalized);

                let final_color = denormalize_color(result_color);
                [
                    final_color[0],
                    final_color[1],
                    final_color[2],
                    (alpha * 255.0).round() as u8,
                ]
            })
            .collect()
    };

    progress.finish_with_message(format!("✓ Processed {} pixels", width * height));

    // Create and save output image
    let save_progress = ProgressBar::new_spinner();
    save_progress.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} Saving image...")
            .expect("Failed to create progress bar style"),
    );
    save_progress.enable_steady_tick(std::time::Duration::from_millis(100));

    let mut output_img = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(width, height);
    for (i, pixel) in output_img.pixels_mut().enumerate() {
        *pixel = Rgba(processed_pixels[i]);
    }

    output_img
        .save(output_path)
        .with_context(|| format!("Failed to save output image: {}", output_path.display()))?;

    save_progress.finish_and_clear();
    println!(
        "✓ Saved to {}",
        output_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );

    Ok(())
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
        if (0.0..=1.0).contains(&fg_r) && (0.0..=1.0).contains(&fg_g) && (0.0..=1.0).contains(&fg_b) {
            best_alpha = alpha;
            best_fg = [fg_r, fg_g, fg_b];
            break; // This is the minimum alpha with direct computation
        }
    }

    Some((best_fg, best_alpha))
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
    let (best_fg, best_alpha) =
        find_minimum_alpha_for_color(obs_norm, background).unwrap_or({
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
        let (best_fg, best_alpha) = find_minimum_alpha_for_color(obs_norm, background)
            .unwrap_or({
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
