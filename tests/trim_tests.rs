mod common;

use assert_cmd::cargo;
use assert_cmd::prelude::*;
use bgone::trim_to_content;
use common::{ensure_output_dir, save_test_images};
use image::{GenericImageView, ImageBuffer, Rgba, RgbaImage};
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_trim_rectangles() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Process rectangles.png with --trim
    let mut cmd = Command::new(cargo::cargo_bin!("bgone"));
    cmd.args([
        "tests/inputs/rectangles.png",
        output_path.to_str().unwrap(),
        "--bg",
        "#0000ff",
        "--trim",
    ]);

    cmd.assert().success();

    // Load the processed image and verify it was trimmed
    let processed = image::open(&output_path).unwrap();
    let (width, height) = processed.to_rgba8().dimensions();

    // The rectangles image is 150x100 with a blue background
    // After background removal and trimming, it should be smaller
    // The rectangles occupy roughly: x=10-140, y=10-90 (based on the image)
    println!("Trimmed rectangles dimensions: {}x{}", width, height);

    // Original is 150x100, trimmed should be smaller
    assert!(
        width < 150 || height < 100,
        "Image should have been trimmed, but dimensions are {}x{}",
        width,
        height
    );

    // Save for visual inspection
    let dummy_reconstructed = processed.clone();
    save_test_images("trim", "rectangles", &processed, &dummy_reconstructed);
}

#[test]
fn test_trim_square_glow() {
    ensure_output_dir();
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.png");

    // Process square-glow.png with --trim
    // This image has a red square with a glow effect on black background
    let mut cmd = Command::new(cargo::cargo_bin!("bgone"));
    cmd.args([
        "tests/inputs/square-glow.png",
        output_path.to_str().unwrap(),
        "--bg",
        "#000000",
        "--fg",
        "#ff0000",
        "--trim",
    ]);

    cmd.assert().success();

    // Load the processed image
    let processed = image::open(&output_path).unwrap();
    let (width, height) = processed.to_rgba8().dimensions();

    // The square-glow image is 200x200
    // After background removal and trimming, the glow should extend beyond the square
    // but we should have trimmed some edges
    println!("Trimmed square-glow dimensions: {}x{}", width, height);

    // Save for visual inspection
    let dummy_reconstructed = processed.clone();
    save_test_images("trim", "square_glow", &processed, &dummy_reconstructed);
}

#[test]
fn test_trim_no_change_when_fully_opaque() {
    ensure_output_dir();

    // Create an image where content fills the entire image
    let mut img: RgbaImage = ImageBuffer::new(10, 10);
    for y in 0..10 {
        for x in 0..10 {
            img.put_pixel(x, y, Rgba([255, 0, 0, 255])); // Fully opaque red
        }
    }

    let trimmed = trim_to_content(&img);
    let (width, height) = trimmed.dimensions();

    assert_eq!(width, 10, "Width should remain 10");
    assert_eq!(height, 10, "Height should remain 10");
}

#[test]
fn test_trim_removes_transparent_padding() {
    ensure_output_dir();

    // Create an image with transparent padding around the content
    let mut img: RgbaImage = ImageBuffer::new(20, 20);
    // Fill with transparent pixels
    for y in 0..20 {
        for x in 0..20 {
            img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
        }
    }
    // Put a 6x6 red square in the center (at positions 7-12 in both dimensions)
    for y in 7..13 {
        for x in 7..13 {
            img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
        }
    }

    let trimmed = trim_to_content(&img);
    let (width, height) = trimmed.dimensions();

    assert_eq!(width, 6, "Trimmed width should be 6, got {}", width);
    assert_eq!(height, 6, "Trimmed height should be 6, got {}", height);

    // Verify the content is preserved
    for y in 0..6 {
        for x in 0..6 {
            let pixel = trimmed.get_pixel(x, y);
            assert_eq!(
                pixel,
                &Rgba([255, 0, 0, 255]),
                "Pixel at ({}, {}) should be red",
                x,
                y
            );
        }
    }
}

#[test]
fn test_trim_asymmetric_content() {
    ensure_output_dir();

    // Create an image with content only in one corner
    let mut img: RgbaImage = ImageBuffer::new(100, 100);
    // Fill with transparent pixels
    for y in 0..100 {
        for x in 0..100 {
            img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
        }
    }
    // Put a 10x5 rectangle in the top-left corner (starting at 2,3)
    for y in 3..8 {
        for x in 2..12 {
            img.put_pixel(x, y, Rgba([0, 255, 0, 255]));
        }
    }

    let trimmed = trim_to_content(&img);
    let (width, height) = trimmed.dimensions();

    assert_eq!(width, 10, "Trimmed width should be 10, got {}", width);
    assert_eq!(height, 5, "Trimmed height should be 5, got {}", height);
}

#[test]
fn test_trim_fully_transparent_image() {
    ensure_output_dir();

    // Create a fully transparent image
    let img: RgbaImage = ImageBuffer::from_pixel(50, 50, Rgba([0, 0, 0, 0]));

    let trimmed = trim_to_content(&img);
    let (width, height) = trimmed.dimensions();

    // Should return a 1x1 transparent pixel
    assert_eq!(width, 1, "Width should be 1 for fully transparent image");
    assert_eq!(height, 1, "Height should be 1 for fully transparent image");
    assert_eq!(
        trimmed.get_pixel(0, 0),
        &Rgba([0, 0, 0, 0]),
        "Single pixel should be transparent"
    );
}

#[test]
fn test_trim_single_pixel_content() {
    ensure_output_dir();

    // Create an image with a single non-transparent pixel
    let mut img: RgbaImage = ImageBuffer::from_pixel(100, 100, Rgba([0, 0, 0, 0]));
    img.put_pixel(50, 50, Rgba([255, 255, 255, 255]));

    let trimmed = trim_to_content(&img);
    let (width, height) = trimmed.dimensions();

    assert_eq!(width, 1, "Width should be 1 for single pixel content");
    assert_eq!(height, 1, "Height should be 1 for single pixel content");
    assert_eq!(
        trimmed.get_pixel(0, 0),
        &Rgba([255, 255, 255, 255]),
        "Single pixel should be white"
    );
}

#[test]
fn test_trim_preserves_partial_transparency() {
    ensure_output_dir();

    // Create an image with partially transparent pixels
    let mut img: RgbaImage = ImageBuffer::from_pixel(20, 20, Rgba([0, 0, 0, 0]));
    // Add some semi-transparent pixels
    for y in 5..15 {
        for x in 5..15 {
            // Gradient of alpha from 1 to 255
            let alpha = ((x - 5 + y - 5) * 12).min(255) as u8;
            if alpha > 0 {
                img.put_pixel(x, y, Rgba([128, 128, 128, alpha]));
            }
        }
    }

    let trimmed = trim_to_content(&img);
    let (width, height) = trimmed.dimensions();

    // Should trim to the bounding box that includes any non-zero alpha
    assert!(width <= 10, "Width should be at most 10, got {}", width);
    assert!(height <= 10, "Height should be at most 10, got {}", height);
}

#[test]
fn test_trim_cli_flag_works() {
    let temp_dir = TempDir::new().unwrap();
    let output_without_trim = temp_dir.path().join("no_trim.png");
    let output_with_trim = temp_dir.path().join("with_trim.png");

    // Process without trim
    let mut cmd = Command::new(cargo::cargo_bin!("bgone"));
    cmd.args([
        "tests/inputs/rectangles.png",
        output_without_trim.to_str().unwrap(),
        "--bg",
        "#0000ff",
    ]);
    cmd.assert().success();

    // Process with trim
    let mut cmd = Command::new(cargo::cargo_bin!("bgone"));
    cmd.args([
        "tests/inputs/rectangles.png",
        output_with_trim.to_str().unwrap(),
        "--bg",
        "#0000ff",
        "--trim",
    ]);
    cmd.assert().success();

    // Load both images
    let img_no_trim = image::open(&output_without_trim).unwrap();
    let img_with_trim = image::open(&output_with_trim).unwrap();

    let (w1, h1) = img_no_trim.dimensions();
    let (w2, h2) = img_with_trim.dimensions();

    println!("Without trim: {}x{}", w1, h1);
    println!("With trim: {}x{}", w2, h2);

    // The original image is 150x100, without trim should preserve that
    assert_eq!(w1, 150, "Without trim should preserve width");
    assert_eq!(h1, 100, "Without trim should preserve height");

    // With trim should be smaller (rectangles don't touch the edges)
    assert!(w2 < w1 || h2 < h1, "With trim should produce smaller image");
}
