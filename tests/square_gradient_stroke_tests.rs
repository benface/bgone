mod common;

use assert_cmd::Command;
use common::{
    calculate_psnr, calculate_similarity_percentage, ensure_output_dir, overlay_on_background,
    save_test_images,
};
use tempfile::TempDir;

/// Test square-gradient-stroke.png in all four modes to showcase
/// how different approaches produce different outputs that all perfectly reconstruct

#[test]
fn test_square_gradient_stroke_non_strict_no_fg() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone in non-strict mode without any foreground colors
    // This should optimize for maximum transparency
    Command::cargo_bin("bgone")
        .unwrap()
        .args(&[
            "tests/inputs/square-gradient-stroke.png",
            output_path.to_str().unwrap(),
            "--bg",
            "14191e", // dark background (20, 25, 30)
        ])
        .assert()
        .success();

    // Load images
    let original = image::open("tests/inputs/square-gradient-stroke.png").unwrap();
    let processed = image::open(&output_path).unwrap();

    // Reconstruct by overlaying on background
    let background_color = [0x14, 0x19, 0x1e]; // (20, 25, 30)
    let reconstructed = overlay_on_background(&processed, background_color);

    // Save outputs for inspection
    save_test_images(
        "square_gradient_stroke",
        "non_strict_no_fg",
        &processed,
        &reconstructed,
    );

    // Verify perfect reconstruction
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);

    println!(
        "Non-strict (no fg) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    assert!(
        similarity > 99.0,
        "Similarity {:.2}% is too low",
        similarity
    );
}

#[test]
fn test_square_gradient_stroke_non_strict_with_fg() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone in non-strict mode with three known foreground colors
    // Colors close to these will use high opacity, others will be flexible
    Command::cargo_bin("bgone")
        .unwrap()
        .args(&[
            "tests/inputs/square-gradient-stroke.png",
            output_path.to_str().unwrap(),
            "--fg",
            "00ffff", // cyan
            "--fg",
            "ff00ff", // magenta
            "--fg",
            "a7b511", // actual stroke color
            "--bg",
            "14191e", // dark background (20, 25, 30)
        ])
        .assert()
        .success();

    // Load images
    let original = image::open("tests/inputs/square-gradient-stroke.png").unwrap();
    let processed = image::open(&output_path).unwrap();

    // Reconstruct by overlaying on background
    let background_color = [0x14, 0x19, 0x1e]; // (20, 25, 30)
    let reconstructed = overlay_on_background(&processed, background_color);

    // Save outputs for inspection
    save_test_images(
        "square_gradient_stroke",
        "non_strict_with_fg",
        &processed,
        &reconstructed,
    );

    // Verify perfect reconstruction
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);

    println!(
        "Non-strict (with fg) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    assert!(
        similarity > 99.0,
        "Similarity {:.2}% is too low",
        similarity
    );
}

#[test]
fn test_square_gradient_stroke_strict_known_colors() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone in strict mode with three known foreground colors
    // This restricts the unmixing to only these specific colors
    Command::cargo_bin("bgone")
        .unwrap()
        .args(&[
            "tests/inputs/square-gradient-stroke.png",
            output_path.to_str().unwrap(),
            "--strict",
            "--fg",
            "00ffff", // cyan
            "--fg",
            "ff00ff", // magenta
            "--fg",
            "a7b511", // actual stroke color
            "--bg",
            "14191e", // dark background (20, 25, 30)
        ])
        .assert()
        .success();

    // Load images
    let original = image::open("tests/inputs/square-gradient-stroke.png").unwrap();
    let processed = image::open(&output_path).unwrap();

    // Reconstruct by overlaying on background
    let background_color = [0x14, 0x19, 0x1e]; // (20, 25, 30)
    let reconstructed = overlay_on_background(&processed, background_color);

    // Save outputs for inspection
    save_test_images(
        "square_gradient_stroke",
        "strict_known",
        &processed,
        &reconstructed,
    );

    // Verify perfect reconstruction
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);

    println!(
        "Strict (known colors) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    assert!(
        similarity > 99.0,
        "Similarity {:.2}% is too low",
        similarity
    );
}

#[test]
fn test_square_gradient_stroke_strict_auto_colors() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone in strict mode with automatic color deduction
    // This will find the optimal 3 colors automatically
    Command::cargo_bin("bgone")
        .unwrap()
        .args(&[
            "tests/inputs/square-gradient-stroke.png",
            output_path.to_str().unwrap(),
            "--strict",
            "--fg",
            "auto",
            "--fg",
            "auto",
            "--fg",
            "auto",
            "--bg",
            "14191e", // dark background (20, 25, 30)
        ])
        .assert()
        .success();

    // Load images
    let original = image::open("tests/inputs/square-gradient-stroke.png").unwrap();
    let processed = image::open(&output_path).unwrap();

    // Reconstruct by overlaying on background
    let background_color = [0x14, 0x19, 0x1e]; // (20, 25, 30)
    let reconstructed = overlay_on_background(&processed, background_color);

    // Save outputs for inspection
    save_test_images(
        "square_gradient_stroke",
        "strict_auto",
        &processed,
        &reconstructed,
    );

    // Verify perfect reconstruction
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);

    println!(
        "Strict (auto colors) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    assert!(
        similarity > 99.0,
        "Similarity {:.2}% is too low",
        similarity
    );
}

#[test]
fn test_square_gradient_stroke_non_strict_with_fg_high_threshold() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone in non-strict mode with three known foreground colors and higher threshold
    // This should make more pixels be considered "close enough" to use the known colors
    Command::cargo_bin("bgone")
        .unwrap()
        .args(&[
            "tests/inputs/square-gradient-stroke.png",
            output_path.to_str().unwrap(),
            "--fg",
            "00ffff", // cyan
            "--fg",
            "ff00ff", // magenta
            "--fg",
            "a7b511", // actual stroke color
            "--bg",
            "14191e", // dark background (20, 25, 30)
            "--threshold",
            "0.5", // 50% threshold instead of default 5%
        ])
        .assert()
        .success();

    // Load images
    let original = image::open("tests/inputs/square-gradient-stroke.png").unwrap();
    let processed = image::open(&output_path).unwrap();

    // Reconstruct by overlaying on background
    let background_color = [0x14, 0x19, 0x1e]; // (20, 25, 30)
    let reconstructed = overlay_on_background(&processed, background_color);

    // Save outputs for inspection
    save_test_images(
        "square_gradient_stroke",
        "non_strict_with_fg_high_threshold",
        &processed,
        &reconstructed,
    );

    // Verify perfect reconstruction
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);

    println!(
        "Non-strict (high threshold) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );
    assert!(
        similarity > 99.0,
        "Similarity {:.2}% is too low",
        similarity
    );
}
