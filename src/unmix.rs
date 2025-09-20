use crate::color::{Color, NormalizedColor};
use nalgebra::{DMatrix, DVector, Vector3};

/// Small epsilon value for numerical stability in floating point comparisons
const EPSILON: f64 = 1e-10;

/// Result of color unmixing: weights for each foreground color and overall alpha
#[derive(Debug, Clone)]
pub struct UnmixResult {
    /// Weight for each foreground color (sums to 1.0 or less)
    pub weights: Vec<f64>,
    /// Overall alpha value (0.0 = fully transparent, 1.0 = fully opaque)
    pub alpha: f64,
}

/// Unmix an observed color into foreground components
///
/// Given an observed color and known foreground/background colors,
/// determines how much of each foreground color contributed to the observed color.
pub fn unmix_colors(
    observed: Color,
    foreground_colors: &[NormalizedColor],
    background: NormalizedColor,
) -> UnmixResult {
    let observed = Vector3::new(
        observed[0] as f64 / 255.0,
        observed[1] as f64 / 255.0,
        observed[2] as f64 / 255.0,
    );

    match foreground_colors.len() {
        0 => UnmixResult {
            weights: vec![],
            alpha: 0.0,
        },
        1 => unmix_single_color(observed, foreground_colors[0], background),
        _ => unmix_multiple_colors(observed, foreground_colors, background),
    }
}

/// Unmix when there's only one foreground color
fn unmix_single_color(
    observed: Vector3<f64>,
    foreground: NormalizedColor,
    background: NormalizedColor,
) -> UnmixResult {
    let fg = Vector3::from_row_slice(&foreground);
    let bg = Vector3::from_row_slice(&background);

    // observed = weight * fg + (1 - weight) * bg
    // Solve for weight
    let obs_minus_bg = observed - bg;
    let fg_minus_bg = fg - bg;

    let weight = if fg_minus_bg.norm() > EPSILON {
        let dot = obs_minus_bg.dot(&fg_minus_bg);
        let norm_sq = fg_minus_bg.dot(&fg_minus_bg);
        (dot / norm_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };

    UnmixResult {
        weights: vec![weight],
        alpha: weight,
    }
}

/// Unmix when there are multiple foreground colors using least squares
fn unmix_multiple_colors(
    observed: Vector3<f64>,
    foreground_colors: &[NormalizedColor],
    background: NormalizedColor,
) -> UnmixResult {
    let n = foreground_colors.len();

    // Build matrix where columns are (fg_i - bg)
    let mut matrix_data = Vec::with_capacity(3 * n);
    for fg in foreground_colors {
        matrix_data.push(fg[0] - background[0]);
        matrix_data.push(fg[1] - background[1]);
        matrix_data.push(fg[2] - background[2]);
    }

    let a = DMatrix::from_column_slice(3, n, &matrix_data);
    let b = observed - Vector3::from_row_slice(&background);
    let b_vec = DVector::from_column_slice(&[b[0], b[1], b[2]]);

    // Solve using pseudo-inverse
    let weights = match a.pseudo_inverse(EPSILON) {
        Ok(a_inv) => {
            let solution = a_inv * b_vec;
            solution.iter().map(|&w| w.max(0.0)).collect()
        }
        Err(_) => {
            // Fallback: use only first color
            let mut weights = vec![0.0; n];
            weights[0] = 1.0;
            weights
        }
    };

    // Calculate alpha as sum of weights (clamped to 1.0)
    let sum: f64 = weights.iter().sum();
    let (final_weights, alpha) = if sum > 1.0 {
        // Normalize weights to sum to 1.0
        let normalized: Vec<f64> = weights.iter().map(|w| w / sum).collect();
        (normalized, 1.0)
    } else {
        (weights, sum)
    };

    UnmixResult {
        weights: final_weights,
        alpha,
    }
}

/// Compute the final color from unmixing results
pub fn compute_result_color(
    unmix_result: &UnmixResult,
    foreground_colors: &[NormalizedColor],
) -> (NormalizedColor, f64) {
    if unmix_result.alpha == 0.0 {
        return ([0.0, 0.0, 0.0], 0.0);
    }

    let mut result = [0.0, 0.0, 0.0];
    let sum_weights: f64 = unmix_result.weights.iter().sum();

    if sum_weights > 0.0 {
        for (i, &weight) in unmix_result.weights.iter().enumerate() {
            if let Some(fg) = foreground_colors.get(i) {
                result[0] += weight * fg[0];
                result[1] += weight * fg[1];
                result[2] += weight * fg[2];
            }
        }

        // Normalize by sum of weights
        result[0] /= sum_weights;
        result[1] /= sum_weights;
        result[2] /= sum_weights;
    }

    (result, unmix_result.alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unmix_no_foreground_colors() {
        let result = unmix_colors([128, 128, 128], &[], [0.0, 0.0, 0.0]);
        assert_eq!(result.weights.len(), 0);
        assert_eq!(result.alpha, 0.0);
    }

    #[test]
    fn test_unmix_single_color_pure() {
        // Pure red on black background
        let result = unmix_colors([255, 0, 0], &[[1.0, 0.0, 0.0]], [0.0, 0.0, 0.0]);
        assert_eq!(result.weights.len(), 1);
        assert!((result.weights[0] - 1.0).abs() < EPSILON);
        assert!((result.alpha - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_unmix_single_color_blend() {
        // 50% red on black = rgb(128, 0, 0)
        let result = unmix_colors([127, 0, 0], &[[1.0, 0.0, 0.0]], [0.0, 0.0, 0.0]);
        assert!((result.weights[0] - 0.498).abs() < 0.01); // ~50%
        assert!((result.alpha - 0.498).abs() < 0.01);
    }

    #[test]
    fn test_unmix_single_color_identical_fg_bg() {
        // When foreground and background are the same, weight should be 0
        let result = unmix_colors([255, 0, 0], &[[1.0, 0.0, 0.0]], [1.0, 0.0, 0.0]);
        assert_eq!(result.weights[0], 0.0);
        assert_eq!(result.alpha, 0.0);
    }

    #[test]
    fn test_unmix_multiple_colors() {
        // Yellow (mix of red and green) on black
        let result = unmix_colors(
            [255, 255, 0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            [0.0, 0.0, 0.0],
        );
        assert_eq!(result.weights.len(), 2);
        // Should be roughly 50/50 mix of red and green
        assert!((result.weights[0] - 0.5).abs() < 0.1);
        assert!((result.weights[1] - 0.5).abs() < 0.1);
        assert!((result.alpha - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_compute_result_color_zero_alpha() {
        let unmix = UnmixResult {
            weights: vec![1.0],
            alpha: 0.0,
        };
        let (color, alpha) = compute_result_color(&unmix, &[[1.0, 0.0, 0.0]]);
        assert_eq!(color, [0.0, 0.0, 0.0]);
        assert_eq!(alpha, 0.0);
    }

    #[test]
    fn test_compute_result_color_weighted() {
        let unmix = UnmixResult {
            weights: vec![0.3, 0.7],
            alpha: 1.0,
        };
        let (color, alpha) = compute_result_color(&unmix, &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        assert!((color[0] - 0.3).abs() < EPSILON);
        assert!((color[1] - 0.7).abs() < EPSILON);
        assert_eq!(color[2], 0.0);
        assert_eq!(alpha, 1.0);
    }

    #[test]
    fn test_unmix_edge_cases() {
        // Test with identical foreground colors (singular matrix)
        let result1 = unmix_colors(
            [128, 0, 0],
            &[[1.0, 0.0, 0.0], [1.0, 0.0, 0.0]], // Same color twice
            [0.0, 0.0, 0.0],
        );
        assert_eq!(result1.weights.len(), 2);
        let total_weight: f64 = result1.weights.iter().sum();
        assert!((total_weight - 0.502).abs() < 0.01); // ~50% red

        // Test when observed color equals background
        let result2 = unmix_colors(
            [128, 128, 128],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            [0.502, 0.502, 0.502], // Gray background same as observed
        );
        assert!(result2.alpha < 0.01); // Should be nearly transparent
    }
}
