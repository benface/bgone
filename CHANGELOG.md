# Changelog

All notable changes to this project will be documented in this file.

## [0.6.0] - 2026-06-09

### Added

- **Batch processing**: process multiple images in one command, in parallel
  - Pass paths individually: `bgone a.png b.png c.png`
  - Or quote a glob and let bgone expand it: `bgone 'images/*.png'` (recommended — also works on Windows)
  - Or let your shell expand a glob on Unix-like systems: `bgone images/*.png`
- `--out-dir <PATH>` flag: route all batched outputs into a single directory (created automatically if missing). Omit it and each output lands next to its input with the usual `-bgone` suffix
- `--bg-threshold FLOAT` flag: force near-background pixels to fully transparent. Useful for cleaning up JPEG artifacts and faint color noise that previously left a ghost when the image was composited
  - Per-channel distance in `0.0`–`1.0` (`0.004` ≈ 1 RGB unit per channel, `0.02` ≈ 5)
  - Default `0.0` preserves the existing behavior (only exact background matches snap)

### Changed

- **BREAKING**: `--threshold` (and the `-t` shorthand) renamed to `--fg-threshold` for symmetry with the new `--bg-threshold`. Existing scripts using either form will need to update
- `bgone input.png output.png` still works for single-file output, but bgone now refuses to overwrite an existing file in the output slot. This prevents the case where `bgone images/*.png` matches exactly two files and silently treats one as the output. To process two (or any number of) files as a batch, use `--out-dir`
- **Library API (BREAKING)**: `process_image` now takes a `ProcessOptions` struct in place of a long positional arg list; `process_image_with_options` is removed (the `quiet` flag moves into `ProcessOptions`). See the README for the new signature

### Fixed

- Library callers passing `strict_mode: true` with no foreground colors now get a clear error instead of silent nonsense (the CLI already rejected this case)

## [0.5.0] - 2025-12-07

### Added

- `--trim` CLI flag to crop output images to the bounding box of non-transparent pixels
- `trim_to_content()` public function in library API
- 8 new tests for trim functionality covering various edge cases

### Changed

- Updated all dependencies via `cargo update`
- Migrated from deprecated `assert_cmd::Command::cargo_bin()` to `cargo::cargo_bin!()` macro in tests
- Fixed clippy `items_after_test_module` warning by reorganizing lib.rs

## [0.4.0] - 2025-10-06

### Added

- Intelligent alpha channel handling: translucent input pixels are pre-composited over the background color
- Smart output format selection: formats without alpha support (JPEG, BMP) auto-convert to PNG
- Comprehensive "Supported Formats" documentation section
- 5 unit tests for alpha channel compositing
- 2 unit tests for format conversion behavior

### Changed

- Upgraded to Rust Edition 2024
- Applied clippy improvements: RangeInclusive::contains(), let-chains, unwrap_or optimization
- Simplified README documentation for better readability
- Background auto-detection now composites translucent edge pixels over black

### Fixed

- Images with existing alpha channels now process correctly instead of being treated as fully opaque
- Case-insensitive file extension handling

## [0.3.0] - 2025-10-06

### Added

- Optional output argument: automatically generates output filename with `-bgone` suffix when not specified
- Auto-incremental naming: appends `-bgone-1`, `-bgone-2`, etc. when file already exists
- Comprehensive unit tests for output path generation logic

### Changed

- Output argument is now optional in CLI (defaults to `<input>-bgone.<ext>`)
- Updated all documentation and usage examples to reflect optional output
- Simplified common usage pattern: `bgone input.png` instead of `bgone input.png output.png`

## [0.2.0] - 2024-09-21

### Added

- Tie-breaker logic in color deduction to prefer colors furthest from background when reconstruction quality is equal
- Mixed mode tests combining known and auto colors (e.g., `--fg fff auto auto`)
- More comprehensive test coverage for translucent recovery scenarios
- Named constants for algorithm thresholds

### Changed

- Improved color deduction algorithm to find more optimal colors (e.g., pure RGB when appropriate)
- Increased candidate thresholds for better color selection:
  - 2 unknowns: 20 → 30 candidates
  - 3 unknowns: 20 → 25 candidates (full search) or 20 (selected)
- Renamed "fully_auto" tests to "non_strict" for clarity
- Updated README to run tests with `--release` flag by default for better performance

### Fixed

- Color deduction now properly handles cases with 3 unknown colors and many candidates
- Test assertion messages now use consistent format
- Reconstructed test images now correctly show the processed output overlaid on background

## [0.1.0] - Initial Release

### Features

- Ultra-fast background removal using color unmixing
- Support for multiple foreground colors
- Automatic background color detection
- Foreground color deduction with `auto` keyword
- Strict and non-strict modes
- Flexible opacity optimization
- Comprehensive test suite
