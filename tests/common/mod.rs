use bgone::color::Color;
use image::{DynamicImage, ImageBuffer, Rgba};
use std::fs;

// Set to true to save test outputs for inspection
pub const SAVE_TEST_OUTPUTS: bool = true;

pub fn ensure_output_dir() {
    if SAVE_TEST_OUTPUTS {
        fs::create_dir_all("tests/outputs").unwrap();
    }
}

pub fn save_test_images(
    file_prefix: &str,
    test_name: &str,
    processed: &image::DynamicImage,
    reconstructed: &image::DynamicImage,
) {
    if SAVE_TEST_OUTPUTS {
        let processed_path = format!("tests/outputs/{}_{}_processed.png", file_prefix, test_name);
        let reconstructed_path = format!(
            "tests/outputs/{}_{}_reconstructed.png",
            file_prefix, test_name
        );

        processed.save(&processed_path).unwrap();
        reconstructed.save(&reconstructed_path).unwrap();

        println!("  Saved: {}", processed_path);
        println!("  Saved: {}", reconstructed_path);
    }
}

/// Overlay an image with alpha channel onto a solid background color
///
/// # Arguments
/// * `foreground` - The image with alpha channel to overlay
/// * `background_color` - The solid RGB background color
///
/// # Returns
/// A new image with the foreground composited onto the background
pub fn overlay_on_background(foreground: &DynamicImage, background_color: Color) -> DynamicImage {
    let fg_rgba = foreground.to_rgba8();
    let (width, height) = fg_rgba.dimensions();

    let mut result = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(width, height);
    let bg_rgba = Rgba([
        background_color[0],
        background_color[1],
        background_color[2],
        255,
    ]);

    for (x, y, result_pixel) in result.enumerate_pixels_mut() {
        let fg_pixel = fg_rgba.get_pixel(x, y);

        // Alpha blending: result = fg * alpha + bg * (1 - alpha)
        let alpha = fg_pixel[3] as f32 / 255.0;
        let inv_alpha = 1.0 - alpha;

        let blended = Rgba([
            (fg_pixel[0] as f32 * alpha + bg_rgba[0] as f32 * inv_alpha) as u8,
            (fg_pixel[1] as f32 * alpha + bg_rgba[1] as f32 * inv_alpha) as u8,
            (fg_pixel[2] as f32 * alpha + bg_rgba[2] as f32 * inv_alpha) as u8,
            255,
        ]);

        *result_pixel = blended;
    }

    DynamicImage::ImageRgba8(result)
}

/// Calculate the mean squared error between two images
///
/// # Arguments
/// * `img1` - First image to compare
/// * `img2` - Second image to compare
///
/// # Returns
/// The mean squared error across all RGB channels (lower is better)
///
/// # Panics
/// Panics if images have different dimensions
pub fn calculate_mse(img1: &DynamicImage, img2: &DynamicImage) -> f64 {
    let rgba1 = img1.to_rgba8();
    let rgba2 = img2.to_rgba8();

    assert_eq!(
        rgba1.dimensions(),
        rgba2.dimensions(),
        "Images must have same dimensions"
    );

    let mut sum_squared_diff = 0.0;
    let mut pixel_count = 0;

    for (p1, p2) in rgba1.pixels().zip(rgba2.pixels()) {
        for i in 0..3 {
            // Only compare RGB, not alpha
            let diff = p1[i] as f64 - p2[i] as f64;
            sum_squared_diff += diff * diff;
            pixel_count += 1;
        }
    }

    sum_squared_diff / pixel_count as f64
}

/// Calculate the Peak Signal-to-Noise Ratio (PSNR) in decibels
/// Higher values mean better quality (typical good values are > 30 dB)
pub fn calculate_psnr(img1: &DynamicImage, img2: &DynamicImage) -> f64 {
    let mse = calculate_mse(img1, img2);

    if mse == 0.0 {
        return f64::INFINITY; // Images are identical
    }

    let max_pixel_value = 255.0;
    20.0 * (max_pixel_value / mse.sqrt()).log10()
}

/// Calculate similarity percentage (0-100%) between two images
/// Based on normalized MSE
///
/// # Arguments
/// * `img1` - First image to compare
/// * `img2` - Second image to compare
///
/// # Returns
/// Similarity percentage where 100% means identical images
pub fn calculate_similarity_percentage(img1: &DynamicImage, img2: &DynamicImage) -> f64 {
    let mse = calculate_mse(img1, img2);
    let normalized_mse = mse / (255.0 * 255.0); // Normalize to 0-1 range
    (1.0 - normalized_mse) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_images_have_perfect_similarity() {
        let img = DynamicImage::new_rgba8(10, 10);
        assert_eq!(calculate_similarity_percentage(&img, &img), 100.0);
        assert_eq!(calculate_psnr(&img, &img), f64::INFINITY);
    }

    #[test]
    fn test_overlay_on_background() {
        // Create a simple test image with alpha
        let mut img = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(2, 2);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255])); // Opaque red
        img.put_pixel(1, 0, Rgba([255, 0, 0, 128])); // Semi-transparent red
        img.put_pixel(0, 1, Rgba([255, 0, 0, 0])); // Fully transparent
        img.put_pixel(1, 1, Rgba([0, 255, 0, 255])); // Opaque green

        let foreground = DynamicImage::ImageRgba8(img);
        let background_color = [0, 0, 255]; // Blue background

        let result = overlay_on_background(&foreground, background_color);
        let result_rgba = result.to_rgba8();

        // Check expected results
        assert_eq!(result_rgba.get_pixel(0, 0), &Rgba([255, 0, 0, 255])); // Still red
        assert_eq!(result_rgba.get_pixel(1, 0)[0], 128); // Mix of red and blue
        assert_eq!(result_rgba.get_pixel(0, 1), &Rgba([0, 0, 255, 255])); // Pure blue
        assert_eq!(result_rgba.get_pixel(1, 1), &Rgba([0, 255, 0, 255])); // Still green
    }
}
