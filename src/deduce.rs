use crate::color::{normalize_color, Color, ForegroundColorSpec, NormalizedColor};
use crate::unmix::{compute_result_color, unmix_colors_internal};
use anyhow::Result;
use image::DynamicImage;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;

/// Calculate Euclidean distance between two colors in RGB space
fn color_distance(c1: NormalizedColor, c2: NormalizedColor) -> f64 {
    (0..3).map(|i| (c1[i] - c2[i]).powi(2)).sum::<f64>().sqrt()
}

/// Given observed colors and a background, find candidate foreground colors
/// that could have produced these observations through alpha blending
fn find_candidate_foreground_colors(
    observed_colors: &[(Color, usize)], // (color, count)
    background: Color,
    num_candidates: usize,
    threshold: f64,
) -> Vec<Color> {
    let bg_norm = normalize_color(background);
    let mut candidates = Vec::new();

    // For each observed color, calculate what foreground colors could produce it
    // at various alpha levels
    for &(observed, _) in observed_colors.iter().take(100) {
        // Limit to avoid too many candidates
        let obs_norm = normalize_color(observed);

        // Skip if too close to background
        if color_distance(obs_norm, bg_norm) < 0.01 {
            continue;
        }

        // Try different alpha values
        for alpha_percent in [25, 50, 75, 90, 100] {
            let alpha = alpha_percent as f64 / 100.0;

            // Calculate what foreground color would produce this observed color
            // observed = fg * alpha + bg * (1 - alpha)
            // fg = (observed - bg * (1 - alpha)) / alpha
            let mut fg = [0.0; 3];
            let mut valid = true;

            for i in 0..3 {
                fg[i] = (obs_norm[i] - bg_norm[i] * (1.0 - alpha)) / alpha;

                // Check if the result is a valid color
                if fg[i] < 0.0 || fg[i] > 1.0 {
                    valid = false;
                    break;
                }
            }

            if valid {
                let fg_u8 = [
                    (fg[0] * 255.0).round() as u8,
                    (fg[1] * 255.0).round() as u8,
                    (fg[2] * 255.0).round() as u8,
                ];

                // Verify reconstruction
                let reconstructed = [
                    (fg[0] * alpha + bg_norm[0] * (1.0 - alpha)) * 255.0,
                    (fg[1] * alpha + bg_norm[1] * (1.0 - alpha)) * 255.0,
                    (fg[2] * alpha + bg_norm[2] * (1.0 - alpha)) * 255.0,
                ];

                let error = (0..3)
                    .map(|i| (reconstructed[i] - observed[i] as f64).powi(2))
                    .sum::<f64>()
                    .sqrt();

                if error < 5.0 {
                    // Allow small rounding errors
                    candidates.push(fg_u8);
                }
            }
        }
    }

    // Deduplicate and find most different candidates
    let mut unique_candidates = Vec::new();
    for candidate in candidates {
        let mut is_duplicate = false;
        for existing in &unique_candidates {
            if color_distance(normalize_color(candidate), normalize_color(*existing)) < threshold {
                is_duplicate = true;
                break;
            }
        }
        if !is_duplicate {
            unique_candidates.push(candidate);
        }
    }

    // If we have too many candidates, select the most different ones
    if unique_candidates.len() > num_candidates {
        select_most_different_colors(&unique_candidates, num_candidates)
    } else {
        unique_candidates
    }
}

/// Select N most different colors from a set
fn select_most_different_colors(colors: &[Color], n: usize) -> Vec<Color> {
    if colors.len() <= n {
        return colors.to_vec();
    }

    let mut selected = Vec::new();

    // Start with a color (pick one with high saturation if possible)
    let first = colors
        .iter()
        .max_by_key(|&&[r, g, b]| {
            let max = r.max(g).max(b) as i32;
            let min = r.min(g).min(b) as i32;
            max - min // Saturation
        })
        .copied()
        .unwrap_or(colors[0]);

    selected.push(first);

    // Greedily select colors that are maximally different
    while selected.len() < n {
        let next = colors
            .iter()
            .filter(|c| !selected.contains(c))
            .max_by_key(|&&color| {
                let min_dist = selected
                    .iter()
                    .map(|&s| {
                        let dist = color_distance(normalize_color(color), normalize_color(s));
                        (dist * 1000.0) as i32
                    })
                    .min()
                    .unwrap_or(i32::MAX);
                min_dist
            });

        if let Some(&color) = next {
            selected.push(color);
        } else {
            break;
        }
    }

    selected
}

/// Evaluate how well a set of foreground colors reproduces the image
fn evaluate_color_set(
    foreground_colors: &[NormalizedColor],
    pixels: &[(Color, usize)], // (color, count)
    background: NormalizedColor,
) -> f64 {
    let mut total_error = 0.0;
    let mut total_weight = 0.0;

    for &(observed, count) in pixels {
        let weight = (count as f64).sqrt(); // Square root to reduce dominance of most common colors

        // Try to unmix this color
        let unmix_result = unmix_colors_internal(observed, foreground_colors, background, false);
        let (result_color, alpha) = compute_result_color(&unmix_result, foreground_colors);

        // Reconstruct what we would see
        let reconstructed = [
            result_color[0] * alpha + background[0] * (1.0 - alpha),
            result_color[1] * alpha + background[1] * (1.0 - alpha),
            result_color[2] * alpha + background[2] * (1.0 - alpha),
        ];

        // Compute error
        let observed_norm = normalize_color(observed);
        let error: f64 = (0..3)
            .map(|i| (reconstructed[i] - observed_norm[i]).powi(2))
            .sum::<f64>()
            .sqrt();

        total_error += error * weight;
        total_weight += weight;
    }

    total_error / total_weight
}

/// Deduce unknown foreground colors from an image
///
/// # Arguments
/// * `image` - The input image
/// * `specs` - The foreground color specifications (mix of known and unknown)
/// * `background_color` - The background color
///
/// # Returns
/// A vector of all foreground colors with unknowns replaced by deduced colors
pub fn deduce_unknown_colors(
    image: &DynamicImage,
    specs: &[ForegroundColorSpec],
    background_color: Color,
    threshold: f64,
) -> Result<Vec<Color>> {
    // Separate known and unknown specs
    let mut known_colors = Vec::new();
    let mut unknown_indices = Vec::new();

    for (i, spec) in specs.iter().enumerate() {
        match spec {
            ForegroundColorSpec::Known(color) => {
                known_colors.push(*color);
            }
            ForegroundColorSpec::Unknown => {
                unknown_indices.push(i);
            }
        }
    }

    if unknown_indices.is_empty() {
        // No unknowns to deduce
        return Ok(specs
            .iter()
            .map(|spec| match spec {
                ForegroundColorSpec::Known(color) => *color,
                ForegroundColorSpec::Unknown => unreachable!(),
            })
            .collect());
    }

    // Collect all unique colors and their counts
    let rgba = image.to_rgba8();
    let mut color_counts = HashMap::new();

    for pixel in rgba.pixels() {
        let color = [pixel[0], pixel[1], pixel[2]];
        *color_counts.entry(color).or_insert(0) += 1;
    }

    let mut pixels: Vec<(Color, usize)> = color_counts.into_iter().collect();
    pixels.sort_by_key(|&(_, count)| std::cmp::Reverse(count));

    println!("  Found {} unique colors in image", pixels.len());

    // Setup progress bar for deduction
    let progress = ProgressBar::new_spinner();
    progress.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} Deducing unknown colors...")
            .expect("Failed to create progress bar style"),
    );
    progress.enable_steady_tick(std::time::Duration::from_millis(100));

    // Find candidate foreground colors based on unmixing
    let unknown_count = unknown_indices.len();
    let candidates = find_candidate_foreground_colors(
        &pixels,
        background_color,
        unknown_count * 10, // Get more candidates for better selection
        threshold,
    );

    // If we don't have enough candidates, add some standard colors
    let mut all_candidates = candidates;
    if all_candidates.len() < unknown_count {
        let standard_colors = vec![
            [255, 0, 0],   // Red
            [0, 255, 0],   // Green
            [0, 0, 255],   // Blue
            [255, 255, 0], // Yellow
            [255, 0, 255], // Magenta
            [0, 255, 255], // Cyan
            [255, 128, 0], // Orange
            [128, 0, 255], // Purple
        ];

        for color in standard_colors {
            if !known_colors.contains(&color) && color != background_color {
                all_candidates.push(color);
            }
        }
    }

    // Evaluate different combinations
    let background_norm = normalize_color(background_color);
    let known_norm: Vec<NormalizedColor> =
        known_colors.iter().map(|&c| normalize_color(c)).collect();

    let mut best_colors = vec![];
    let mut best_error = f64::MAX;

    // For 1-2 unknowns, try all combinations
    if unknown_count == 1 {
        for candidate in &all_candidates {
            let mut test_fg = vec![[0.0; 3]; specs.len()];
            let mut known_idx = 0;

            for (i, spec) in specs.iter().enumerate() {
                match spec {
                    ForegroundColorSpec::Known(_) => {
                        test_fg[i] = known_norm[known_idx];
                        known_idx += 1;
                    }
                    ForegroundColorSpec::Unknown => {
                        test_fg[i] = normalize_color(*candidate);
                    }
                }
            }

            let error = evaluate_color_set(&test_fg, &pixels, background_norm);
            if error < best_error {
                best_error = error;
                best_colors = vec![*candidate];
            }
        }
    } else if unknown_count == 2 && all_candidates.len() <= 20 {
        for (i, c1) in all_candidates.iter().enumerate() {
            for c2 in all_candidates.iter().skip(i + 1) {
                let mut test_fg = vec![[0.0; 3]; specs.len()];
                let mut known_idx = 0;
                let test_unknown = [*c1, *c2];
                let mut unknown_idx = 0;

                for (i, spec) in specs.iter().enumerate() {
                    match spec {
                        ForegroundColorSpec::Known(_) => {
                            test_fg[i] = known_norm[known_idx];
                            known_idx += 1;
                        }
                        ForegroundColorSpec::Unknown => {
                            test_fg[i] = normalize_color(test_unknown[unknown_idx]);
                            unknown_idx += 1;
                        }
                    }
                }

                let error = evaluate_color_set(&test_fg, &pixels, background_norm);
                if error < best_error {
                    best_error = error;
                    best_colors = test_unknown.to_vec();
                }
            }
        }
    } else {
        // For more unknowns, use the most different candidates
        best_colors = select_most_different_colors(&all_candidates, unknown_count);
    }

    progress.finish_and_clear();

    // Build final result
    let mut final_colors = Vec::new();
    let mut unknown_idx = 0;

    for spec in specs {
        match spec {
            ForegroundColorSpec::Known(color) => {
                final_colors.push(*color);
            }
            ForegroundColorSpec::Unknown => {
                if unknown_idx < best_colors.len() {
                    final_colors.push(best_colors[unknown_idx]);
                } else {
                    final_colors.push([128, 128, 128]); // Fallback
                }
                unknown_idx += 1;
            }
        }
    }

    // Print results
    let deduced_strs: Vec<String> = best_colors
        .iter()
        .map(|&[r, g, b]| format!("#{:02x}{:02x}{:02x}", r, g, b))
        .collect();

    let plural = if unknown_count == 1 {
        "color"
    } else {
        "colors"
    };
    println!(
        "✓ Deduced {} unknown {}: {}",
        unknown_count,
        plural,
        deduced_strs.join(" ")
    );

    Ok(final_colors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::ForegroundColorSpec;
    use image::DynamicImage;

    #[test]
    fn test_no_unknowns() {
        let specs = vec![
            ForegroundColorSpec::Known([255, 0, 0]),
            ForegroundColorSpec::Known([0, 255, 0]),
        ];

        let img = DynamicImage::new_rgb8(10, 10);
        let result = deduce_unknown_colors(&img, &specs, [0, 0, 0], 0.05).unwrap();

        assert_eq!(result, vec![[255, 0, 0], [0, 255, 0]]);
    }
}
