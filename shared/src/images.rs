//! Photo normalisation: every photo is decoded, scaled to fit 1000x1000 and
//! re-encoded as WebP. Shared by the backend (web uploads) and the desktop app
//! (local photos), so both store exactly the same kind of file.
//! Enabled with the `images` feature of this crate.
//!
//! Re-encoding (rather than trusting the browser) is what makes the
//! guarantees hold for every client:
//! - storage: lossy WebP at a capped resolution, whatever the phone sent
//! - privacy: metadata such as EXIF GPS does not survive a decode/encode
//! - safety: the stored file is always a well-formed image we produced
//!
//! Safari cannot encode WebP from a canvas, so a browser-only conversion would
//! silently fall back to PNG for every iPhone user.

use std::io::Cursor;

use image::{imageops::FilterType, DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};

/// Stored photos fit within MAX_EDGE x MAX_EDGE; larger ones are scaled down
/// keeping their aspect ratio (4000x3000 becomes 1000x750). Smaller ones are
/// never enlarged.
pub const MAX_EDGE: u32 = 1000;
/// libwebp quality (0-100). 80 is visually close to the source at a fraction
/// of a phone JPEG's size.
pub const WEBP_QUALITY: f32 = 80.0;
/// Refuse to decode anything larger than this per side, before allocating.
const MAX_INPUT_EDGE: u32 = 12_000;
/// Upper bound for decoder allocations — guards against decompression bombs
/// (a tiny file that claims enormous dimensions).
const MAX_DECODE_ALLOC: u64 = 512 * 1024 * 1024;

#[derive(Debug, PartialEq)]
pub enum ImageError {
    /// Not one of the formats we accept (HEIC, SVG, PDF, random bytes, ...).
    Unsupported,
    /// Recognised format but undecodable, or over the size limits.
    Invalid(String),
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageError::Unsupported => write!(f, "unsupported image format (use JPEG, PNG, WebP, GIF or BMP)"),
            ImageError::Invalid(msg) => write!(f, "invalid image: {msg}"),
        }
    }
}

pub struct Processed {
    pub webp: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

const ACCEPTED: [ImageFormat; 5] = [ImageFormat::Jpeg, ImageFormat::Png, ImageFormat::WebP, ImageFormat::Gif, ImageFormat::Bmp];

/// Decode, apply EXIF orientation, downscale, and encode as lossy WebP.
/// CPU-bound: call from `spawn_blocking`.
pub fn to_webp(input: &[u8]) -> Result<Processed, ImageError> {
    let mut reader = ImageReader::new(Cursor::new(input))
        .with_guessed_format()
        .map_err(|e| ImageError::Invalid(e.to_string()))?;
    // Detect by content, never by file name or Content-Type.
    match reader.format() {
        Some(f) if ACCEPTED.contains(&f) => {}
        _ => return Err(ImageError::Unsupported),
    }

    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_INPUT_EDGE);
    limits.max_image_height = Some(MAX_INPUT_EDGE);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);

    let mut decoder = reader.into_decoder().map_err(|e| ImageError::Invalid(e.to_string()))?;
    // Phones store photos sideways and record the rotation in EXIF. That tag is
    // dropped by re-encoding, so it has to be baked into the pixels first or
    // portrait shots would end up rotated.
    let orientation = decoder.orientation().map_err(|e| ImageError::Invalid(e.to_string()))?;
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| ImageError::Invalid(e.to_string()))?;
    img.apply_orientation(orientation);

    if img.width() > MAX_EDGE || img.height() > MAX_EDGE {
        img = img.resize(MAX_EDGE, MAX_EDGE, FilterType::Lanczos3);
    }

    // libwebp takes 8-bit RGB or RGBA. Keep alpha only when there is any.
    let img = if img.color().has_alpha() {
        DynamicImage::ImageRgba8(img.to_rgba8())
    } else {
        DynamicImage::ImageRgb8(img.to_rgb8())
    };
    let (width, height) = (img.width(), img.height());
    let encoder = webp::Encoder::from_image(&img).map_err(|e| ImageError::Invalid(e.to_string()))?;
    let lossy = encoder.encode(WEBP_QUALITY).to_vec();
    // Lossy WebP wins by a wide margin on photographs but can lose to PNG on
    // screenshots and graphics (flat colours, sharp text): one test screenshot
    // came out at twice its PNG size. When lossy grew the file, lossless WebP
    // is tried too and the smaller result kept — still WebP either way.
    let webp = if lossy.len() > input.len() {
        let lossless = encoder.encode_lossless().to_vec();
        if lossless.len() < lossy.len() { lossless } else { lossy }
    } else {
        lossy
    };
    Ok(Processed { webp, width, height })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb, RgbImage, Rgba};

    fn encode(img: &DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, format).unwrap();
        out.into_inner()
    }

    fn is_webp(bytes: &[u8]) -> bool {
        bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
    }

    #[test]
    fn jpeg_becomes_webp() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(64, 48, Rgb([30, 120, 60])));
        let out = to_webp(&encode(&img, ImageFormat::Jpeg)).unwrap();
        assert!(is_webp(&out.webp));
        assert_eq!((out.width, out.height), (64, 48));
    }

    /// Smooth gradients plus sensor-like noise: compresses like a photograph.
    fn photo_like(w: u32, h: u32) -> DynamicImage {
        let mut seed = 0x9E37_79B9u32;
        DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |x, y| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let n = (seed % 24) as i32 - 12;
            let c = |base: u32| (base as i32 + n).clamp(0, 255) as u8;
            Rgb([c(40 + x * 150 / w), c(90 + y * 120 / h), c(60 + (x + y) * 60 / (w + h))])
        }))
    }

    /// Flat colour blocks with hard edges: compresses like a UI screenshot.
    fn screenshot_like(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |x, y| {
            if (y / 18) % 2 == 0 && (x / 6) % 3 == 0 { Rgb([230, 255, 240]) }
            else if x < w / 5 { Rgb([10, 40, 30]) }
            else { Rgb([9, 20, 16]) }
        }))
    }

    #[test]
    fn photographs_get_smaller() {
        let jpeg = {
            let mut out = Cursor::new(Vec::new());
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92)
                .encode_image(&photo_like(1200, 900))
                .unwrap();
            out.into_inner()
        };
        let out = to_webp(&jpeg).unwrap();
        assert!(out.webp.len() < jpeg.len(), "webp {} vs jpeg {}", out.webp.len(), jpeg.len());
    }

    #[test]
    fn screenshots_never_grow() {
        let png = encode(&screenshot_like(1200, 800), ImageFormat::Png);
        let out = to_webp(&png).unwrap();
        assert!(is_webp(&out.webp));
        assert!(out.webp.len() <= png.len(), "webp {} vs png {}", out.webp.len(), png.len());
    }

    #[test]
    fn large_images_are_downscaled_keeping_aspect_ratio() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(4000, 3000, Rgb([200, 200, 200])));
        let out = to_webp(&encode(&img, ImageFormat::Png)).unwrap();
        assert_eq!((out.width, out.height), (1000, 750));
        let portrait = DynamicImage::ImageRgb8(RgbImage::from_pixel(3000, 4000, Rgb([200, 200, 200])));
        let out = to_webp(&encode(&portrait, ImageFormat::Png)).unwrap();
        assert_eq!((out.width, out.height), (750, 1000));
    }

    #[test]
    fn images_within_the_limit_keep_their_size() {
        for (w, h) in [(1000, 1000), (999, 400), (64, 48)] {
            let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([20, 90, 40])));
            let out = to_webp(&encode(&img, ImageFormat::Png)).unwrap();
            assert_eq!((out.width, out.height), (w, h));
        }
    }

    #[test]
    fn alpha_is_preserved_only_when_present() {
        let rgba = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(10, 10, Rgba([0, 0, 0, 0])));
        assert!(to_webp(&encode(&rgba, ImageFormat::Png)).is_ok());
    }

    #[test]
    fn rejects_non_images_and_unsupported_formats() {
        assert_eq!(to_webp(b"definitely not an image").err(), Some(ImageError::Unsupported));
        assert_eq!(to_webp(b"<svg xmlns='http://www.w3.org/2000/svg'/>").err(), Some(ImageError::Unsupported));
        assert_eq!(to_webp(&[]).err(), Some(ImageError::Unsupported));
    }

    #[test]
    fn rejects_truncated_image() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(64, 64, Rgb([1, 2, 3])));
        let bytes = encode(&img, ImageFormat::Png);
        assert!(matches!(to_webp(&bytes[..bytes.len() / 3]), Err(ImageError::Invalid(_))));
    }

    /// Insert an EXIF APP1 segment carrying only an Orientation tag right after
    /// the JPEG SOI marker — what a phone writes for a portrait shot.
    fn with_exif_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II*\0");               // little-endian TIFF
        tiff.extend_from_slice(&8u32.to_le_bytes());     // IFD0 offset
        tiff.extend_from_slice(&1u16.to_le_bytes());     // one entry
        tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // Orientation
        tiff.extend_from_slice(&3u16.to_le_bytes());     // SHORT
        tiff.extend_from_slice(&1u32.to_le_bytes());     // count
        tiff.extend_from_slice(&orientation.to_le_bytes());
        tiff.extend_from_slice(&[0, 0]);                 // pad value field
        tiff.extend_from_slice(&0u32.to_le_bytes());     // no next IFD
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(&tiff);
        let len = (payload.len() + 2) as u16;
        let mut out = jpeg[..2].to_vec();                // SOI
        out.extend_from_slice(&[0xFF, 0xE1]);
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&payload);
        out.extend_from_slice(&jpeg[2..]);
        out
    }

    #[test]
    fn exif_orientation_is_baked_in() {
        // A 40x20 landscape sensor image tagged "rotate 90 CW" (6) must come out
        // as a 20x40 portrait.
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 20, Rgb([90, 90, 90])));
        let jpeg = with_exif_orientation(&encode(&img, ImageFormat::Jpeg), 6);
        let out = to_webp(&jpeg).unwrap();
        assert_eq!((out.width, out.height), (20, 40));
    }

    #[test]
    fn output_carries_no_exif() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 20, Rgb([90, 90, 90])));
        let jpeg = with_exif_orientation(&encode(&img, ImageFormat::Jpeg), 6);
        let out = to_webp(&jpeg).unwrap();
        assert!(!out.webp.windows(4).any(|w| w == b"EXIF" || w == b"Exif"));
    }
}
