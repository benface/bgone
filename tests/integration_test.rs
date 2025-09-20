use assert_cmd::Command;
use bgone::testing::{calculate_psnr, calculate_similarity_percentage, overlay_on_background};
use std::fs;
use tempfile::TempDir;

// Set to true to save test outputs for inspection
const SAVE_TEST_OUTPUTS: bool = true;

fn ensure_output_dir() {
    if SAVE_TEST_OUTPUTS {
        fs::create_dir_all("tests/outputs").unwrap();
    }
}

fn save_test_images(
    test_name: &str,
    processed: &image::DynamicImage,
    reconstructed: &image::DynamicImage,
) {
    if SAVE_TEST_OUTPUTS {
        let processed_path = format!("tests/outputs/{}_processed.png", test_name);
        let reconstructed_path = format!("tests/outputs/{}_reconstructed.png", test_name);

        processed.save(&processed_path).unwrap();
        reconstructed.save(&reconstructed_path).unwrap();

        println!("  Saved: {}", processed_path);
        println!("  Saved: {}", reconstructed_path);
    }
}

#[test]
fn test_red_on_black_removal() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/red_on_black.png",
        output_path.to_str().unwrap(),
        "--fg",
        "#ff0000",
        "--bg",
        "#000000",
    ]);

    cmd.assert().success();

    // Load images
    let original = image::open("tests/fixtures/red_on_black.png").unwrap();
    let processed = image::open(&output_path).unwrap();

    // Overlay processed image back on black background
    let reconstructed = overlay_on_background(&processed, [0, 0, 0]);

    // Save for inspection
    save_test_images("red_on_black", &processed, &reconstructed);

    // Compare with original
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);

    println!(
        "Red on black - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );

    // Should be nearly identical
    assert!(
        similarity > 99.0,
        "Similarity {:.2}% is too low",
        similarity
    );
    assert!(psnr > 40.0, "PSNR {:.2} dB is too low", psnr);
}

#[test]
fn test_red_gradient_on_white_removal() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/red_gradient_on_white.png",
        output_path.to_str().unwrap(),
        "--fg",
        "#ff0000",
        "--bg",
        "#ffffff",
    ]);

    cmd.assert().success();

    // Load images
    let original = image::open("tests/fixtures/red_gradient_on_white.png").unwrap();
    let processed = image::open(&output_path).unwrap();

    // Overlay processed image back on white background
    let reconstructed = overlay_on_background(&processed, [255, 255, 255]);

    // Save for inspection
    save_test_images("red_gradient_on_white", &processed, &reconstructed);

    // Compare with original
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);

    println!(
        "Red gradient on white - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );

    // Gradients are harder, so we allow a bit more tolerance
    assert!(
        similarity > 95.0,
        "Similarity {:.2}% is too low",
        similarity
    );
    assert!(psnr > 30.0, "PSNR {:.2} dB is too low", psnr);
}

#[test]
fn test_multicolor_on_blue_removal() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone with multiple foreground colors
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/multicolor_on_blue.png",
        output_path.to_str().unwrap(),
        "--fg",
        "#ff0000",
        "#00ff00",
        "#ffff00",
        "--bg",
        "#0000ff",
    ]);

    cmd.assert().success();

    // Load images
    let original = image::open("tests/fixtures/multicolor_on_blue.png").unwrap();
    let processed = image::open(&output_path).unwrap();

    // Overlay processed image back on blue background
    let reconstructed = overlay_on_background(&processed, [0, 0, 255]);

    // Save for inspection
    save_test_images("multicolor_on_blue", &processed, &reconstructed);

    // Compare with original
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);

    println!(
        "Multicolor on blue - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );

    // Multiple colors with blending is the hardest case
    assert!(
        similarity > 90.0,
        "Similarity {:.2}% is too low",
        similarity
    );
    assert!(psnr > 25.0, "PSNR {:.2} dB is too low", psnr);
}

#[test]
fn test_auto_background_detection() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone without specifying background (should auto-detect black)
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/red_on_black.png",
        output_path.to_str().unwrap(),
        "--fg",
        "#ff0000",
    ]);

    cmd.assert().success();

    // Should detect black background automatically
    let original = image::open("tests/fixtures/red_on_black.png").unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [0, 0, 0]);

    // Save for inspection
    save_test_images("auto_detect_black", &processed, &reconstructed);

    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    assert!(
        similarity > 99.0,
        "Auto-detection failed: similarity {:.2}%",
        similarity
    );
}
