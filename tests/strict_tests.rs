mod common;

use assert_cmd::Command;
use common::{
    calculate_psnr, calculate_similarity_percentage, ensure_output_dir, overlay_on_background,
    save_test_images,
};
use predicates;
use tempfile::TempDir;

#[test]
fn test_square_removal() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/square.png",
        output_path.to_str().unwrap(),
        "--strict",
        "--fg",
        "#ff0000",
        "--bg",
        "#000000",
    ]);

    cmd.assert().success();

    // Load images
    let original = image::open("tests/inputs/square.png").unwrap();
    let processed = image::open(&output_path).unwrap();

    // Overlay processed image back on black background
    let reconstructed = overlay_on_background(&processed, [0, 0, 0]);

    // Save for inspection
    save_test_images("strict", "square", &processed, &reconstructed);

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
        "Similarity {:.4}% is too low",
        similarity
    );
    assert!(
        psnr > 50.0,
        "PSNR {:.2} dB is too low (expected > 50)",
        psnr
    );
}

#[test]
fn test_circle_gradient_removal() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/circle-gradient.png",
        output_path.to_str().unwrap(),
        "--strict",
        "--fg",
        "#ff0000",
        "--bg",
        "#ffffff",
    ]);

    cmd.assert().success();

    // Load images
    let original = image::open("tests/inputs/circle-gradient.png").unwrap();
    let processed = image::open(&output_path).unwrap();

    // Overlay processed image back on white background
    let reconstructed = overlay_on_background(&processed, [255, 255, 255]);

    // Save for inspection
    save_test_images("strict", "circle_gradient", &processed, &reconstructed);

    // Compare with original
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);

    println!(
        "Red gradient on white - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );

    // Should be nearly identical
    assert!(
        similarity > 99.0,
        "Similarity {:.4}% is too low",
        similarity
    );
    assert!(
        psnr > 50.0,
        "PSNR {:.2} dB is too low (expected > 50)",
        psnr
    );
}

#[test]
fn test_rectangles_removal() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/rectangles.png",
        output_path.to_str().unwrap(),
        "--strict",
        "--fg",
        "#ff0000", // red
        "#00ff00", // green
        "#ffff00", // yellow
        "--bg",
        "#0000ff",
    ]);

    cmd.assert().success();

    // Load images
    let original = image::open("tests/inputs/rectangles.png").unwrap();
    let processed = image::open(&output_path).unwrap();

    // Overlay processed image back on blue background
    let reconstructed = overlay_on_background(&processed, [0, 0, 255]);

    // Save for inspection
    save_test_images("strict", "rectangles", &processed, &reconstructed);

    // Compare with original
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);

    println!(
        "Multicolor on blue - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );

    // Multiple colors with blending is the hardest case
    assert!(
        similarity > 99.0,
        "Similarity {:.2}% is too low",
        similarity
    );
    assert!(
        psnr > 50.0,
        "PSNR {:.2} dB is too low (expected > 50)",
        psnr
    );
}

#[test]
fn test_auto_background_detection() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone without specifying background (should auto-detect black)
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/square.png",
        output_path.to_str().unwrap(),
        "--strict",
        "--fg",
        "#ff0000",
    ]);

    cmd.assert().success();

    // Should detect black background automatically
    let original = image::open("tests/inputs/square.png").unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [0, 0, 0]);

    // Save for inspection
    save_test_images("strict", "auto_detect_black", &processed, &reconstructed);

    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    assert!(
        similarity > 99.0,
        "Auto-detection failed: similarity {:.4}%",
        similarity
    );
}

#[test]
fn test_square_gradient_with_known_colors() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with known cyan and magenta - should preserve transparency
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/square-gradient.png",
        output_path.to_str().unwrap(),
        "--strict",
        "--fg",
        "#00ffff",
        "#ff00ff",
        "--bg",
        "#14191e",
    ]);

    cmd.assert().success();

    // Load and save images for inspection
    let original = image::open("tests/inputs/square-gradient.png").unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [20, 25, 30]);

    save_test_images(
        "strict",
        "square_gradient_known",
        &processed,
        &reconstructed,
    );

    // Check reconstruction quality
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);
    println!(
        "Square gradient (known colors) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );

    assert!(
        similarity > 99.0,
        "Reconstruction quality too low: {:.4}%",
        similarity
    );
}

#[test]
fn test_strict_mode_requires_fg() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test: No foreground colors specified in strict mode - should fail
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/square.png",
        output_path.to_str().unwrap(),
        "--strict",
    ]);

    cmd.assert()
        .failure()
        .stderr(predicates::str::contains("strict mode"));
}
