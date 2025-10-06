# Changelog

All notable changes to this project will be documented in this file.

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