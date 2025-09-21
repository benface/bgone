# bgone

Ultra-fast CLI tool for removing solid background colors from images using color unmixing.

## Features

- **Blazing fast** - Written in Rust with parallel processing
- **Multiple foreground colors** - Handles images with multiple foreground colors mixed with the background
- **Auto background detection** - Automatically detects background color from image edges
- **Auto color deduction** - Can automatically find unknown foreground colors using the `auto` keyword
- **Opacity optimization** - Maximizes opacity while maintaining exact color accuracy
- **Progress tracking** - Real-time progress bar during processing
- **Precise color unmixing** - Uses least-squares optimization for accurate color separation

## Usage

```bash
# Single foreground color with auto-detected background
bgone input.png output.png --fg=#ff0000

# Using shorthand color notation
bgone input.png output.png --fg=#f00 --bg=#fff

# Multiple foreground colors
bgone input.png output.png --fg f00 0f0 00f

# Automatic color deduction - finds unknown colors
bgone input.png output.png --fg auto
bgone input.png output.png --fg auto auto --bg fff

# Mix known and unknown colors
bgone input.png output.png --fg ff0000 auto
bgone input.png output.png --fg f00 auto 00f

# Multiple colors with # prefix still works, but requires quotes in shell
bgone input.png output.png --fg "#f00" "#0f0" "#00f"

# Mix of shorthand and full notation
bgone input.png output.png --fg ff0000 0f0 00f --bg fff
```

## How it works

The tool uses a color unmixing algorithm to determine how much of each foreground color and the background color contributed to each pixel. It then reconstructs the image with proper alpha transparency.

When using the `auto` keyword, bgone:
1. Analyzes all colors in the image
2. Calculates what unmixed foreground colors could produce the observed blended colors
3. Evaluates different color combinations to find the best match
4. Optimizes for maximum opacity while preserving exact color accuracy

## Project Structure

```
src/
├── main.rs        # CLI entry point
├── lib.rs         # Main image processing logic
├── color.rs       # Color types and utilities
├── background.rs  # Background detection
├── deduce.rs      # Automatic color deduction
└── unmix.rs       # Color unmixing algorithm
```

## Building

```bash
cargo build --release
```

## Running Locally

You can test the tool without installing it using `cargo run`:

```bash
# Debug mode (faster compilation, slower execution)
cargo run -- input.png output.png --fg ff0000

# Release mode (slower compilation, much faster execution)
cargo run --release -- input.png output.png --fg ff0000
```

The `--` separates cargo's arguments from bgone's arguments.

## Testing

The project includes a comprehensive testing framework that validates the color unmixing algorithm by:

1. Processing test images to remove backgrounds
2. Overlaying the results back onto the original background color
3. Comparing with the original image to ensure accuracy

### Running Tests

```bash
# Run all tests
cargo test -- --nocapture

# Generate test fixtures (only needed once)
cargo test --test generate_fixtures -- --ignored

# Run integration tests with output
cargo test --test integration_test -- --nocapture
```

### Test Results

The algorithm achieves excellent results:

- **Simple cases** (solid colors): 100% similarity, infinite PSNR
- **Complex cases** (gradients, multiple colors): 100% similarity, >50 dB PSNR

PSNR values above 40 dB indicate excellent quality reconstruction.

### Test Coverage

- **Unit tests**: Cover color parsing, normalization, background detection, color unmixing algorithm, and image overlaying
- **Integration tests**: Test the full CLI workflow including auto-background detection
- **Validation approach**: Process image → overlay on background → compare with original
