use image::ExtendedColorType;
use image::ImageEncoder;
use image::codecs::jpeg::JpegEncoder;

use crate::clipboard::ClipboardImage;

pub const JPEG_QUALITY: u8 = 80;

pub fn info_text(image: &ClipboardImage) -> String {
    let png_len = image.png_bytes().ok().map(|bytes| bytes.len());
    let jpeg_len = jpeg_bytes(image, JPEG_QUALITY)
        .ok()
        .map(|bytes| bytes.len());
    let mut lines = vec![
        "Image".to_string(),
        format!("Size: {}×{}", image.width, image.height),
        format!("Aspect: {}", aspect_ratio(image.width, image.height)),
        format!("Pixels: {}", image.width.saturating_mul(image.height)),
    ];
    if let Some(len) = png_len {
        lines.push(format!("PNG: {}", format_bytes(len)));
    }
    if let Some(len) = jpeg_len {
        lines.push(format!("JPEG: {}", format_bytes(len)));
    }
    lines.join("\n")
}

pub fn jpeg_bytes(image: &ClipboardImage, quality: u8) -> Result<Vec<u8>, String> {
    let rgb = rgba_to_rgb(&image.rgba);
    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, quality)
        .write_image(
            &rgb,
            image.width as u32,
            image.height as u32,
            ExtendedColorType::Rgb8,
        )
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

pub fn try_jpeg(image: &ClipboardImage) -> Option<Vec<u8>> {
    let png = image.png_bytes().ok()?;
    let jpeg = jpeg_bytes(image, JPEG_QUALITY).ok()?;
    (jpeg.len() < png.len()).then_some(jpeg)
}

pub fn image_from_encoded(bytes: &[u8]) -> Option<ClipboardImage> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    ClipboardImage::new(img.width() as usize, img.height() as usize, img.into_raw())
}

fn rgba_to_rgb(rgba: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for px in rgba.as_chunks::<4>().0 {
        let a = u16::from(px[3]);
        let blend = |c: u8| ((u16::from(c) * a + 255 * (255 - a)) / 255) as u8;
        rgb.push(blend(px[0]));
        rgb.push(blend(px[1]));
        rgb.push(blend(px[2]));
    }
    rgb
}

fn aspect_ratio(width: usize, height: usize) -> String {
    let g = gcd(width, height);
    format!("{}:{}", width / g, height / g)
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let rest = a % b;
        a = b;
        b = rest;
    }
    a.max(1)
}

fn format_bytes(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1} MB", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1} KB", n as f64 / 1_000.0)
    } else {
        format!("{n} B")
    }
}

#[cfg(test)]
mod tests {
    use super::{aspect_ratio, format_bytes, image_from_encoded, info_text, jpeg_bytes, try_jpeg};
    use crate::clipboard::ClipboardImage;

    fn sample() -> ClipboardImage {
        let mut rgba = Vec::with_capacity(64 * 64 * 4);
        for y in 0u8..64 {
            for x in 0u8..64 {
                let n = x.wrapping_mul(37) ^ y.wrapping_mul(91);
                rgba.extend_from_slice(&[n, n.wrapping_add(x), n.wrapping_add(y), 255]);
            }
        }
        ClipboardImage::new(64, 64, rgba).expect("rgba")
    }

    #[test]
    fn info_includes_dimensions_and_sizes() {
        let text = info_text(&sample());
        assert!(text.contains("64×64"));
        assert!(text.contains("1:1"));
        assert!(text.contains("PNG:"));
        assert!(text.contains("JPEG:"));
        assert!(text.contains("Pixels: 4096"));
    }

    #[test]
    fn jpeg_roundtrips() {
        let image = sample();
        let jpeg = jpeg_bytes(&image, 80).expect("jpeg");
        assert!(jpeg.starts_with(&[0xFF, 0xD8]));
        let decoded = image_from_encoded(&jpeg).expect("decode");
        assert_eq!(decoded.width, 64);
        assert_eq!(decoded.height, 64);
    }

    #[test]
    fn noisy_image_jpeg_is_smaller_than_png() {
        let jpeg = try_jpeg(&sample());
        assert!(jpeg.is_some());
    }

    #[test]
    fn format_bytes_uses_units() {
        assert_eq!(format_bytes(12), "12 B");
        assert_eq!(format_bytes(1500), "1.5 KB");
        assert_eq!(format_bytes(2_000_000), "2.0 MB");
        assert_eq!(aspect_ratio(1920, 1080), "16:9");
    }
}
