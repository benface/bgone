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

/// Compare two images using image-compare library's hybrid comparison
///
/// This uses a combination of SSIM for structure and color difference metrics
/// Returns a score between 0.0 and 1.0 where 1.0 is identical
pub fn compare_images(img1: &DynamicImage, img2: &DynamicImage) -> Result<f64, String> {
    let rgb1 = img1.to_rgb8();
    let rgb2 = img2.to_rgb8();

    match image_compare::rgb_hybrid_compare(&rgb1, &rgb2) {
        Ok(result) => Ok(result.score),
        Err(_) => Err("Images must have same dimensions".to_string()),
    }
}

/// Compare two RGBA images directly, accounting for transparency
///
/// This is useful for comparing images with alpha channels
#[allow(dead_code)]
pub fn compare_rgba_images(img1: &DynamicImage, img2: &DynamicImage) -> Result<f64, String> {
    let rgba1 = img1.to_rgba8();
    let rgba2 = img2.to_rgba8();

    match image_compare::rgba_hybrid_compare(&rgba1, &rgba2) {
        Ok(result) => Ok(result.score),
        Err(_) => Err("Images must have same dimensions".to_string()),
    }
}

/// Calculate similarity percentage (0-100%) between two images
/// Uses image-compare's hybrid comparison
pub fn calculate_similarity_percentage(img1: &DynamicImage, img2: &DynamicImage) -> f64 {
    match compare_images(img1, img2) {
        Ok(score) => score * 100.0,
        Err(_) => 0.0,
    }
}

/// Calculate PSNR using RMS comparison
/// Note: image-compare doesn't provide PSNR directly, so we'll keep a simple version for now
pub fn calculate_psnr(img1: &DynamicImage, img2: &DynamicImage) -> f64 {
    let rgba1 = img1.to_rgba8();
    let rgba2 = img2.to_rgba8();

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

    let mse = sum_squared_diff / pixel_count as f64;
    if mse == 0.0 {
        return f64::INFINITY; // Images are identical
    }

    20.0 * (255.0 / mse.sqrt()).log10()
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
