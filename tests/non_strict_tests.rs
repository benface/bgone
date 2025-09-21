mod common;

use assert_cmd::Command;
use bgone::process_image;
use common::{
    calculate_psnr, calculate_similarity_percentage, ensure_output_dir, overlay_on_background,
    save_test_images,
};
use image::{DynamicImage, Rgba, RgbaImage};
use tempfile::TempDir;

#[test]
fn test_non_strict_mode_no_fg() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test non-strict mode without any foreground colors
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/circle-gradients.png",
        output_path.to_str().unwrap(),
        "--bg",
        "#ffffff",
    ]);

    cmd.assert().success();

    // Load and save images for inspection
    let original = image::open("tests/inputs/circle-gradients.png").unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [255, 255, 255]);

    save_test_images("non_strict", "no_fg", &processed, &reconstructed);

    // Check reconstruction quality
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);
    println!(
        "Non-strict mode (no fg) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );

    // Should have near-perfect reconstruction
    assert!(
        similarity > 99.9,
        "Reconstruction quality too low: {:.4}%",
        similarity
    );
}

#[test]
fn test_non_strict_mode_with_fg() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Test non-strict mode with red foreground color
    let mut cmd = Command::cargo_bin("bgone").unwrap();
    cmd.args(&[
        "tests/inputs/square-glow.png",
        output_path.to_str().unwrap(),
        "--fg",
        "#ff0000",
        "--bg",
        "#000000",
    ]);

    cmd.assert().success();

    // Load and save images for inspection
    let original = image::open("tests/inputs/square-glow.png").unwrap();
    let processed = image::open(&output_path).unwrap();
    let reconstructed = overlay_on_background(&processed, [0, 0, 0]);

    save_test_images("non_strict", "with_fg", &processed, &reconstructed);

    // Check reconstruction quality
    let similarity = calculate_similarity_percentage(&original, &reconstructed);
    let psnr = calculate_psnr(&original, &reconstructed);
    println!(
        "Non-strict mode (with fg) - Similarity: {:.2}%, PSNR: {:.2} dB",
        similarity, psnr
    );

    // Should have very good reconstruction
    assert!(
        similarity > 98.5,
        "Reconstruction quality too low: {:.4}%",
        similarity
    );
}

// Helper function to overlay a single pixel
fn overlay_single_pixel(pixel: Rgba<u8>, background: [u8; 3]) -> (u8, u8, u8) {
    let alpha = pixel[3] as f32 / 255.0;
    let inv_alpha = 1.0 - alpha;

    let r = (pixel[0] as f32 * alpha + background[0] as f32 * inv_alpha) as u8;
    let g = (pixel[1] as f32 * alpha + background[1] as f32 * inv_alpha) as u8;
    let b = (pixel[2] as f32 * alpha + background[2] as f32 * inv_alpha) as u8;

    (r, g, b)
}

#[test]
fn test_non_strict_optimal_alpha() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();

    // Test case 1: Light pink on white background
    // Observed: (255, 242, 242), Background: (255, 255, 255)
    // Current algorithm might use (255, 35, 35) with alpha ~0.058
    // But optimal would be (255, 0, 0) with alpha ~0.051
    let mut img = RgbaImage::new(1, 1);
    img.put_pixel(0, 0, Rgba([255, 242, 242, 255]));
    let input = DynamicImage::ImageRgba8(img);

    let input_path = temp_dir.path().join("input.png");
    let output_path = temp_dir.path().join("output.png");
    input.save(&input_path).unwrap();

    let background = [255u8, 255, 255];
    process_image(&input_path, &output_path, vec![], background, false, None).unwrap();

    let result = image::open(&output_path).unwrap();
    if let DynamicImage::ImageRgba8(result_img) = &result {
        let pixel = result_img.get_pixel(0, 0);
        let alpha = pixel[3] as f64 / 255.0;

        println!("Test case 1 - Light pink on white:");
        println!(
            "  Result: RGBA({}, {}, {}, {})",
            pixel[0], pixel[1], pixel[2], pixel[3]
        );
        println!("  Alpha: {:.4}", alpha);

        // Verify perfect reconstruction
        let reconstructed = overlay_single_pixel(*pixel, background);
        assert_eq!(reconstructed.0, 255);
        assert!(
            (reconstructed.1 as i32 - 242).abs() <= 1,
            "G component mismatch: {} vs 242",
            reconstructed.1
        );
        assert!(
            (reconstructed.2 as i32 - 242).abs() <= 1,
            "B component mismatch: {} vs 242",
            reconstructed.2
        );

        // The optimal alpha for (255, 0, 0) would be ~0.051
        // Current algorithm gives ~0.058, so this will fail initially
        assert!(
            alpha <= 0.052,
            "Alpha not optimal: {:.4} (expected ~0.051)",
            alpha
        );
    } else {
        panic!("Expected RGBA8 image");
    }
}

#[test]
fn test_non_strict_edge_cases() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();

    // Test case 2: Color very close to background
    let mut img = RgbaImage::new(1, 1);
    img.put_pixel(0, 0, Rgba([254, 255, 255, 255]));
    let input = DynamicImage::ImageRgba8(img);

    let input_path = temp_dir.path().join("input2.png");
    let output_path = temp_dir.path().join("output2.png");
    input.save(&input_path).unwrap();

    let background = [255u8, 255, 255];
    process_image(&input_path, &output_path, vec![], background, false, None).unwrap();

    let result = image::open(&output_path).unwrap();
    if let DynamicImage::ImageRgba8(result_img) = &result {
        let pixel = result_img.get_pixel(0, 0);
        let alpha = pixel[3] as f64 / 255.0;

        println!("Test case 2 - Near-white on white:");
        println!(
            "  Result: RGBA({}, {}, {}, {})",
            pixel[0], pixel[1], pixel[2], pixel[3]
        );
        println!("  Alpha: {:.4}", alpha);

        // The optimal solution would use a foreground color like (0, 255, 255) with very low alpha
        assert!(
            alpha <= 0.005,
            "Alpha not minimal for near-background color: {:.4}",
            alpha
        );
    }

    // Test case 3: Pure color channel
    let mut img = RgbaImage::new(1, 1);
    img.put_pixel(0, 0, Rgba([128, 255, 255, 255]));
    let input = DynamicImage::ImageRgba8(img);

    let input_path = temp_dir.path().join("input3.png");
    let output_path = temp_dir.path().join("output3.png");
    input.save(&input_path).unwrap();

    process_image(&input_path, &output_path, vec![], background, false, None).unwrap();

    let result = image::open(&output_path).unwrap();
    if let DynamicImage::ImageRgba8(result_img) = &result {
        let pixel = result_img.get_pixel(0, 0);
        let alpha = pixel[3] as f64 / 255.0;

        println!("Test case 3 - Cyan-tinted on white:");
        println!(
            "  Result: RGBA({}, {}, {}, {})",
            pixel[0], pixel[1], pixel[2], pixel[3]
        );
        println!("  Alpha: {:.4}", alpha);

        // The optimal solution would use (0, 255, 255) cyan with alpha 0.5
        assert!(
            alpha <= 0.502,
            "Alpha not optimal for cyan tint: {:.4} (expected ~0.5)",
            alpha
        );
    }
}

#[test]
fn test_non_strict_alpha_minimization() {
    ensure_output_dir();

    // Test systematic alpha minimization across different scenarios
    let test_cases = vec![
        // (observed_color, background, description)
        ([200, 200, 255, 255], [255, 255, 255], "Blue tint on white"),
        ([255, 200, 200, 255], [200, 200, 200], "Red tint on gray"),
        ([100, 150, 200, 255], [255, 255, 255], "Dark blue on white"),
        ([255, 128, 0, 255], [0, 0, 0], "Orange on black"),
    ];

    for (i, (observed, background, desc)) in test_cases.iter().enumerate() {
        let temp_dir = TempDir::new().unwrap();
        println!("\nTesting: {}", desc);

        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba(*observed));
        let input = DynamicImage::ImageRgba8(img);

        let input_path = temp_dir.path().join(format!("input_{}.png", i));
        let output_path = temp_dir.path().join(format!("output_{}.png", i));
        input.save(&input_path).unwrap();

        process_image(&input_path, &output_path, vec![], *background, false, None).unwrap();

        let result = image::open(&output_path).unwrap();
        if let DynamicImage::ImageRgba8(result_img) = &result {
            let pixel = result_img.get_pixel(0, 0);
            let alpha = pixel[3] as f64 / 255.0;

            println!(
                "  Observed: {:?}, Background: {:?}",
                &observed[0..3],
                background
            );
            println!(
                "  Result: RGBA({}, {}, {}, {})",
                pixel[0], pixel[1], pixel[2], pixel[3]
            );
            println!("  Alpha: {:.4}", alpha);

            // Verify reconstruction
            let reconstructed = overlay_single_pixel(*pixel, *background);
            assert!((reconstructed.0 as i32 - observed[0] as i32).abs() <= 1);
            assert!((reconstructed.1 as i32 - observed[1] as i32).abs() <= 1);
            assert!((reconstructed.2 as i32 - observed[2] as i32).abs() <= 1);

            // For each case, we should find a truly minimal alpha
            // This is hard to verify without knowing the optimal solution,
            // but we can at least ensure the alpha is reasonable
            assert!(alpha > 0.0 && alpha <= 1.0, "Alpha out of range: {}", alpha);
        }
    }
}
