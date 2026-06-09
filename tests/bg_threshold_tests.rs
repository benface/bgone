use bgone::{ProcessOptions, process_image};
use image::{DynamicImage, Rgba, RgbaImage};
use tempfile::TempDir;

/// Build a 4x1 image with a controlled palette and process it. Returns the four
/// output pixels in input order.
fn process_palette(
    palette: [[u8; 4]; 4],
    background: [u8; 3],
    fg_threshold: Option<f64>,
    bg_threshold: Option<f64>,
    strict: bool,
    foreground_colors: Vec<[u8; 3]>,
) -> [Rgba<u8>; 4] {
    let temp_dir = TempDir::new().unwrap();
    let mut img = RgbaImage::new(4, 1);
    for (x, px) in palette.iter().enumerate() {
        img.put_pixel(x as u32, 0, Rgba(*px));
    }
    let input_path = temp_dir.path().join("in.png");
    let output_path = temp_dir.path().join("out.png");
    DynamicImage::ImageRgba8(img).save(&input_path).unwrap();

    process_image(
        &input_path,
        &output_path,
        foreground_colors,
        background,
        ProcessOptions {
            strict_mode: strict,
            fg_threshold,
            bg_threshold,
            ..Default::default()
        },
    )
    .unwrap();

    let out = image::open(&output_path).unwrap().to_rgba8();
    [
        *out.get_pixel(0, 0),
        *out.get_pixel(1, 0),
        *out.get_pixel(2, 0),
        *out.get_pixel(3, 0),
    ]
}

#[test]
fn test_bg_threshold_default_zero_does_not_snap_near_background() {
    // Without --bg-threshold, a near-black pixel should NOT be fully transparent;
    // it should pick up a small non-zero alpha (the algorithm's existing behavior).
    let bg = [0, 0, 0];
    let palette = [
        [0, 0, 0, 255],   // exact bg
        [1, 0, 0, 255],   // 1/255 off
        [3, 3, 3, 255],   // 3/255 off
        [255, 0, 0, 255], // clearly foreground
    ];
    let out = process_palette(palette, bg, None, None, false, vec![]);

    // Exact background snaps regardless (existing behavior)
    assert_eq!(out[0][3], 0, "exact bg should be fully transparent");
    // Near-bg pixels should NOT snap when bg_threshold is not set — they keep small alpha
    assert!(
        out[1][3] > 0,
        "near-bg pixel #1 should retain non-zero alpha without --bg-threshold, got alpha={}",
        out[1][3]
    );
    assert!(
        out[2][3] > 0,
        "near-bg pixel #3 should retain non-zero alpha without --bg-threshold, got alpha={}",
        out[2][3]
    );
    assert!(out[3][3] > 0, "true fg should stay opaque-ish");
}

#[test]
fn test_bg_threshold_snaps_near_black_pixels_to_transparent() {
    // With --bg-threshold ≈ 4/255, pixels within 4 per-channel of pure black
    // should become fully transparent.
    let bg = [0, 0, 0];
    let palette = [
        [0, 0, 0, 255],  // exact bg
        [1, 0, 0, 255],  // 1/255 off — within
        [3, 3, 3, 255],  // 3/255 off — within
        [10, 0, 0, 255], // 10/255 off — outside
    ];
    let out = process_palette(palette, bg, None, Some(0.016), false, vec![]); // 0.016 ≈ 4/255

    assert_eq!(out[0][3], 0, "exact bg snaps");
    assert_eq!(out[1][3], 0, "#010000 snaps with bg_threshold≈4/255");
    assert_eq!(out[2][3], 0, "#030303 snaps with bg_threshold≈4/255");
    assert!(
        out[3][3] > 0,
        "10/255 off must NOT snap, got alpha={}",
        out[3][3]
    );
}

#[test]
fn test_bg_threshold_works_off_non_black_background() {
    // Confirm the snap is relative to any background color, not hardcoded black.
    let bg = [200, 100, 50];
    let palette = [
        [200, 100, 50, 255], // exact bg
        [201, 99, 51, 255],  // 1/255 off — within
        [200, 100, 53, 255], // 3/255 off — within
        [220, 100, 50, 255], // 20/255 off — outside
    ];
    let out = process_palette(palette, bg, None, Some(0.016), false, vec![]);

    assert_eq!(out[0][3], 0);
    assert_eq!(out[1][3], 0, "near-bg snaps");
    assert_eq!(out[2][3], 0, "near-bg snaps");
    assert!(
        out[3][3] > 0,
        "far from bg must NOT snap, got alpha={}",
        out[3][3]
    );
}

#[test]
fn test_bg_threshold_l_infinity_metric_not_euclidean() {
    // Key behavioral choice: distance is L∞ (per-channel max), not L2 (Euclidean).
    // A pixel at delta (5, 5, 5) has L∞ = 5 but L2 ≈ 8.66. With bg_threshold ≈ 6/255,
    // L∞ would snap, L2 would NOT. The test asserts L∞ behavior.
    let bg = [0, 0, 0];
    let palette = [
        [0, 0, 0, 255],  // exact bg (control)
        [5, 5, 5, 255],  // L∞=5 (snap), L2≈8.66 (would not snap if Euclidean)
        [0, 0, 0, 255],  // exact bg (control)
        [10, 0, 0, 255], // L∞=10 (outside) — control: this must NOT snap
    ];
    let out = process_palette(palette, bg, None, Some(0.024), false, vec![]); // ~6.1/255

    assert_eq!(
        out[1][3], 0,
        "L∞=5 should snap at threshold ≈6/255 (L∞ metric)"
    );
    assert!(out[3][3] > 0, "L∞=10 must not snap at threshold ≈6/255");
}

#[test]
fn test_bg_threshold_applies_in_strict_mode() {
    // Snap should fire before strict-mode unmixing too.
    let bg = [0, 0, 0];
    let palette = [
        [0, 0, 0, 255],   // exact bg
        [2, 0, 0, 255],   // 2/255 off — within
        [255, 0, 0, 255], // pure red — foreground
        [128, 0, 0, 255], // half-mixed red — foreground
    ];
    let out = process_palette(
        palette,
        bg,
        None,
        Some(0.016),
        true,
        vec![[255, 0, 0]], // strict: red foreground
    );

    assert_eq!(out[0][3], 0);
    assert_eq!(out[1][3], 0, "near-bg snaps even in strict mode");
    assert!(
        out[2][3] > 200,
        "pure red should be ~opaque, got alpha={}",
        out[2][3]
    );
    assert!(out[3][3] > 0, "half-red should have non-zero alpha");
}

#[test]
fn test_bg_threshold_snaps_translucent_input_pixels() {
    // A translucent input pixel is pre-composited over the background before
    // unmixing. If the composite lands near the background, the snap should
    // fire — even though the raw input color was nowhere near it.
    let temp_dir = TempDir::new().unwrap();
    let mut img = RgbaImage::new(2, 1);
    // Raw RGB is mid-gray, but alpha=5/255 ≈ 2% means it composites to ~[2,2,2]
    // over a black background — well within --bg-threshold ≈ 4/255.
    img.put_pixel(0, 0, Rgba([128, 128, 128, 5]));
    // Control: an opaque pixel clearly above the snap threshold.
    img.put_pixel(1, 0, Rgba([200, 200, 200, 255]));

    let input_path = temp_dir.path().join("in.png");
    let output_path = temp_dir.path().join("out.png");
    DynamicImage::ImageRgba8(img).save(&input_path).unwrap();

    process_image(
        &input_path,
        &output_path,
        vec![],
        [0, 0, 0],
        ProcessOptions {
            bg_threshold: Some(0.016), // ~4/255
            ..Default::default()
        },
    )
    .unwrap();

    let out = image::open(&output_path).unwrap().to_rgba8();
    assert_eq!(
        out.get_pixel(0, 0).0,
        [0, 0, 0, 0],
        "translucent input compositing to near-bg should snap to fully transparent"
    );
    assert!(
        out.get_pixel(1, 0).0[3] > 0,
        "opaque mid-gray must not snap, got alpha={}",
        out.get_pixel(1, 0).0[3]
    );
}

#[test]
fn test_bg_threshold_combines_with_trim() {
    // Mostly-bg image with a small content region. Without --bg-threshold the
    // near-bg pixels keep tiny alphas so --trim can't crop them; with
    // --bg-threshold they snap to fully transparent and --trim drops them.
    let temp_dir = TempDir::new().unwrap();
    let mut img = RgbaImage::new(5, 1);
    img.put_pixel(0, 0, Rgba([1, 0, 0, 255])); // near-bg, will snap
    img.put_pixel(1, 0, Rgba([2, 2, 2, 255])); // near-bg, will snap
    img.put_pixel(2, 0, Rgba([255, 0, 0, 255])); // foreground content
    img.put_pixel(3, 0, Rgba([0, 1, 0, 255])); // near-bg, will snap
    img.put_pixel(4, 0, Rgba([3, 0, 3, 255])); // near-bg, will snap

    let input_path = temp_dir.path().join("in.png");
    let output_path = temp_dir.path().join("out.png");
    DynamicImage::ImageRgba8(img).save(&input_path).unwrap();

    process_image(
        &input_path,
        &output_path,
        vec![],
        [0, 0, 0],
        ProcessOptions {
            bg_threshold: Some(0.016),
            trim: true,
            ..Default::default()
        },
    )
    .unwrap();

    let out = image::open(&output_path).unwrap().to_rgba8();
    assert_eq!(
        out.dimensions(),
        (1, 1),
        "after snap+trim, only the single fg pixel should remain"
    );
    let kept = out.get_pixel(0, 0);
    assert!(
        kept[3] > 0,
        "the surviving pixel must be non-transparent, got alpha={}",
        kept[3]
    );
}
