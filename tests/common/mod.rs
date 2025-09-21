use std::fs;

// Set to true to save test outputs for inspection
pub const SAVE_TEST_OUTPUTS: bool = true;

pub fn ensure_output_dir() {
    if SAVE_TEST_OUTPUTS {
        fs::create_dir_all("tests/outputs").unwrap();
    }
}

pub fn save_test_images(
    file_prefix: &str,
    test_name: &str,
    processed: &image::DynamicImage,
    reconstructed: &image::DynamicImage,
) {
    if SAVE_TEST_OUTPUTS {
        let processed_path = format!("tests/outputs/{}_{}_processed.png", file_prefix, test_name);
        let reconstructed_path = format!(
            "tests/outputs/{}_{}_reconstructed.png",
            file_prefix, test_name
        );

        processed.save(&processed_path).unwrap();
        reconstructed.save(&reconstructed_path).unwrap();

        println!("  Saved: {}", processed_path);
        println!("  Saved: {}", reconstructed_path);
    }
}
