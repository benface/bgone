use crate::color::{normalize_color, Color, ForegroundColorSpec, NormalizedColor};
use crate::unmix::{compute_result_color, unmix_colors_internal};
use anyhow::Result;
use image::DynamicImage;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;

// Constants for color deduction algorithm
const MAX_CANDIDATES_2_UNKNOWNS: usize = 30; // Max candidates for exhaustive 2-unknown search
const MAX_CANDIDATES_3_UNKNOWNS_ALL: usize = 25; // Max candidates for full 3-unknown search
const MAX_CANDIDATES_3_UNKNOWNS_SELECTED: usize = 20; // Selected candidates for large 3-unknown search

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

/// Select N most different colors from a set, prioritizing pure/standard colors
fn select_most_different_colors(colors: &[Color], n: usize) -> Vec<Color> {
    if colors.len() <= n {
        return colors.to_vec();
    }

    let mut selected: Vec<Color> = Vec::new();

    while selected.len() < n {
        let next = colors
            .iter()
            .filter(|&&c| !selected.contains(&c))
            .max_by_key(|&&color| {
                if selected.is_empty() {
                    // If no colors selected yet, pick the most saturated
                    let [r, g, b] = color;
                    let max = r.max(g).max(b) as i32;
                    let min = r.min(g).min(b) as i32;
                    max - min // Saturation
                } else {
                    // Otherwise pick the one most different from selected colors
                    let min_dist = selected
                        .iter()
                        .map(|s| {
                            let dist = color_distance(normalize_color(color), normalize_color(*s));
                            (dist * 1000.0) as i32
                        })
                        .min()
                        .unwrap_or(i32::MAX);
                    min_dist
                }
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

    let reconstruction_error = total_error / total_weight;

    // Tie-breaker: Add a very small penalty for colors close to the background.
    // This helps prefer colors that are visually distinct from the background
    // when multiple color sets achieve nearly identical reconstruction quality.
    //
    // The penalty is designed to be insignificant compared to reconstruction error
    // (max 0.00001 per color), ensuring it only affects the choice when reconstruction
    // quality is essentially equal between candidates.
    let mut color_quality_penalty = 0.0;
    for fg_color in foreground_colors {
        let distance_to_bg = color_distance(*fg_color, background);
        // Maximum possible distance in normalized RGB space is sqrt(3) ≈ 1.732
        // (when colors are at opposite corners of the RGB cube)
        const MAX_RGB_DISTANCE: f64 = 1.732;
        let normalized_distance = distance_to_bg / MAX_RGB_DISTANCE;
        // Penalty decreases linearly with distance from background
        const MAX_PENALTY_PER_COLOR: f64 = 0.00001;
        color_quality_penalty += (1.0 - normalized_distance) * MAX_PENALTY_PER_COLOR;
    }
    color_quality_penalty /= foreground_colors.len() as f64;

    reconstruction_error + color_quality_penalty
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

    // Always add standard pure colors as they are often the best choice
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
            // Add if not already in candidates
            if !all_candidates
                .iter()
                .any(|&c| color_distance(normalize_color(c), normalize_color(color)) < 0.01)
            {
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
    } else if unknown_count == 2 && all_candidates.len() <= MAX_CANDIDATES_2_UNKNOWNS {
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
    } else if unknown_count == 3 {
        // For 3 unknowns, determine how many candidates to test based on total count
        let candidates_to_try = if all_candidates.len() <= MAX_CANDIDATES_3_UNKNOWNS_ALL {
            // Small set: test all combinations (25 choose 3 = 2300 combinations)
            all_candidates.clone()
        } else {
            // Large set: select the most different candidates to keep computation reasonable
            // (20 choose 3 = 1140 combinations)
            select_most_different_colors(&all_candidates, MAX_CANDIDATES_3_UNKNOWNS_SELECTED)
        };

        // Exhaustive search through all 3-color combinations
        for (i, c1) in candidates_to_try.iter().enumerate() {
            for (j, c2) in candidates_to_try.iter().enumerate().skip(i + 1) {
                for c3 in candidates_to_try.iter().skip(j + 1) {
                    let mut test_fg = vec![[0.0; 3]; specs.len()];
                    let mut known_idx = 0;
                    let test_unknown = [*c1, *c2, *c3];
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
        }
    } else {
        // For 4+ unknowns, use the most different candidates
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
