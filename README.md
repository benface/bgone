# bgone

Ultra-fast CLI tool for removing solid background colors from images using color unmixing.

## Features

- **Blazing fast** - Written in Rust with parallel processing
- **Multiple foreground colors** - Handles images with multiple foreground colors mixed with the background
- **Auto background detection** - Automatically detects background color from image edges
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

# Multiple colors with # prefix still works, but requires quotes in shell
bgone input.png output.png --fg "#f00" "#0f0" "#00f"

# Mix of shorthand and full notation
bgone input.png output.png --fg ff0000 0f0 00f --bg fff
```

## How it works

The tool uses a color unmixing algorithm to determine how much of each foreground color and the background color contributed to each pixel. It then reconstructs the image with proper alpha transparency.

## Project Structure

```
src/
├── main.rs        # CLI entry point
├── lib.rs         # Main image processing logic
├── color.rs       # Color types and utilities
├── background.rs  # Background detection
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
cargo test

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
