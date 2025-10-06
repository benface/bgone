mod common;
use assert_cmd::Command;
use common::{
    calculate_psnr, compare_rgba_images, ensure_output_dir, overlay_on_background, save_test_images,
};
use image::{DynamicImage, ImageBuffer, Rgba};
use tempfile::TempDir;

// Load the fire image (translucent test image)
fn load_fire_image() -> DynamicImage {
    image::open("tests/inputs/fire.png").expect("Failed to load fire image")
}

// Create a composited image (foreground over background)
fn create_composited_image(foreground: &DynamicImage, background_color: [u8; 3]) -> DynamicImage {
    let fg_rgba = foreground.to_rgba8();
    let (width, height) = fg_rgba.dimensions();

    let mut result = ImageBuffer::new(width, height);
    let bg_rgba = Rgba([
        background_color[0],
        background_color[1],
        background_color[2],
        255,
    ]);

    for (x, y, result_pixel) in result.enumerate_pixels_mut() {
        let fg_pixel = fg_rgba.get_pixel(x, y);

        // Alpha blending
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

// Compare processed image with original
fn compare_with_original(processed: &DynamicImage, original: &DynamicImage) -> (f64, f64) {
    let similarity = match compare_rgba_images(processed, original) {
        Ok(score) => score * 100.0,
        Err(_) => 0.0,
    };
    let psnr = calculate_psnr(processed, original);
    (similarity, psnr)
}

// Black background tests

#[test]
fn test_fire_on_black_non_strict() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();

    // Load the original translucent fire image
    let original_fire = load_fire_image();

    // Create fire on black background (what bgone will process)
    let black_bg = [0, 0, 0];
    let fire_on_black = create_composited_image(&original_fire, black_bg);

    // Save the composited image
    let composited_path = temp_dir.path().join("fire_on_black.png");
    fire_on_black.save(&composited_path).unwrap();

    // Run bgone in non-strict mode without specifying foreground colors
    let output_path = temp_dir.path().join("output.png");
    Command::cargo_bin("bgone")
        .unwrap()
        .args([
            composited_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "--bg",
            "000000",
        ])
        .assert()
        .success();

    // Load the processed image and compare with the original translucent fire
    let processed = image::open(&output_path).unwrap();
    let (similarity, psnr) = compare_with_original(&processed, &original_fire);

    // Save for inspection
    let reconstructed = overlay_on_background(&processed, black_bg);
    save_test_images(
        "translucent_recovery",
        "black_non_strict",
        &processed,
        &reconstructed,
    );

    println!(
        "Fire on black (non-strict) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );

    // We expect good recovery in non-strict mode
    assert!(
        similarity > 95.0,
        "Similarity {:.2}% is too low",
        similarity
    );
}

#[test]
fn test_fire_on_black_non_strict_single_auto() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();

    let original_fire = load_fire_image();
    let black_bg = [0, 0, 0];
    let fire_on_black = create_composited_image(&original_fire, black_bg);

    let composited_path = temp_dir.path().join("fire_on_black.png");
    fire_on_black.save(&composited_path).unwrap();

    // Run bgone in non-strict mode with single auto
    let output_path = temp_dir.path().join("output.png");
    Command::cargo_bin("bgone")
        .unwrap()
        .args([
            composited_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "--fg",
            "auto",
            "--bg",
            "000000",
        ])
        .assert()
        .success();

    let processed = image::open(&output_path).unwrap();
    let (similarity, psnr) = compare_with_original(&processed, &original_fire);

    let reconstructed = overlay_on_background(&processed, black_bg);
    save_test_images(
        "translucent_recovery",
        "black_non_strict_single_auto",
        &processed,
        &reconstructed,
    );

    println!(
        "Fire on black (non-strict, single auto) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    assert!(
        similarity > 85.0,
        "Similarity {:.2}% is too low",
        similarity
    );
}

#[test]
fn test_fire_on_black_strict_single_auto() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();

    let original_fire = load_fire_image();
    let black_bg = [0, 0, 0];
    let fire_on_black = create_composited_image(&original_fire, black_bg);

    let composited_path = temp_dir.path().join("fire_on_black.png");
    fire_on_black.save(&composited_path).unwrap();

    // Run bgone in strict mode with single auto
    let output_path = temp_dir.path().join("output.png");
    Command::cargo_bin("bgone")
        .unwrap()
        .args([
            composited_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "--strict",
            "--fg",
            "auto",
            "--bg",
            "000000",
        ])
        .assert()
        .success();

    let processed = image::open(&output_path).unwrap();
    let (similarity, psnr) = compare_with_original(&processed, &original_fire);

    let reconstructed = overlay_on_background(&processed, black_bg);
    save_test_images(
        "translucent_recovery",
        "black_strict_single_auto",
        &processed,
        &reconstructed,
    );

    println!(
        "Fire on black (strict, single auto) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    // With strict mode and only one auto color, we expect poor similarity
    // as the complex fire gradients cannot be represented with a single color
    assert!(
        similarity < 75.0,
        "Similarity {:.2}% is too high for single-color strict mode (expected poor recovery)",
        similarity
    );
}

#[test]
fn test_fire_on_black_strict_mixed() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();

    let original_fire = load_fire_image();
    let black_bg = [0, 0, 0];
    let fire_on_black = create_composited_image(&original_fire, black_bg);

    let composited_path = temp_dir.path().join("fire_on_black.png");
    fire_on_black.save(&composited_path).unwrap();

    // Run bgone in strict mode with white + 2 auto colors
    let output_path = temp_dir.path().join("output.png");
    Command::cargo_bin("bgone")
        .unwrap()
        .args([
            composited_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "--strict",
            "--fg",
            "fff",
            "--fg",
            "auto",
            "--fg",
            "auto",
            "--bg",
            "000000",
        ])
        .assert()
        .success();

    let processed = image::open(&output_path).unwrap();
    let (similarity, psnr) = compare_with_original(&processed, &original_fire);

    let reconstructed = overlay_on_background(&processed, black_bg);
    save_test_images(
        "translucent_recovery",
        "black_strict_mixed",
        &processed,
        &reconstructed,
    );

    println!(
        "Fire on black (strict, white + 2 auto) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    // With white + 2 auto colors, we expect excellent similarity
    assert!(
        similarity > 95.0,
        "Similarity {:.2}% is too low",
        similarity
    );
}

// White background tests

#[test]
fn test_fire_on_white_non_strict() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();

    let original_fire = load_fire_image();
    let white_bg = [255, 255, 255];
    let fire_on_white = create_composited_image(&original_fire, white_bg);

    let composited_path = temp_dir.path().join("fire_on_white.png");
    fire_on_white.save(&composited_path).unwrap();

    let output_path = temp_dir.path().join("output.png");
    Command::cargo_bin("bgone")
        .unwrap()
        .args([
            composited_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "--bg",
            "ffffff",
        ])
        .assert()
        .success();

    let processed = image::open(&output_path).unwrap();
    let (similarity, psnr) = compare_with_original(&processed, &original_fire);

    let reconstructed = overlay_on_background(&processed, white_bg);
    save_test_images(
        "translucent_recovery",
        "white_non_strict",
        &processed,
        &reconstructed,
    );

    println!(
        "Fire on white (non-strict) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    assert!(
        similarity > 90.0,
        "Similarity {:.2}% is too low",
        similarity
    );
}

#[test]
fn test_fire_on_white_non_strict_single_auto() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();

    let original_fire = load_fire_image();
    let white_bg = [255, 255, 255];
    let fire_on_white = create_composited_image(&original_fire, white_bg);

    let composited_path = temp_dir.path().join("fire_on_white.png");
    fire_on_white.save(&composited_path).unwrap();

    let output_path = temp_dir.path().join("output.png");
    Command::cargo_bin("bgone")
        .unwrap()
        .args([
            composited_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "--fg",
            "auto",
            "--bg",
            "ffffff",
        ])
        .assert()
        .success();

    let processed = image::open(&output_path).unwrap();
    let (similarity, psnr) = compare_with_original(&processed, &original_fire);

    let reconstructed = overlay_on_background(&processed, white_bg);
    save_test_images(
        "translucent_recovery",
        "white_non_strict_single_auto",
        &processed,
        &reconstructed,
    );

    println!(
        "Fire on white (non-strict, single auto) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    assert!(
        similarity > 65.0 && similarity < 85.0,
        "Similarity {:.2}% is out of expected range (65-85%)",
        similarity
    );
}

#[test]
fn test_fire_on_white_strict_single_auto() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();

    let original_fire = load_fire_image();
    let white_bg = [255, 255, 255];
    let fire_on_white = create_composited_image(&original_fire, white_bg);

    let composited_path = temp_dir.path().join("fire_on_white.png");
    fire_on_white.save(&composited_path).unwrap();

    let output_path = temp_dir.path().join("output.png");
    Command::cargo_bin("bgone")
        .unwrap()
        .args([
            composited_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "--strict",
            "--fg",
            "auto",
            "--bg",
            "ffffff",
        ])
        .assert()
        .success();

    let processed = image::open(&output_path).unwrap();
    let (similarity, psnr) = compare_with_original(&processed, &original_fire);

    let reconstructed = overlay_on_background(&processed, white_bg);
    save_test_images(
        "translucent_recovery",
        "white_strict_single_auto",
        &processed,
        &reconstructed,
    );

    println!(
        "Fire on white (strict, single auto) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    // With strict mode and only one auto color, we expect poor similarity
    // as the complex fire gradients cannot be represented with a single color
    assert!(
        similarity < 60.0,
        "Similarity {:.2}% is too high for single-color strict mode (expected poor recovery)",
        similarity
    );
}

#[test]
fn test_fire_on_white_strict_mixed() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();

    let original_fire = load_fire_image();
    let white_bg = [255, 255, 255];
    let fire_on_white = create_composited_image(&original_fire, white_bg);

    let composited_path = temp_dir.path().join("fire_on_white.png");
    fire_on_white.save(&composited_path).unwrap();

    // Run bgone in strict mode with black + 2 auto colors
    let output_path = temp_dir.path().join("output.png");
    Command::cargo_bin("bgone")
        .unwrap()
        .args([
            composited_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "--strict",
            "--fg",
            "000",
            "--fg",
            "auto",
            "--fg",
            "auto",
            "--bg",
            "ffffff",
        ])
        .assert()
        .success();

    let processed = image::open(&output_path).unwrap();
    let (similarity, psnr) = compare_with_original(&processed, &original_fire);

    let reconstructed = overlay_on_background(&processed, white_bg);
    save_test_images(
        "translucent_recovery",
        "white_strict_mixed",
        &processed,
        &reconstructed,
    );

    println!(
        "Fire on white (strict, black + 2 auto) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    // With black + 2 auto colors, we expect good similarity
    // Slightly lower than black background due to color contrast
    assert!(
        similarity > 85.0,
        "Similarity {:.2}% is too low",
        similarity
    );
}

// Colored background tests

#[test]
fn test_fire_on_colored_non_strict() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();

    let original_fire = load_fire_image();
    let colored_bg = [100, 150, 200]; // Light blue
    let fire_on_colored = create_composited_image(&original_fire, colored_bg);

    let composited_path = temp_dir.path().join("fire_on_colored.png");
    fire_on_colored.save(&composited_path).unwrap();

    let output_path = temp_dir.path().join("output.png");
    Command::cargo_bin("bgone")
        .unwrap()
        .args([
            composited_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "--bg",
            "6496c8",
        ])
        .assert()
        .success();

    let processed = image::open(&output_path).unwrap();
    let (similarity, psnr) = compare_with_original(&processed, &original_fire);

    let reconstructed = overlay_on_background(&processed, colored_bg);
    save_test_images(
        "translucent_recovery",
        "colored_non_strict",
        &processed,
        &reconstructed,
    );

    println!(
        "Fire on colored (non-strict) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    assert!(
        similarity > 90.0,
        "Similarity {:.2}% is too low",
        similarity
    );
}

#[test]
fn test_fire_on_colored_non_strict_single_auto() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();

    let original_fire = load_fire_image();
    let colored_bg = [100, 150, 200];
    let fire_on_colored = create_composited_image(&original_fire, colored_bg);

    let composited_path = temp_dir.path().join("fire_on_colored.png");
    fire_on_colored.save(&composited_path).unwrap();

    let output_path = temp_dir.path().join("output.png");
    Command::cargo_bin("bgone")
        .unwrap()
        .args([
            composited_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "--fg",
            "auto",
            "--bg",
            "6496c8",
        ])
        .assert()
        .success();

    let processed = image::open(&output_path).unwrap();
    let (similarity, psnr) = compare_with_original(&processed, &original_fire);

    let reconstructed = overlay_on_background(&processed, colored_bg);
    save_test_images(
        "translucent_recovery",
        "colored_non_strict_single_auto",
        &processed,
        &reconstructed,
    );

    println!(
        "Fire on colored (non-strict, single auto) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    assert!(
        similarity > 65.0 && similarity < 85.0,
        "Similarity {:.2}% is out of expected range (65-85%)",
        similarity
    );
}

#[test]
fn test_fire_on_colored_strict_single_auto() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();

    let original_fire = load_fire_image();
    let colored_bg = [100, 150, 200];
    let fire_on_colored = create_composited_image(&original_fire, colored_bg);

    let composited_path = temp_dir.path().join("fire_on_colored.png");
    fire_on_colored.save(&composited_path).unwrap();

    let output_path = temp_dir.path().join("output.png");
    Command::cargo_bin("bgone")
        .unwrap()
        .args([
            composited_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "--strict",
            "--fg",
            "auto",
            "--bg",
            "6496c8",
        ])
        .assert()
        .success();

    let processed = image::open(&output_path).unwrap();
    let (similarity, psnr) = compare_with_original(&processed, &original_fire);

    let reconstructed = overlay_on_background(&processed, colored_bg);
    save_test_images(
        "translucent_recovery",
        "colored_strict_single_auto",
        &processed,
        &reconstructed,
    );

    println!(
        "Fire on colored (strict, single auto) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    // With strict mode and only one auto color, we expect poor similarity
    // as the complex fire gradients cannot be represented with a single color
    assert!(
        similarity < 60.0,
        "Similarity {:.2}% is too high for single-color strict mode (expected poor recovery)",
        similarity
    );
}

#[test]
fn test_fire_on_colored_strict_mixed() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();

    let original_fire = load_fire_image();
    let colored_bg = [100, 150, 200];
    let fire_on_colored = create_composited_image(&original_fire, colored_bg);

    let composited_path = temp_dir.path().join("fire_on_colored.png");
    fire_on_colored.save(&composited_path).unwrap();

    // Run bgone in strict mode with black + 2 auto colors
    let output_path = temp_dir.path().join("output.png");
    Command::cargo_bin("bgone")
        .unwrap()
        .args([
            composited_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "--strict",
            "--fg",
            "000",
            "--fg",
            "auto",
            "--fg",
            "auto",
            "--bg",
            "6496c8",
        ])
        .assert()
        .success();

    let processed = image::open(&output_path).unwrap();
    let (similarity, psnr) = compare_with_original(&processed, &original_fire);

    let reconstructed = overlay_on_background(&processed, colored_bg);
    save_test_images(
        "translucent_recovery",
        "colored_strict_mixed",
        &processed,
        &reconstructed,
    );

    println!(
        "Fire on colored (strict, black + 2 auto) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    // With black + 2 auto colors, we expect good similarity
    assert!(
        similarity > 85.0,
        "Similarity {:.2}% is too low",
        similarity
    );
}
