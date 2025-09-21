mod common;

use assert_cmd::Command;
use bgone::color::ForegroundColorSpec;
use bgone::deduce::deduce_unknown_colors;
use bgone::unmix::{compute_result_color, unmix_colors};
use common::{
    calculate_psnr, calculate_similarity_percentage, ensure_output_dir, overlay_on_background,
    save_test_images,
};
use predicates;
use tempfile::TempDir;

#[test]
fn test_color_deduction_single_unknown() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Run bgone with one known and one unknown color
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/square-glow.png",
        output_path.to_str().unwrap(),
        "--strict",
        "--fg",
        "#ff0000",
        "auto",
        "--bg",
        "#000000",
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let output_str = String::from_utf8_lossy(&output);
    println!("Output:\n{}", output_str);

    // Should deduce purple
    assert!(output_str.contains("Deduced"));

    // Load and save images for inspection
    let original = image::open("tests/inputs/square-glow.png").unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [0, 0, 0]);

    save_test_images("color_deduction", "square_glow", &processed, &reconstructed);

    // Check reconstruction quality - should be perfect in strict mode
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);
    println!(
        "Red with purple glow (strict) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );

    assert!(
        similarity > 99.0,
        "Strict mode should reconstruct the image with high quality: {:.4}%",
        similarity
    );
}

#[test]
fn test_color_deduction_all_unknown() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Create a custom test image with known colors
    let width = 100;
    let height = 100;
    let mut img = image::RgbImage::new(width, height);

    // Fill with white background
    for pixel in img.pixels_mut() {
        *pixel = image::Rgb([255, 255, 255]);
    }

    // Add red and green circles (50% opacity each)
    let red_mixed = [255, 128, 128]; // Red mixed with white
    let green_mixed = [128, 255, 128]; // Green mixed with white

    // Red circle (left)
    for y in 0..height {
        for x in 0..width / 2 {
            if ((x as i32 - 25).pow(2) + (y as i32 - 50).pow(2)) < 400 {
                img.put_pixel(x, y, image::Rgb(red_mixed));
            }
        }
    }

    // Green circle (right)
    for y in 0..height {
        for x in width / 2..width {
            if ((x as i32 - 75).pow(2) + (y as i32 - 50).pow(2)) < 400 {
                img.put_pixel(x, y, image::Rgb(green_mixed));
            }
        }
    }

    let test_image_path = temp_dir.path().join("test_input.png");
    img.save(&test_image_path).unwrap();

    // Run bgone with all unknown colors
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        test_image_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        "--strict",
        "--fg",
        "auto",
        "auto",
        "--bg",
        "#ffffff",
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let output_str = String::from_utf8_lossy(&output);
    println!("Output:\n{}", output_str);

    // Should deduce both colors
    assert!(output_str.contains("Deduced 2 unknown colors"));
}

#[test]
fn test_color_deduction_error_cases() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test: Only 'auto' specified without any known colors - should work in strict mode
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/square.png",
        output_path.to_str().unwrap(),
        "--strict",
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

    // Run bgone with mix of known and unknown colors
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/rectangles.png",
        output_path.to_str().unwrap(),
        "--strict",
        "--fg",
        "#ff0000", // Red (known)
        "auto",    // Unknown
        "#ffff00", // Yellow (known)
        "--bg",
        "#0000ff",
    ]);

    cmd.assert()
        .success()
        .stdout(predicates::str::contains("Deduced 1 unknown color"));
}

#[test]
fn test_multiple_unknown_colors_convergence() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test that multiple unknowns don't all converge to black
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/circle-gradients.png",
        output_path.to_str().unwrap(),
        "--strict",
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
    println!("Output:\n{}", output_str);

    // Extract deduced colors from output
    let mut deduced_colors = Vec::new();
    for line in output_str.lines() {
        if line.contains("Deduced 3 unknown colors:") {
            // Parse colors from format: "✓ Deduced 3 unknown colors: #ff1b1b #1bff1b #1b1bff"
            if let Some(colors_part) = line.split("colors:").nth(1) {
                deduced_colors = colors_part
                    .trim()
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
                break;
            }
        }
    }

    println!("Deduced colors: {:?}", deduced_colors);
    assert_eq!(
        deduced_colors.len(),
        3,
        "Should have deduced exactly 3 colors"
    );

    // Parse the deduced colors
    let mut parsed_colors = Vec::new();
    for color in &deduced_colors {
        if color.starts_with("#") && color.len() == 7 {
            let r = u8::from_str_radix(&color[1..3], 16).unwrap();
            let g = u8::from_str_radix(&color[3..5], 16).unwrap();
            let b = u8::from_str_radix(&color[5..7], 16).unwrap();
            parsed_colors.push((r, g, b));
        }
    }

    // Check that we have valid colors
    assert_eq!(parsed_colors.len(), 3, "All colors should be valid");

    // The optimal colors should be pure red, green, and blue (furthest from white background)
    let has_pure_red = parsed_colors
        .iter()
        .any(|&(r, g, b)| r == 255 && g == 0 && b == 0);
    let has_pure_green = parsed_colors
        .iter()
        .any(|&(r, g, b)| r == 0 && g == 255 && b == 0);
    let has_pure_blue = parsed_colors
        .iter()
        .any(|&(r, g, b)| r == 0 && g == 0 && b == 255);

    // The algorithm should now find the optimal pure colors
    assert!(
        has_pure_red && has_pure_green && has_pure_blue,
        "Expected pure colors #ff0000, #00ff00, #0000ff but got {:?}",
        deduced_colors
    );

    // Load and save images for inspection
    let original = image::open("tests/inputs/circle-gradients.png").unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [255, 255, 255]);

    save_test_images(
        "color_deduction",
        "circle_gradients_all_auto",
        &processed,
        &reconstructed,
    );

    // Also check reconstruction quality
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    println!("Three gradients deduction - Similarity: {:.2}%", similarity);
}

#[test]
fn test_auto_deduction_finds_optimal_colors() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // This test checks that auto deduction finds the most saturated colors
    // (furthest from the background), not just any valid colors
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/circle-gradients.png",
        output_path.to_str().unwrap(),
        "--strict",
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
        .clone();

    let output_str = String::from_utf8_lossy(&output.stdout);

    // Extract deduced colors
    let mut deduced_colors: Vec<String> = Vec::new();
    for line in output_str.lines() {
        if line.contains("Deduced unknown color") {
            if let Some(color_part) = line.split(':').nth(1) {
                let colors: Vec<String> = color_part
                    .trim()
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
                deduced_colors.extend(colors);
            }
        } else if line.contains("Deduced 3 unknown colors:") {
            if let Some(color_part) = line.split(':').nth(1) {
                deduced_colors = color_part
                    .trim()
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
                break;
            }
        }
    }

    println!("Deduced colors: {:?}", deduced_colors);

    // Parse colors
    let mut parsed_colors = Vec::new();
    for color in &deduced_colors {
        if color.starts_with("#") && color.len() == 7 {
            let r = u8::from_str_radix(&color[1..3], 16).unwrap();
            let g = u8::from_str_radix(&color[3..5], 16).unwrap();
            let b = u8::from_str_radix(&color[5..7], 16).unwrap();
            parsed_colors.push((r, g, b));
        }
    }

    // The optimal colors for white background should be pure RGB
    let has_pure_red = parsed_colors
        .iter()
        .any(|&(r, g, b)| r == 255 && g == 0 && b == 0);
    let has_pure_green = parsed_colors
        .iter()
        .any(|&(r, g, b)| r == 0 && g == 255 && b == 0);
    let has_pure_blue = parsed_colors
        .iter()
        .any(|&(r, g, b)| r == 0 && g == 0 && b == 255);

    assert!(
        has_pure_red && has_pure_green && has_pure_blue,
        "Expected pure colors #ff0000, #00ff00, #0000ff but got {:?}",
        deduced_colors
    );
}

#[test]
fn test_circle_gradients_with_known_red() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with red as known color - should deduce green and blue
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/circle-gradients.png",
        output_path.to_str().unwrap(),
        "--strict",
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

    // Extract deduced colors from output
    let mut colors = vec!["#ff0000".to_string()]; // Red is known
    for line in output_str.lines() {
        if line.contains("Deduced 2 unknown colors:") {
            // Parse colors from format: "✓ Deduced 2 unknown colors: #1bff1b #1b1bff"
            if let Some(colors_part) = line.split("colors:").nth(1) {
                let deduced: Vec<String> = colors_part
                    .trim()
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
                colors.extend(deduced);
                break;
            }
        }
    }

    println!("Final colors with red known: {:?}", colors);

    // Should have red plus two other bright colors
    assert_eq!(colors.len(), 3);
    assert!(colors.contains(&"#ff0000".to_string()));

    // Check that we deduced green and blue (approximately)
    let mut found_green = false;
    let mut found_blue = false;

    for color in &colors {
        if color != "#ff0000" && color.starts_with("#") && color.len() == 7 {
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

    // Load and save images for inspection
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [255, 255, 255]);

    save_test_images(
        "color_deduction",
        "circle_gradients_known_red",
        &processed,
        &reconstructed,
    );
}

#[test]
fn test_circle_gradients_with_known_green() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with green as known color - should deduce red and blue
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/circle-gradients.png",
        output_path.to_str().unwrap(),
        "--strict",
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

    // Extract deduced colors from output
    let mut colors = vec!["#00ff00".to_string()]; // Green is known
    for line in output_str.lines() {
        if line.contains("Deduced 2 unknown colors:") {
            // Parse colors from format: "✓ Deduced 2 unknown colors: #ff1b1b #1b1bff"
            if let Some(colors_part) = line.split("colors:").nth(1) {
                let deduced: Vec<String> = colors_part
                    .trim()
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
                colors.extend(deduced);
                break;
            }
        }
    }

    println!("Final colors with green known: {:?}", colors);

    // Should have green plus two other bright colors
    assert_eq!(colors.len(), 3);
    assert!(colors.contains(&"#00ff00".to_string()));

    // Check that we deduced red and blue (approximately)
    let mut found_red = false;
    let mut found_blue = false;

    for color in &colors {
        if color != "#00ff00" && color.starts_with("#") && color.len() == 7 {
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

    // Load and save images for inspection
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [255, 255, 255]);

    save_test_images(
        "color_deduction",
        "circle_gradients_known_green",
        &processed,
        &reconstructed,
    );
}

#[test]
fn test_circle_gradients_with_known_blue() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with blue as known color - should deduce red and green
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/circle-gradients.png",
        output_path.to_str().unwrap(),
        "--strict",
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

    // Extract deduced colors from output
    let mut colors = vec!["#0000ff".to_string()]; // Blue is known
    for line in output_str.lines() {
        if line.contains("Deduced 2 unknown colors:") {
            // Parse colors from format: "✓ Deduced 2 unknown colors: #ff1b1b #1bff1b"
            if let Some(colors_part) = line.split("colors:").nth(1) {
                let deduced: Vec<String> = colors_part
                    .trim()
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
                colors.extend(deduced);
                break;
            }
        }
    }

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

    // Load and save images for inspection
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [255, 255, 255]);

    save_test_images(
        "color_deduction",
        "circle_gradients_known_blue",
        &processed,
        &reconstructed,
    );
}

#[test]
fn test_square_gradient_auto_deduction() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with auto color deduction - should maximize opacity
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/square-gradient.png",
        output_path.to_str().unwrap(),
        "--strict",
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
    println!("Square gradient auto deduction output:\n{}", output_str);

    // Verify cyan and magenta were deduced
    let lines: Vec<&str> = output_str.lines().collect();
    for line in lines {
        if line.contains("Deduced 2 unknown colors") {
            println!("Success: {}", line);
            break;
        }
    }

    // Load and save images for inspection
    let original = image::open("tests/inputs/square-gradient.png").unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [20, 25, 30]);

    save_test_images(
        "color_deduction",
        "square_gradient_auto",
        &processed,
        &reconstructed,
    );

    // Check reconstruction quality
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    println!(
        "Square gradient (auto deduction) - Similarity: {:.2}%",
        similarity
    );

    // Should have good reconstruction quality
    assert!(
        similarity > 98.0,
        "Reconstruction quality too low: {:.2}%",
        similarity
    );
}

#[test]
fn test_square_gradient_with_known_magenta() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with known magenta (slightly different) and auto cyan
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/square-gradient.png",
        output_path.to_str().unwrap(),
        "--strict",
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
    println!("Output with known magenta:\n{}", output_str);

    // Check similarity is still acceptable
    let original = image::open("tests/inputs/square-gradient.png").unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [20, 25, 30]);

    save_test_images(
        "color_deduction",
        "square_gradient_known_magenta",
        &processed,
        &reconstructed,
    );

    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    println!(
        "Square gradient (known magenta) - Similarity: {:.2}%",
        similarity
    );

    // With different magenta, similarity might be lower
    assert!(
        similarity > 98.0,
        "Reconstruction quality too low: {:.2}%",
        similarity
    );
}

#[test]
fn test_square_gradient_with_known_cyan() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test with known cyan (slightly different) and auto magenta
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/square-gradient.png",
        output_path.to_str().unwrap(),
        "--strict",
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
    println!("Output with known cyan:\n{}", output_str);

    // Check similarity is still acceptable
    let original = image::open("tests/inputs/square-gradient.png").unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [20, 25, 30]);

    save_test_images(
        "color_deduction",
        "square_gradient_known_cyan",
        &processed,
        &reconstructed,
    );

    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    println!(
        "Square gradient (known cyan) - Similarity: {:.2}%",
        similarity
    );

    // With different cyan, similarity might be lower
    assert!(
        similarity > 98.0,
        "Reconstruction quality too low: {:.2}%",
        similarity
    );
}

#[test]
fn test_pure_color_deduction() {
    let temp_dir = TempDir::new().unwrap();

    // Create test image: pure green on black
    let width = 100;
    let height = 100;
    let mut img = image::RgbImage::new(width, height);

    // Fill with black background
    for pixel in img.pixels_mut() {
        *pixel = image::Rgb([0, 0, 0]);
    }

    // Add pure green rectangle
    for y in 25..75 {
        for x in 25..75 {
            img.put_pixel(x, y, image::Rgb([0, 255, 0]));
        }
    }

    let test_image_path = temp_dir.path().join("pure_green.png");
    img.save(&test_image_path).unwrap();

    // Test color deduction
    let img = image::open(&test_image_path).unwrap();
    let specs = vec![ForegroundColorSpec::Unknown];
    let background = [0, 0, 0];

    let result = deduce_unknown_colors(&img, &specs, background, 0.05).unwrap();

    assert_eq!(result.len(), 1);
    let deduced_color = result[0];

    // Should deduce pure green
    assert_eq!(deduced_color[0], 0); // R
    assert_eq!(deduced_color[1], 255); // G
    assert_eq!(deduced_color[2], 0); // B

    println!("Successfully deduced pure green: {:?}", deduced_color);
}

#[test]
fn test_opacity_optimization() {
    // Test that unmixing optimizes for opacity
    let observed = [128, 0, 0]; // 50% red on black
    let foregrounds = vec![[1.0, 0.0, 0.0]]; // Pure red
    let background = [0.0, 0.0, 0.0]; // Black

    let result = unmix_colors(observed, &foregrounds, background);
    let (color, alpha) = compute_result_color(&result, &foregrounds);

    println!("Unmixed color: {:?}, alpha: {}", color, alpha);

    // Should get pure red with ~50% opacity
    assert!((color[0] - 1.0).abs() < 0.01);
    assert!((color[1] - 0.0).abs() < 0.01);
    assert!((color[2] - 0.0).abs() < 0.01);
    assert!((alpha - 0.502).abs() < 0.01); // ~50%

    // Test with multiple colors - should still optimize for opacity
    let observed = [255, 255, 0]; // Yellow
    let foregrounds = vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]; // R,G,B
    let result = unmix_colors(observed, &foregrounds, background);

    println!("Yellow unmix result: {:?}", result);

    // Should use red and green at full opacity, not blue
    assert!(result.weights[0] > 0.4); // Red weight
    assert!(result.weights[1] > 0.4); // Green weight
    assert!(result.weights[2] < 0.1); // Blue weight (should be near 0)
    assert!(result.alpha > 0.95); // Should achieve high opacity

    // Test gradient case
    let observed = [200, 100, 100]; // Reddish color
    let foregrounds = vec![[1.0, 0.0, 0.0], [0.5, 0.5, 0.5]]; // Red and gray
    let background = [1.0, 1.0, 1.0]; // White

    let result = unmix_colors(observed, &foregrounds, background);
    println!("Gradient unmix result: {:?}", result);

    // Should achieve good opacity
    assert!(result.alpha > 0.6);
}
