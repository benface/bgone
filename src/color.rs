use anyhow::{Context, Result};

/// Multiplier to expand hex color shorthand (e.g., F -> FF)
const HEX_SHORTHAND_MULTIPLIER: u8 = 17;

/// RGB color represented as [R, G, B] with values 0-255
pub type Color = [u8; 3];

/// Normalized RGB color with values 0.0-1.0
pub type NormalizedColor = [f64; 3];

/// A foreground color specification - either known or unknown
#[derive(Debug, Clone, PartialEq)]
pub enum ForegroundColorSpec {
    /// A known color specified by the user
    Known(Color),
    /// An unknown color to be deduced by the algorithm
    Unknown,
}

/// Parse a hex color string into RGB
/// Supports: "#ff0000", "ff0000", "#f00", "f00"
pub fn parse_hex_color(hex: &str) -> Result<Color> {
    let hex = hex.trim_start_matches('#');

    let (r, g, b) = match hex.len() {
        3 => {
            // Expand shorthand: "f00" -> "ff0000"
            let r = u8::from_str_radix(&hex[0..1], 16).context("Invalid red component")?;
            let g = u8::from_str_radix(&hex[1..2], 16).context("Invalid green component")?;
            let b = u8::from_str_radix(&hex[2..3], 16).context("Invalid blue component")?;
            (
                r * HEX_SHORTHAND_MULTIPLIER,
                g * HEX_SHORTHAND_MULTIPLIER,
                b * HEX_SHORTHAND_MULTIPLIER,
            )
        }
        6 => {
            // Full hex color
            let r = u8::from_str_radix(&hex[0..2], 16).context("Invalid red component")?;
            let g = u8::from_str_radix(&hex[2..4], 16).context("Invalid green component")?;
            let b = u8::from_str_radix(&hex[4..6], 16).context("Invalid blue component")?;
            (r, g, b)
        }
        _ => anyhow::bail!("Hex color must be 3 or 6 characters long (got: {})", hex),
    };

    Ok([r, g, b])
}

/// Parse a foreground color specification
/// Can be either a hex color or "auto" for unknown
pub fn parse_foreground_spec(spec: &str) -> Result<ForegroundColorSpec> {
    if spec == "auto" {
        Ok(ForegroundColorSpec::Unknown)
    } else {
        parse_hex_color(spec).map(ForegroundColorSpec::Known)
    }
}

/// Convert a Color to NormalizedColor
pub fn normalize_color(color: Color) -> NormalizedColor {
    [
        color[0] as f64 / 255.0,
        color[1] as f64 / 255.0,
        color[2] as f64 / 255.0,
    ]
}

/// Convert a NormalizedColor back to Color
pub fn denormalize_color(color: NormalizedColor) -> Color {
    [
        (color[0] * 255.0).round().clamp(0.0, 255.0) as u8,
        (color[1] * 255.0).round().clamp(0.0, 255.0) as u8,
        (color[2] * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        // Full format with # prefix
        assert_eq!(parse_hex_color("#ff0000").unwrap(), [255, 0, 0]);
        assert_eq!(parse_hex_color("#00ff00").unwrap(), [0, 255, 0]);
        assert_eq!(parse_hex_color("#0000ff").unwrap(), [0, 0, 255]);

        // Full format without # prefix
        assert_eq!(parse_hex_color("ff0000").unwrap(), [255, 0, 0]);
        assert_eq!(parse_hex_color("00ff00").unwrap(), [0, 255, 0]);
        assert_eq!(parse_hex_color("0000ff").unwrap(), [0, 0, 255]);

        // Shorthand with # prefix
        assert_eq!(parse_hex_color("#f00").unwrap(), [255, 0, 0]);
        assert_eq!(parse_hex_color("#0f0").unwrap(), [0, 255, 0]);
        assert_eq!(parse_hex_color("#00f").unwrap(), [0, 0, 255]);
        assert_eq!(parse_hex_color("#fff").unwrap(), [255, 255, 255]);
        assert_eq!(parse_hex_color("#000").unwrap(), [0, 0, 0]);
        assert_eq!(parse_hex_color("#369").unwrap(), [51, 102, 153]); // 3*17=51, 6*17=102, 9*17=153

        // Shorthand without # prefix
        assert_eq!(parse_hex_color("f00").unwrap(), [255, 0, 0]);
        assert_eq!(parse_hex_color("0f0").unwrap(), [0, 255, 0]);
        assert_eq!(parse_hex_color("00f").unwrap(), [0, 0, 255]);

        // Error cases
        assert!(parse_hex_color("ff").is_err()); // Wrong length
        assert!(parse_hex_color("ffff").is_err()); // Wrong length
        assert!(parse_hex_color("#gggggg").is_err()); // Invalid hex
        assert!(parse_hex_color("#ggg").is_err()); // Invalid hex shorthand
    }

    #[test]
    fn test_normalize_color() {
        assert_eq!(normalize_color([255, 127, 0]), [1.0, 127.0 / 255.0, 0.0]);
    }

    #[test]
    fn test_parse_foreground_spec() {
        // Test unknown placeholder
        assert_eq!(
            parse_foreground_spec("auto").unwrap(),
            ForegroundColorSpec::Unknown
        );

        // Test known colors
        assert_eq!(
            parse_foreground_spec("#ff0000").unwrap(),
            ForegroundColorSpec::Known([255, 0, 0])
        );
        assert_eq!(
            parse_foreground_spec("f00").unwrap(),
            ForegroundColorSpec::Known([255, 0, 0])
        );

        // Test invalid input
        assert!(parse_foreground_spec("invalid").is_err());
    }
}
