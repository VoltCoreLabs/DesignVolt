//! Image fills: decoding PNG and JPEG files and drawing them in a shape.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use dv_core::{ImageBytes, ImageFill, ImageMode, Page};
use vello::Scene;
use vello::kurbo::{Affine, Rect, Shape};
use vello::peniko::{
    Blob, Brush, Extend, Fill, ImageAlphaType, ImageBrush, ImageData, ImageFormat,
};

/// Largest side of an image, in pixels.
pub const MAX_SIDE: u32 = 8192;

/// A decoded image: straight (not premultiplied) RGBA, 8 bits per channel.
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Decodes a PNG or JPEG file; `None` for anything else, a damaged file or a huge image.
pub fn decode(bytes: &[u8]) -> Option<Decoded> {
    let d = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        decode_png(bytes)?
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        decode_jpeg(bytes)?
    } else {
        return None;
    };
    let ok = d.width > 0
        && d.height > 0
        && d.width <= MAX_SIDE
        && d.height <= MAX_SIDE
        && d.rgba.len() == d.width as usize * d.height as usize * 4;
    ok.then_some(d)
}

fn decode_png(bytes: &[u8]) -> Option<Decoded> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let info = reader.info();
    if info.width > MAX_SIDE || info.height > MAX_SIDE {
        return None;
    }
    let mut buf = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buf).ok()?;
    let data = &buf[..frame.buffer_size()];
    let rgba = match frame.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => data
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Indexed => return None,
    };
    Some(Decoded {
        width: frame.width,
        height: frame.height,
        rgba,
    })
}

fn decode_jpeg(bytes: &[u8]) -> Option<Decoded> {
    let mut decoder =
        zune_jpeg::JpegDecoder::new(zune_jpeg::zune_core::bytestream::ZCursor::new(bytes));
    decoder.decode_headers().ok()?;
    let info = decoder.info()?;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    if width > MAX_SIDE || height > MAX_SIDE {
        return None;
    }
    let rgb = decoder.decode().ok()?;
    if rgb.len() != width as usize * height as usize * 3 {
        return None;
    }
    let rgba = rgb
        .as_chunks::<3>()
        .0
        .iter()
        .flat_map(|p| [p[0], p[1], p[2], 255])
        .collect();
    Some(Decoded {
        width,
        height,
        rgba,
    })
}

/// Images already decoded, by asset id. An entry keeps the file bytes it came from, so an id
/// used by another document (or a replaced file) is decoded again.
type Cache = HashMap<u64, (ImageBytes, Option<ImageData>)>;

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(HashMap::new());
}

/// Most decoded images kept; the cache is dropped when it grows past this.
const CACHE_LIMIT: usize = 64;

fn cached(asset: u64, bytes: &ImageBytes) -> Option<ImageData> {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((have, img)) = c.get(&asset)
            && Arc::ptr_eq(have, bytes)
        {
            return img.clone();
        }
        if c.len() >= CACHE_LIMIT {
            c.clear();
        }
        let img = decode(bytes).map(|d| ImageData {
            data: Blob::new(Arc::new(d.rgba)),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::Alpha,
            width: d.width,
            height: d.height,
        });
        c.insert(asset, (bytes.clone(), img.clone()));
        img
    })
}

/// Where the image goes inside a `w` x `h` box: maps image pixels to box coordinates.
pub fn placement(mode: ImageMode, iw: f64, ih: f64, w: f64, h: f64, scale: f64) -> Affine {
    let (sx, sy) = match mode {
        ImageMode::Fill => {
            let s = (w / iw).max(h / ih);
            (s, s)
        }
        ImageMode::Fit => {
            let s = (w / iw).min(h / ih);
            (s, s)
        }
        ImageMode::Tile => (scale, scale),
        ImageMode::Stretch => (w / iw, h / ih),
    };
    let (tx, ty) = match mode {
        ImageMode::Tile | ImageMode::Stretch => (0.0, 0.0),
        _ => ((w - iw * sx) / 2.0, (h - ih * sy) / 2.0),
    };
    Affine::translate((tx, ty)) * Affine::scale_non_uniform(sx, sy)
}

/// Fills `shape` (inside a `w` x `h` box) with the image of `fill`.
pub fn draw(
    scene: &mut Scene,
    page: &Page,
    world: Affine,
    shape: &impl Shape,
    (w, h): (f64, f64),
    fill: &ImageFill,
) {
    let Some(bytes) = page.image(fill.asset) else {
        return;
    };
    let Some(img) = cached(fill.asset, bytes) else {
        return;
    };
    let (iw, ih) = (f64::from(img.width), f64::from(img.height));
    let place = placement(fill.mode, iw, ih, w, h, fill.scale);
    let brush = ImageBrush::new(img);
    if fill.mode == ImageMode::Tile {
        let brush = Brush::Image(brush.with_extend(Extend::Repeat));
        scene.fill(Fill::NonZero, world, &brush, Some(place), shape);
        return;
    }
    scene.push_clip_layer(Fill::NonZero, world, shape);
    let rect = Rect::new(0.0, 0.0, iw, ih);
    scene.fill(
        Fill::NonZero,
        world * place,
        &Brush::Image(brush),
        None,
        &rect,
    );
    scene.pop_layer();
}

#[cfg(test)]
mod tests {
    use super::*;
    use vello::kurbo::Point;

    #[test]
    fn fill_covers_and_fit_contains() {
        let fill = placement(ImageMode::Fill, 200.0, 100.0, 100.0, 100.0, 1.0);
        // The wide image is scaled to the height and centered: it overflows left and right.
        assert_eq!(fill * Point::new(0.0, 0.0), Point::new(-50.0, 0.0));
        assert_eq!(fill * Point::new(200.0, 100.0), Point::new(150.0, 100.0));
        let fit = placement(ImageMode::Fit, 200.0, 100.0, 100.0, 100.0, 1.0);
        assert_eq!(fit * Point::new(0.0, 0.0), Point::new(0.0, 25.0));
        assert_eq!(fit * Point::new(200.0, 100.0), Point::new(100.0, 75.0));
        let stretch = placement(ImageMode::Stretch, 200.0, 100.0, 100.0, 50.0, 1.0);
        assert_eq!(stretch * Point::new(200.0, 100.0), Point::new(100.0, 50.0));
        let tile = placement(ImageMode::Tile, 20.0, 10.0, 100.0, 100.0, 2.0);
        assert_eq!(tile * Point::new(20.0, 10.0), Point::new(40.0, 20.0));
    }

    /// A 2x1 RGBA PNG: an opaque red pixel and a half transparent blue one.
    fn tiny_png() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, 2, 1);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&[255, 0, 0, 255, 0, 0, 255, 128])
                .unwrap();
        }
        out
    }

    #[test]
    fn png_files_decode_and_garbage_does_not() {
        let d = decode(&tiny_png()).unwrap();
        assert_eq!((d.width, d.height), (2, 1));
        assert_eq!(d.rgba, [255, 0, 0, 255, 0, 0, 255, 128]);
        assert!(decode(b"not an image").is_none());
        assert!(decode(&tiny_png()[..20]).is_none(), "truncated file");
        assert!(decode(&[0xFF, 0xD8, 1, 2, 3]).is_none(), "broken jpeg");
    }
}
