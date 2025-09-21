pub mod background;
pub mod color;
pub mod deduce;
pub mod testing;
pub mod unmix;

use anyhow::{Context, Result};
use image::{ImageBuffer, Rgba};
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::Path;

use crate::color::{denormalize_color, normalize_color, Color, NormalizedColor};
use crate::unmix::{compute_result_color, unmix_colors};

/// Process an image to remove its background
pub fn process_image<P: AsRef<Path>>(
    input_path: P,
    output_path: P,
    foreground_colors: Vec<Color>,
    background_color: Color,
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
    let processed_pixels: Vec<[u8; 4]> = pixels
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
        .collect();

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
