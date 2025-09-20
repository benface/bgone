//! Generate test fixtures for integration tests
//! Run with: cargo test --test generate_fixtures -- --ignored

use image::{Rgba, RgbaImage};
use std::path::Path;

#[test]
#[ignore]
fn generate_test_images() {
    let fixtures_dir = Path::new("tests/fixtures");
    std::fs::create_dir_all(fixtures_dir).unwrap();

    // Test case 1: Red square on black background
    generate_red_on_black();

    // Test case 2: Red gradient (with alpha blend) on white background
    generate_red_gradient_on_white();

    // Test case 3: Multiple colors on blue background
    generate_multicolor_on_blue();

    println!("Test fixtures generated in tests/fixtures/");
}

fn generate_red_on_black() {
    let mut img = RgbaImage::new(100, 100);

    // Fill with black background
    for pixel in img.pixels_mut() {
        *pixel = Rgba([0, 0, 0, 255]);
    }

    // Add red square in center
    for y in 25..75 {
        for x in 25..75 {
            img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
        }
    }

    img.save("tests/fixtures/red_on_black.png").unwrap();
}

fn generate_red_gradient_on_white() {
    let mut img = RgbaImage::new(100, 100);

    // Create a red gradient that fades to white (simulating alpha blend)
    for y in 0..100 {
        for x in 0..100 {
            let distance = ((x as f32 - 50.0).powi(2) + (y as f32 - 50.0).powi(2)).sqrt();
            let alpha = (1.0 - (distance / 50.0)).max(0.0);

            // Blend red with white background
            let r = 255;
            let g = ((1.0 - alpha) * 255.0) as u8;
            let b = ((1.0 - alpha) * 255.0) as u8;

            img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }

    img.save("tests/fixtures/red_gradient_on_white.png")
        .unwrap();
}

fn generate_multicolor_on_blue() {
    let mut img = RgbaImage::new(150, 100);

    // Fill with blue background
    for pixel in img.pixels_mut() {
        *pixel = Rgba([0, 0, 255, 255]);
    }

    // Add pure red rectangle
    for y in 20..40 {
        for x in 20..50 {
            img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
        }
    }

    // Add pure green rectangle
    for y in 20..40 {
        for x in 60..90 {
            img.put_pixel(x, y, Rgba([0, 255, 0, 255]));
        }
    }

    // Add yellow rectangle (mix of red and green)
    for y in 20..40 {
        for x in 100..130 {
            img.put_pixel(x, y, Rgba([255, 255, 0, 255]));
        }
    }

    // Add semi-transparent overlays to test blending
    for y in 50..70 {
        for x in 20..50 {
            // 50% red on blue = purple
            img.put_pixel(x, y, Rgba([128, 0, 128, 255]));
        }
        for x in 60..90 {
            // 50% green on blue = teal
            img.put_pixel(x, y, Rgba([0, 128, 128, 255]));
        }
        for x in 100..130 {
            // 50% yellow on blue
            img.put_pixel(x, y, Rgba([128, 128, 128, 255]));
        }
    }

    img.save("tests/fixtures/multicolor_on_blue.png").unwrap();
}
