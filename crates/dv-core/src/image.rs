//! Images as fills: how an image is placed in a shape, and the base64 used to keep the
//! image files inside the document file.

use std::sync::Arc;

/// How an image is placed in the shape it fills.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImageMode {
    /// Covers the whole shape, cropped at the edges.
    Fill,
    /// Fits inside the shape, keeping the proportions.
    Fit,
    /// Repeated at its own size (times `scale`).
    Tile,
    /// Like fill, but zoomed by `scale` and moved by `offset` (cropped by hand).
    Crop,
    /// Stretched to the shape.
    Stretch,
}

impl ImageMode {
    pub const ALL: [ImageMode; 5] = [
        ImageMode::Fill,
        ImageMode::Fit,
        ImageMode::Tile,
        ImageMode::Crop,
        ImageMode::Stretch,
    ];

    pub fn code(self) -> &'static str {
        match self {
            ImageMode::Fill => "fill",
            ImageMode::Fit => "fit",
            ImageMode::Tile => "tile",
            ImageMode::Crop => "crop",
            ImageMode::Stretch => "stretch",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.code() == s)
    }
}

/// A fill that shows an image stored in the page (`Page::image`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ImageFill {
    pub asset: u64,
    pub mode: ImageMode,
    /// Size of the tiles (tile mode) or the zoom (crop mode), relative to the image.
    pub scale: f64,
    /// Crop mode: how far the image is moved, as a fraction of the shape size.
    pub offset: (f64, f64),
}

impl ImageFill {
    pub fn new(asset: u64) -> Self {
        Self {
            asset,
            mode: ImageMode::Fill,
            scale: 1.0,
            offset: (0.0, 0.0),
        }
    }

    pub fn to_code(self) -> String {
        format!(
            "{};{};{};{};{}",
            self.asset,
            self.mode.code(),
            self.scale,
            self.offset.0,
            self.offset.1
        )
    }

    /// The offset columns are optional (older files end after the scale).
    pub fn from_code(s: &str) -> Option<Self> {
        let mut f = s.split(';');
        let asset = f.next()?.parse().ok()?;
        let mode = ImageMode::from_code(f.next()?)?;
        let num = |v: Option<&str>| v?.parse::<f64>().ok().filter(|v| v.is_finite());
        let scale = num(f.next())?;
        let ox = num(f.next()).unwrap_or(0.0);
        let oy = num(f.next()).unwrap_or(0.0);
        Some(Self {
            asset,
            mode,
            scale: scale.clamp(0.01, 50.0),
            offset: (ox.clamp(-50.0, 50.0), oy.clamp(-50.0, 50.0)),
        })
    }
}

/// The bytes of an image file, shared between the page and the commands.
pub type ImageBytes = Arc<Vec<u8>>;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        if c == b'=' {
            break;
        }
        let v = ALPHABET.iter().position(|a| *a == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_every_length() {
        for n in 0..40u8 {
            let data: Vec<u8> = (0..n)
                .map(|i| i.wrapping_mul(37).wrapping_add(11))
                .collect();
            assert_eq!(base64_decode(&base64_encode(&data)), Some(data));
        }
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert!(base64_decode("TW*u").is_none());
    }

    #[test]
    fn image_fill_codes_round_trip_and_reject_garbage() {
        let f = ImageFill {
            asset: 12,
            mode: ImageMode::Crop,
            scale: 1.5,
            offset: (0.25, -0.5),
        };
        assert_eq!(ImageFill::from_code(&f.to_code()), Some(f));
        assert!(ImageFill::from_code("12;nope;1").is_none());
        assert!(ImageFill::from_code("x;fill;1").is_none());
    }

    #[test]
    fn old_codes_without_an_offset_still_load() {
        let f = ImageFill::from_code("3;tile;0.5").unwrap();
        assert_eq!(
            (f.mode, f.scale, f.offset),
            (ImageMode::Tile, 0.5, (0.0, 0.0))
        );
    }
}
