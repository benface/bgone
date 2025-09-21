use assert_cmd::Command;
use bgone::color::ForegroundColorSpec;
use bgone::deduce::deduce_unknown_colors;
use bgone::testing::{calculate_psnr, calculate_similarity_percentage, overlay_on_background};
use bgone::unmix::{compute_result_color, unmix_colors};
use predicates;
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

    // Run bgone with only the actual foreground colors used to create the image
    // The bottom row should be detected as 75% opacity of the top row colors
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/multicolor_on_blue.png",
        output_path.to_str().unwrap(),
        "--fg",
        "#ff0000", // red
        "#00ff00", // green
        "#ffff00", // yellow
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

#[test]
fn test_color_deduction_single_unknown() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone with one known and one unknown color
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/red_with_purple_glow.png",
        output_path.to_str().unwrap(),
        "--fg",
        "#ff0000",
        "auto",
        "--bg",
        "#000000",
    ]);

    cmd.assert().success();

    // Load images
    let original = image::open("tests/fixtures/red_with_purple_glow.png").unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [0, 0, 0]);

    // Save for inspection
    save_test_images("color_deduction_single", &processed, &reconstructed);

    // Verify reconstruction quality
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);

    println!(
        "Color deduction (1 unknown) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );

    assert!(
        similarity > 99.9,
        "Color deduction quality too low: {:.2}%",
        similarity
    );
    assert!(psnr > 38.0, "PSNR {:.2} dB is too low", psnr);
}

#[test]
fn test_color_deduction_all_unknown() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Create a simple test image with two distinct colors
    let test_image_path = temp_dir.path().join("test_input.png");
    create_two_color_test_image(&test_image_path);

    // Run bgone with all unknown colors
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        test_image_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        "--fg",
        "auto",
        "auto",
        "--bg",
        "#ffffff",
    ]);

    let output = cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("Deduced 2 unknown colors"))
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);

    // Extract and verify deduced colors are distinct
    if let Some(line) = output_str
        .lines()
        .find(|l| l.contains("Deduced 2 unknown colors"))
    {
        let colors: Vec<&str> = line
            .split(':')
            .nth(1)
            .unwrap()
            .trim()
            .split_whitespace()
            .collect();
        assert_eq!(colors.len(), 2, "Should deduce exactly 2 colors");
        assert_ne!(
            colors[0], colors[1],
            "Deduced colors should be different, got: {} and {}",
            colors[0], colors[1]
        );

        // Both colors should not be black
        for color in &colors {
            assert_ne!(
                color, &"#000000",
                "Should not deduce black for colored gradients"
            );
        }
    } else {
        panic!("Could not find deduced colors in output");
    }

    // Load images
    let original = image::open(&test_image_path).unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [255, 255, 255]);

    // Save for inspection
    save_test_images("color_deduction_all_auto", &processed, &reconstructed);

    // Verify reconstruction quality
    let similarity = calculate_similarity_percentage(&original, &reconstructed);

    println!(
        "Color deduction (all unknown) - Similarity: {:.2}%",
        similarity
    );

    // May be lower accuracy when all colors are unknown
    assert!(
        similarity > 90.0,
        "Color deduction quality too low: {:.2}%",
        similarity
    );
}

/// Create a test image with two distinct foreground colors on white background
fn create_two_color_test_image(path: &std::path::Path) {
    use image::{Rgba, RgbaImage};

    let mut img = RgbaImage::new(100, 100);

    // Fill with white background
    for pixel in img.pixels_mut() {
        *pixel = Rgba([255, 255, 255, 255]);
    }

    // Add blue rectangle on left
    for y in 25..75 {
        for x in 10..40 {
            img.put_pixel(x, y, Rgba([0, 0, 255, 255]));
        }
    }

    // Add green rectangle on right
    for y in 25..75 {
        for x in 60..90 {
            img.put_pixel(x, y, Rgba([0, 255, 0, 255]));
        }
    }

    img.save(path).unwrap();
}

#[test]
fn test_color_deduction_error_cases() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test: No foreground colors specified
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/red_on_black.png",
        output_path.to_str().unwrap(),
    ]);

    cmd.assert()
        .failure()
        .stderr(predicates::str::contains("required"));

    // Test: Only 'auto' specified without any known colors - should work
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/red_on_black.png",
        output_path.to_str().unwrap(),
        "--fg",
        "auto",
    ]);

    cmd.assert().success();
}

#[test]
fn test_mixed_known_and_unknown_colors() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with multiple knowns and unknowns mixed
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/multicolor_on_blue.png",
        output_path.to_str().unwrap(),
        "--fg",
        "#ff0000", // Red (known)
        "auto",    // Unknown
        "#ffff00", // Yellow (known)
        "--bg",
        "#0000ff",
    ]);

    cmd.assert().success();

    // Verify output was created
    assert!(output_path.exists());
}

#[test]
fn test_multiple_unknown_colors_convergence() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test that multiple unknowns don't all converge to black
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/three_gradients_on_white.png",
        output_path.to_str().unwrap(),
        "--fg",
        "auto",
        "auto",
        "auto",
        "--bg",
        "#ffffff",
    ]);

    let output = cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("Deduced 3 unknown colors"))
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);

    // Extract deduced colors from output
    if let Some(line) = output_str
        .lines()
        .find(|l| l.contains("Deduced 3 unknown colors"))
    {
        let colors: Vec<&str> = line
            .split(':')
            .nth(1)
            .unwrap()
            .trim()
            .split_whitespace()
            .collect();
        assert_eq!(colors.len(), 3, "Should deduce exactly 3 colors");

        // Should deduce approximately red, green, and blue (allowing some tolerance)
        let mut found_red = false;
        let mut found_green = false;
        let mut found_blue = false;

        for color in &colors {
            if color.starts_with("#") && color.len() == 7 {
                let r = u8::from_str_radix(&color[1..3], 16).unwrap();
                let g = u8::from_str_radix(&color[3..5], 16).unwrap();
                let b = u8::from_str_radix(&color[5..7], 16).unwrap();

                // Check if this is approximately red (high R, low G and B)
                if r > 200 && g < 50 && b < 50 {
                    found_red = true;
                }
                // Check if this is approximately green (high G, low R and B)
                else if g > 200 && r < 50 && b < 50 {
                    found_green = true;
                }
                // Check if this is approximately blue (high B, low R and G)
                else if b > 200 && r < 50 && g < 50 {
                    found_blue = true;
                }
            }
        }

        assert!(found_red, "Should deduce a red color, got: {:?}", colors);
        assert!(
            found_green,
            "Should deduce a green color, got: {:?}",
            colors
        );
        assert!(found_blue, "Should deduce a blue color, got: {:?}", colors);
    } else {
        panic!("Could not find deduced colors in output");
    }

    // Load and save images for inspection
    let original = image::open("tests/fixtures/three_gradients_on_white.png").unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [255, 255, 255]);

    save_test_images("three_gradients_deduction", &processed, &reconstructed);

    // Also check reconstruction quality
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    println!("Three gradients deduction - Similarity: {:.2}%", similarity);
}

#[test]
fn test_three_gradients_with_known_red() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with red as known color - should deduce green and blue
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/three_gradients_on_white.png",
        output_path.to_str().unwrap(),
        "--fg",
        "#ff0000",
        "auto",
        "auto",
        "--bg",
        "#ffffff",
    ]);

    let output = cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("Deduced 2 unknown colors"))
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);

    // Extract deduced colors
    if let Some(line) = output_str
        .lines()
        .find(|l| l.contains("Deduced 2 unknown colors"))
    {
        let colors: Vec<&str> = line
            .split(':')
            .nth(1)
            .unwrap()
            .trim()
            .split_whitespace()
            .collect();

        let mut found_green = false;
        let mut found_blue = false;

        for color in &colors {
            if color.starts_with("#") && color.len() == 7 {
                let r = u8::from_str_radix(&color[1..3], 16).unwrap();
                let g = u8::from_str_radix(&color[3..5], 16).unwrap();
                let b = u8::from_str_radix(&color[5..7], 16).unwrap();

                if g > 200 && r < 50 && b < 50 {
                    found_green = true;
                } else if b > 200 && r < 50 && g < 50 {
                    found_blue = true;
                }
            }
        }

        assert!(
            found_green,
            "Should deduce green when red is known, got: {:?}",
            colors
        );
        assert!(
            found_blue,
            "Should deduce blue when red is known, got: {:?}",
            colors
        );
    }
}

#[test]
fn test_three_gradients_with_known_green() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with green as known color - should deduce red and blue
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/three_gradients_on_white.png",
        output_path.to_str().unwrap(),
        "--fg",
        "#00ff00",
        "auto",
        "auto",
        "--bg",
        "#ffffff",
    ]);

    let output = cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("Deduced 2 unknown colors"))
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);

    // Extract deduced colors
    if let Some(line) = output_str
        .lines()
        .find(|l| l.contains("Deduced 2 unknown colors"))
    {
        let colors: Vec<&str> = line
            .split(':')
            .nth(1)
            .unwrap()
            .trim()
            .split_whitespace()
            .collect();

        let mut found_red = false;
        let mut found_blue = false;

        for color in &colors {
            if color.starts_with("#") && color.len() == 7 {
                let r = u8::from_str_radix(&color[1..3], 16).unwrap();
                let g = u8::from_str_radix(&color[3..5], 16).unwrap();
                let b = u8::from_str_radix(&color[5..7], 16).unwrap();

                if r > 200 && g < 50 && b < 50 {
                    found_red = true;
                } else if b > 200 && r < 50 && g < 50 {
                    found_blue = true;
                }
            }
        }

        assert!(
            found_red,
            "Should deduce red when green is known, got: {:?}",
            colors
        );
        assert!(
            found_blue,
            "Should deduce blue when green is known, got: {:?}",
            colors
        );
    }
}

#[test]
fn test_three_gradients_with_known_blue() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with blue as known color - should deduce red and green
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/three_gradients_on_white.png",
        output_path.to_str().unwrap(),
        "--fg",
        "#0000ff",
        "auto",
        "auto",
        "--bg",
        "#ffffff",
    ]);

    let output = cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("Deduced 2 unknown colors"))
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);

    // Extract deduced colors
    if let Some(line) = output_str
        .lines()
        .find(|l| l.contains("Deduced 2 unknown colors"))
    {
        let colors: Vec<&str> = line
            .split(':')
            .nth(1)
            .unwrap()
            .trim()
            .split_whitespace()
            .collect();

        let mut found_red = false;
        let mut found_green = false;

        for color in &colors {
            if color.starts_with("#") && color.len() == 7 {
                let r = u8::from_str_radix(&color[1..3], 16).unwrap();
                let g = u8::from_str_radix(&color[3..5], 16).unwrap();
                let b = u8::from_str_radix(&color[5..7], 16).unwrap();

                if r > 200 && g < 50 && b < 50 {
                    found_red = true;
                } else if g > 200 && r < 50 && b < 50 {
                    found_green = true;
                }
            }
        }

        assert!(
            found_red,
            "Should deduce red when blue is known, got: {:?}",
            colors
        );
        assert!(
            found_green,
            "Should deduce green when blue is known, got: {:?}",
            colors
        );
    }
}

#[test]
fn test_gradient_rect_with_known_colors() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with known cyan and magenta - should preserve transparency
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/gradient_rect_on_dark.png",
        output_path.to_str().unwrap(),
        "--fg",
        "#00ffff",
        "#ff00ff",
        "--bg",
        "#14191e",
    ]);

    cmd.assert().success();

    // Load and save images for inspection
    let original = image::open("tests/fixtures/gradient_rect_on_dark.png").unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [20, 25, 30]);

    save_test_images("gradient_rect_known", &processed, &reconstructed);

    // Check reconstruction quality
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);
    println!(
        "Gradient rect (known colors) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );

    assert!(
        similarity > 99.0,
        "Reconstruction quality too low: {:.2}%",
        similarity
    );
}

#[test]
fn test_gradient_rect_auto_deduction() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with auto color deduction - should maximize opacity
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/gradient_rect_on_dark.png",
        output_path.to_str().unwrap(),
        "--fg",
        "auto",
        "auto",
        "--bg",
        "#14191e",
    ]);

    let output = cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("Deduced 2 unknown colors"))
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);

    // Extract deduced colors
    if let Some(line) = output_str
        .lines()
        .find(|l| l.contains("Deduced 2 unknown colors"))
    {
        let colors: Vec<&str> = line
            .split(':')
            .nth(1)
            .unwrap()
            .trim()
            .split_whitespace()
            .collect();
        println!("Deduced colors for gradient rect: {:?}", colors);

        // The deduced colors should NOT both be black
        let black_count = colors.iter().filter(|&&c| c == "#000000").count();
        assert!(
            black_count < 2,
            "Should not deduce all black colors for gradient, got: {:?}",
            colors
        );
    }

    // Load and save images for inspection
    let original = image::open("tests/fixtures/gradient_rect_on_dark.png").unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [20, 25, 30]);

    save_test_images("gradient_rect_auto", &processed, &reconstructed);

    // Check reconstruction quality
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    println!(
        "Gradient rect (auto deduction) - Similarity: {:.2}%",
        similarity
    );

    // May have lower similarity due to opacity optimization
    assert!(
        similarity > 90.0,
        "Reconstruction quality too low: {:.2}%",
        similarity
    );
}

#[test]
fn test_gradient_rect_with_known_magenta() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with a slightly modified magenta (close to what would be deduced)
    // This ensures the algorithm deduces cyan instead of re-deducing magenta
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/gradient_rect_on_dark.png",
        output_path.to_str().unwrap(),
        "--fg",
        "#fa08fa", // Slightly different from #f805fe that it would normally deduce
        "auto",
        "--bg",
        "#14191e",
    ]);

    let output = cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("Deduced 1 unknown color"))
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);

    // Extract deduced color
    if let Some(line) = output_str
        .lines()
        .find(|l| l.contains("Deduced 1 unknown color"))
    {
        let colors: Vec<&str> = line
            .split(':')
            .nth(1)
            .unwrap()
            .trim()
            .split_whitespace()
            .collect();
        println!("Deduced color when magenta is known: {}", colors[0]);

        // Should deduce a cyan-like color, not magenta
        let color = colors[0];
        assert!(color.starts_with("#"), "Color should be in hex format");

        // Parse the color
        let r = u8::from_str_radix(&color[1..3], 16).unwrap();
        let g = u8::from_str_radix(&color[3..5], 16).unwrap();
        let b = u8::from_str_radix(&color[5..7], 16).unwrap();

        // Check that it's cyan-ish (low red, high green/blue)
        assert!(r < 50, "Red should be low for cyan, got {}", r);
        assert!(g > 100, "Green should be high for cyan, got {}", g);
        assert!(b > 100, "Blue should be high for cyan, got {}", b);

        // Make sure it's NOT magenta (which would have high red, low green, high blue)
        assert!(
            !(r > 200 && g < 50),
            "Should not deduce magenta when magenta is known"
        );
    }
}

#[test]
fn test_gradient_rect_with_known_cyan() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with a slightly modified cyan (close to what would be deduced)
    // This ensures the algorithm deduces magenta instead of re-deducing cyan
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/fixtures/gradient_rect_on_dark.png",
        output_path.to_str().unwrap(),
        "--fg",
        "#109098", // Slightly different from #0e929a that it would normally deduce
        "auto",
        "--bg",
        "#14191e",
    ]);

    let output = cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("Deduced 1 unknown color"))
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);

    // Extract deduced color
    if let Some(line) = output_str
        .lines()
        .find(|l| l.contains("Deduced 1 unknown color"))
    {
        let colors: Vec<&str> = line
            .split(':')
            .nth(1)
            .unwrap()
            .trim()
            .split_whitespace()
            .collect();
        println!("Deduced color when cyan is known: {}", colors[0]);

        // Should deduce a magenta-like color, not cyan
        let color = colors[0];
        assert!(color.starts_with("#"), "Color should be in hex format");

        // Parse the color
        let r = u8::from_str_radix(&color[1..3], 16).unwrap();
        let g = u8::from_str_radix(&color[3..5], 16).unwrap();
        let b = u8::from_str_radix(&color[5..7], 16).unwrap();

        // Check that it's magenta-ish (high red, low green, high blue)
        assert!(r > 200, "Red should be high for magenta, got {}", r);
        assert!(g < 50, "Green should be low for magenta, got {}", g);
        assert!(b > 200, "Blue should be high for magenta, got {}", b);

        // Make sure it's NOT cyan (which would have low red, high green/blue)
        assert!(
            !(r < 50 && g > 100),
            "Should not deduce cyan when cyan is known"
        );
    }
}

#[test]
fn test_opacity_optimization() {
    // Test case: A color that could be achieved with multiple opacity levels
    // Gray (128, 128, 128) could be:
    // - 50% white on black background
    // - 100% gray on black background
    // We should prefer the 100% gray solution for maximum opacity

    let observed = [128, 128, 128];
    let background = [0.0, 0.0, 0.0]; // Black

    // Test 1: When gray is available as a foreground color
    let foreground_colors = vec![
        [1.0, 1.0, 1.0],       // White
        [0.502, 0.502, 0.502], // Gray (very close to 128/255)
    ];

    let result = unmix_colors(observed, &foreground_colors, background);
    println!("Test 1 - Gray available:");
    println!("  Weights: {:?}", result.weights);
    println!("  Alpha: {}", result.alpha);

    // Should strongly prefer the gray color for maximum opacity
    assert!(
        result.weights[1] > 0.9,
        "Should use mostly gray, got weight: {}",
        result.weights[1]
    );
    assert!(
        result.alpha > 0.99,
        "Should have near-full opacity, got: {}",
        result.alpha
    );

    // Test 2: Pink (255, 128, 128) on white background
    // Could be achieved with:
    // - Low opacity red on white
    // - High opacity pink on white
    let observed2 = [255, 128, 128];
    let background2 = [1.0, 1.0, 1.0]; // White
    let foreground_colors2 = vec![
        [1.0, 0.0, 0.0],     // Pure red
        [1.0, 0.502, 0.502], // Pink (close to target)
    ];

    let result2 = unmix_colors(observed2, &foreground_colors2, background2);
    println!("\nTest 2 - Pink on white:");
    println!("  Weights: {:?}", result2.weights);
    println!("  Alpha: {}", result2.alpha);

    // Should prefer pink for maximum opacity
    assert!(
        result2.weights[1] > 0.9,
        "Should use mostly pink, got weight: {}",
        result2.weights[1]
    );

    // Test 3: Color achievable only with blend
    // Orange (255, 128, 0) = mix of red and yellow
    let observed3 = [255, 128, 0];
    let background3 = [0.0, 0.0, 0.0]; // Black
    let foreground_colors3 = vec![
        [1.0, 0.0, 0.0], // Red
        [1.0, 1.0, 0.0], // Yellow
        [0.0, 0.0, 1.0], // Blue (not needed)
    ];

    let result3 = unmix_colors(observed3, &foreground_colors3, background3);
    let (color, _) = compute_result_color(&result3, &foreground_colors3);

    println!("\nTest 3 - Orange from red+yellow:");
    println!("  Weights: {:?}", result3.weights);
    println!("  Alpha: {}", result3.alpha);
    println!(
        "  Result color: [{:.0}, {:.0}, {:.0}]",
        color[0] * 255.0,
        color[1] * 255.0,
        color[2] * 255.0
    );

    // Should use both red and yellow to achieve orange with max opacity
    assert!(
        result3.alpha > 0.99,
        "Should have near-full opacity for orange, got: {}",
        result3.alpha
    );
    assert!(
        result3.weights[0] > 0.3 && result3.weights[0] < 0.7,
        "Red weight should be moderate: {}",
        result3.weights[0]
    );
    assert!(
        result3.weights[1] > 0.3 && result3.weights[1] < 0.7,
        "Yellow weight should be moderate: {}",
        result3.weights[1]
    );
}

#[test]
fn test_pure_color_deduction() {
    use image::{Rgba, RgbaImage};

    // Create a simple red gradient on white background
    let mut img = RgbaImage::new(100, 100);

    // Fill with white
    for pixel in img.pixels_mut() {
        *pixel = Rgba([255, 255, 255, 255]);
    }

    // Add pure red circle with gradient
    let center_x = 50.0;
    let center_y = 50.0;
    let radius = 40.0;

    for y in 0..100 {
        for x in 0..100 {
            let dx = x as f32 - center_x;
            let dy = y as f32 - center_y;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist < radius {
                let alpha = 1.0 - (dist / radius);
                // Blend pure red with white based on alpha
                let r = (255.0 * alpha + 255.0 * (1.0 - alpha)) as u8;
                let g = (0.0 * alpha + 255.0 * (1.0 - alpha)) as u8;
                let b = (0.0 * alpha + 255.0 * (1.0 - alpha)) as u8;

                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }
    }

    let dynamic_img = image::DynamicImage::ImageRgba8(img);

    // Test with single unknown
    let specs = vec![ForegroundColorSpec::Unknown];
    let result = deduce_unknown_colors(&dynamic_img, &specs, [255, 255, 255]).unwrap();

    println!("Deduced color: {:?}", result[0]);

    // The deduced color should be pure red or very close to it
    assert!(
        result[0][0] >= 253,
        "Red channel should be close to 255, got {}",
        result[0][0]
    );
    assert!(
        result[0][1] < 30,
        "Green channel should be close to 0, got {}",
        result[0][1]
    );
    assert!(
        result[0][2] < 20,
        "Blue channel should be close to 0, got {}",
        result[0][2]
    );
}
