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

    // Test case 4: Red with purple glow
    generate_red_with_purple_glow();

    // Test case 5: Three color gradients on white
    generate_three_gradients_on_white();

    // Test case 6: Gradient rectangle on dark background
    generate_gradient_rect_on_dark();

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
    // Using 75% opacity to avoid confusion with solid colors
    for y in 50..70 {
        for x in 20..50 {
            // 75% red + 25% blue = (191, 0, 64)
            img.put_pixel(x, y, Rgba([191, 0, 64, 255]));
        }
        for x in 60..90 {
            // 75% green + 25% blue = (0, 191, 64)
            img.put_pixel(x, y, Rgba([0, 191, 64, 255]));
        }
        for x in 100..130 {
            // 75% yellow + 25% blue = (191, 191, 64)
            img.put_pixel(x, y, Rgba([191, 191, 64, 255]));
        }
    }

    img.save("tests/fixtures/multicolor_on_blue.png").unwrap();
}

fn generate_red_with_purple_glow() {
    let width = 200;
    let height = 200;
    let mut img = RgbaImage::new(width, height);

    // Fill with black background
    for pixel in img.pixels_mut() {
        *pixel = Rgba([0, 0, 0, 255]);
    }

    // First pass: Add purple glow
    let glow_center_x = 100.0;
    let glow_center_y = 100.0;
    let glow_radius = 50.0;

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - glow_center_x;
            let dy = y as f32 - glow_center_y;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist < glow_radius {
                // Calculate glow intensity based on distance
                let intensity = 1.0 - (dist / glow_radius);
                let alpha = (intensity * 0.5 * 255.0) as u8; // Max 50% alpha

                // Blend purple glow with existing pixel
                let existing = img.get_pixel(x, y);
                let purple_r = 128;
                let purple_g = 0;
                let purple_b = 128;

                // Alpha blending
                let alpha_f = alpha as f32 / 255.0;
                let inv_alpha = 1.0 - alpha_f;

                let new_r = (purple_r as f32 * alpha_f + existing[0] as f32 * inv_alpha) as u8;
                let new_g = (purple_g as f32 * alpha_f + existing[1] as f32 * inv_alpha) as u8;
                let new_b = (purple_b as f32 * alpha_f + existing[2] as f32 * inv_alpha) as u8;

                img.put_pixel(x, y, Rgba([new_r, new_g, new_b, 255]));
            }
        }
    }

    // Second pass: Add red logo (opaque)
    for y in 75..125 {
        for x in 75..125 {
            img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
        }
    }

    img.save("tests/fixtures/red_with_purple_glow.png").unwrap();
}

fn generate_three_gradients_on_white() {
    let width = 300;
    let height = 100;
    let mut img = RgbaImage::new(width, height);

    // Fill with white background
    for pixel in img.pixels_mut() {
        *pixel = Rgba([255, 255, 255, 255]);
    }

    // Create three circular gradients side by side
    // Red gradient on the left
    let red_center_x = 50.0;
    let center_y = 50.0;
    let radius = 40.0;

    // Green gradient in the middle
    let green_center_x = 150.0;

    // Blue gradient on the right
    let blue_center_x = 250.0;

    for y in 0..height {
        for x in 0..width {
            let y_f = y as f32;
            let x_f = x as f32;

            // Calculate distances to each center
            let dist_red = ((x_f - red_center_x).powi(2) + (y_f - center_y).powi(2)).sqrt();
            let dist_green = ((x_f - green_center_x).powi(2) + (y_f - center_y).powi(2)).sqrt();
            let dist_blue = ((x_f - blue_center_x).powi(2) + (y_f - center_y).powi(2)).sqrt();

            // Start with white background
            let mut r = 255u8;
            let mut g = 255u8;
            let mut b = 255u8;

            // Apply red gradient
            if dist_red < radius {
                let intensity = 1.0 - (dist_red / radius);
                let alpha = intensity;
                // Blend red with white
                r = (255.0 * alpha + 255.0 * (1.0 - alpha)) as u8;
                g = (0.0 * alpha + 255.0 * (1.0 - alpha)) as u8;
                b = (0.0 * alpha + 255.0 * (1.0 - alpha)) as u8;
            }

            // Apply green gradient
            if dist_green < radius {
                let intensity = 1.0 - (dist_green / radius);
                let alpha = intensity;
                // Blend green with white
                let new_r = (0.0 * alpha + 255.0 * (1.0 - alpha)) as u8;
                let new_g = (255.0 * alpha + 255.0 * (1.0 - alpha)) as u8;
                let new_b = (0.0 * alpha + 255.0 * (1.0 - alpha)) as u8;
                if new_r < r || new_b < b {
                    r = new_r;
                    g = new_g;
                    b = new_b;
                }
            }

            // Apply blue gradient
            if dist_blue < radius {
                let intensity = 1.0 - (dist_blue / radius);
                let alpha = intensity;
                // Blend blue with white
                let new_r = (0.0 * alpha + 255.0 * (1.0 - alpha)) as u8;
                let new_g = (0.0 * alpha + 255.0 * (1.0 - alpha)) as u8;
                let new_b = (255.0 * alpha + 255.0 * (1.0 - alpha)) as u8;
                if new_r < r || new_g < g {
                    r = new_r;
                    g = new_g;
                    b = new_b;
                }
            }

            img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }

    img.save("tests/fixtures/three_gradients_on_white.png")
        .unwrap();
}

fn generate_gradient_rect_on_dark() {
    let mut img = RgbaImage::new(200, 200);

    // Fill with dark (but not black) background - dark gray/blue
    let bg_color = Rgba([20, 25, 30, 255]);
    for pixel in img.pixels_mut() {
        *pixel = bg_color;
    }

    // Create a gradient rectangle from cyan to magenta with 50% opacity
    let rect_left = 50;
    let rect_right = 150;
    let rect_top = 50;
    let rect_bottom = 150;
    let rect_width = rect_right - rect_left;

    for y in rect_top..rect_bottom {
        for x in rect_left..rect_right {
            // Calculate gradient position (0.0 to 1.0)
            let gradient_pos = (x - rect_left) as f32 / rect_width as f32;

            // Interpolate between cyan and magenta
            let cyan = [0.0, 255.0, 255.0];
            let magenta = [255.0, 0.0, 255.0];

            let fg_r = cyan[0] * (1.0 - gradient_pos) + magenta[0] * gradient_pos;
            let fg_g = cyan[1] * (1.0 - gradient_pos) + magenta[1] * gradient_pos;
            let fg_b = cyan[2] * (1.0 - gradient_pos) + magenta[2] * gradient_pos;

            // Apply 50% opacity blend with background
            let opacity = 0.5;
            let final_r = (fg_r * opacity + bg_color[0] as f32 * (1.0 - opacity)) as u8;
            let final_g = (fg_g * opacity + bg_color[1] as f32 * (1.0 - opacity)) as u8;
            let final_b = (fg_b * opacity + bg_color[2] as f32 * (1.0 - opacity)) as u8;

            img.put_pixel(x, y, Rgba([final_r, final_g, final_b, 255]));
        }
    }

    img.save("tests/fixtures/gradient_rect_on_dark.png")
        .unwrap();
}
