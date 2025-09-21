# bgone

Ultra-fast CLI tool for removing solid background colors from images using color unmixing.

## Features

- **Blazing fast** - Written in Rust with parallel processing
- **Multiple foreground colors** - Handles images with multiple foreground colors mixed with the background
- **Background color detection** - Automatically detects background color from image edges
- **Foreground color deduction** - Can automatically find unknown foreground colors using the `auto` keyword
- **Flexible modes** - Strict mode for exact color matching, or non-strict mode for more natural transparency
- **Opacity optimization** - Intelligently optimizes opacity based on mode and colors
- **Precise color unmixing** - Uses least-squares optimization for accurate color separation

## Installation

### Using Homebrew (macOS/Linux)

```bash
brew tap benface/bgone
brew install bgone
```

### Using Cargo

```bash
cargo install bgone
```

## Usage

### Non-Strict Mode (Default)

In non-strict mode, bgone can use any color needed to perfectly reconstruct the image while making the background transparent.

```bash
# Fully automatic - detects the background and removes it
bgone input.png output.png

# With background color - overrides automatic detection
bgone input.png output.png --bg=#ffffff

# With foreground color - optimizes for high opacity when pixels match this color (within a threshold)
bgone input.png output.png --fg=#ff0000

# Multiple foreground colors - output pixels can be any mix of these colors
bgone input.png output.png --fg ff0000 00ff00 0000ff

# Foreground color deduction - uses a known amount of unknown colors
bgone input.png output.png --fg auto
bgone input.png output.png --fg auto auto --bg ffffff

# Mix known and unknown colors
bgone input.png output.png --fg ff0000 auto

# Using shorthand notation
bgone input.png output.png -f f00 -b fff
bgone input.png output.png -f auto -s
bgone input.png output.png -f f00 0f0 00f -b fff -t 0.1
```

### Strict Mode

Strict mode restricts unmixing to only the specified foreground colors, ensuring exact color matching.

```bash
# Strict mode requires --fg, but supports both known and unknown colors
bgone input.png output.png --strict --fg=#ff0000
bgone input.png output.png --strict --fg auto
bgone input.png output.png --strict --fg ff0000 auto

# With specific background color
bgone input.png output.png --strict --fg=#f00 --bg=#fff
```

### Additional Examples

```bash
# Multiple colors with # prefix still works, but requires quotes in shell
bgone input.png output.png --fg "#f00" "#0f0" "#00f"

# Mix of shorthand and full notation
bgone input.png output.png --fg ff0000 0f0 00f --bg fff
```

## CLI Options

- `input` - Path to the input image
- `output` - Path for the output image
- `-f, --fg COLOR...` - Foreground colors in hex format (e.g., `f00`, `ff0000`, `#ff0000`) or `auto` to deduce unknown colors
  - Optional in non-strict mode
  - Required in strict mode
- `-b, --bg COLOR` - Background color in hex format
  - If not specified, automatically detects the background color
- `-s, --strict` - Enable strict mode (requires `--fg` and restricts to specified colors only)
- `-t, --threshold FLOAT` - Color similarity threshold (`0.0`-`1.0`, default: `0.05`)
  - When using one or multiple `auto` foreground colors: colors within this threshold are considered similar during deduction
  - When using any `--fg` in non-strict mode: pixels within this threshold of a (known or deduced) foreground color will use that color
- `-h, --help` - Print help information
- `-v, --version` - Print version information

## How it works

The tool uses a color unmixing algorithm to determine how much of each foreground color and the background color contributed to each pixel. It then reconstructs the image with proper alpha transparency.

### Non-Strict Mode (Default)

- **Without foreground colors**: Finds the optimal color and transparency for each pixel to perfectly reconstruct the image. Uses the maximum transparency (minimum opacity) possible for each pixel.
- **With foreground colors**:
  - Pixels within the threshold distance (default: 5%) of specified foreground colors use those colors with high opacity
  - Other pixels (like glows, shadows, or gradients) can use ANY color needed for perfect reconstruction
  - Always prioritizes correctness - every pixel is perfectly reconstructed

### Strict Mode

- Requires foreground colors to be specified
- Restricts unmixing to only the specified colors
- Optimizes for maximum opacity while maintaining exact color accuracy
- Best for images with known, specific foreground colors

### Foreground Color Deduction

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
├── deduce.rs      # Foreground color deduction
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

# Generate test inputs (only needed once)
cargo test --test generate_inputs -- --ignored

# Run specific test suites
cargo test --test strict_tests -- --nocapture
cargo test --test non_strict_tests -- --nocapture
cargo test --test color_deduction_tests -- --nocapture
```

### Test Results

The algorithm achieves excellent results:

- **Simple cases** (solid colors): 100% similarity, infinite PSNR
- **Complex cases** (gradients, multiple colors): 100% similarity, >50 dB PSNR

PSNR values above 40 dB indicate excellent quality reconstruction.

### Test Coverage

- **Unit tests**: Cover color parsing, normalization, background detection, color unmixing algorithm, and image overlaying
- **Integration tests**: Comprehensive tests for strict mode, non-strict mode, and color deduction
- **Validation approach**: Process image → overlay on background → compare with original
