//! Image decoding helpers for docx packages.

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
}

impl ImageFormat {
    pub(crate) fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Gif => "gif",
        }
    }

    pub(crate) fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub(crate) data: Vec<u8>,
    pub(crate) format: ImageFormat,
    pub(crate) width_mm: f32,
    pub(crate) height_mm: f32,
    pub(crate) alt: String,
}

impl Image {
    /// Reads PNG/JPEG/GIF dimensions from the file header. Pixel dimensions are
    /// converted at 96 DPI for the initial display size.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let (format, width_px, height_px) = detect(&data)?;
        if width_px == 0 || height_px == 0 {
            return Err(Error::Invalid("图片尺寸不能为 0".to_owned()));
        }
        Ok(Self {
            data,
            format,
            width_mm: width_px as f32 * 25.4 / 96.0,
            height_mm: height_px as f32 * 25.4 / 96.0,
            alt: String::new(),
        })
    }

    pub fn with_alt(mut self, alt: impl Into<String>) -> Self {
        self.alt = alt.into();
        self
    }

    /// Scales the image down to fit the box. Never upscales.
    pub fn fit_within(mut self, max_width_mm: f32, max_height_mm: f32) -> Self {
        if max_width_mm > 0.0 && max_height_mm > 0.0 {
            let scale = (max_width_mm / self.width_mm)
                .min(max_height_mm / self.height_mm)
                .min(1.0);
            self.width_mm *= scale;
            self.height_mm *= scale;
        }
        self
    }

    pub fn width_mm(&self) -> f32 {
        self.width_mm
    }

    pub fn height_mm(&self) -> f32 {
        self.height_mm
    }

    pub fn format(&self) -> ImageFormat {
        self.format
    }

    pub(crate) fn width_emu(&self) -> i64 {
        (self.width_mm as f64 * 36000.0).round() as i64
    }

    pub(crate) fn height_emu(&self) -> i64 {
        (self.height_mm as f64 * 36000.0).round() as i64
    }
}

fn detect(data: &[u8]) -> Result<(ImageFormat, u32, u32)> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return png_size(data);
    }
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        return gif_size(data);
    }
    if data.starts_with(b"\xff\xd8") {
        return jpeg_size(data);
    }
    Err(Error::Invalid(
        "无法识别图片格式；当前支持 PNG、JPEG、GIF".to_owned(),
    ))
}

fn png_size(data: &[u8]) -> Result<(ImageFormat, u32, u32)> {
    if data.len() < 24 || &data[12..16] != b"IHDR" {
        return Err(Error::Invalid("PNG 头不完整".to_owned()));
    }
    let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    Ok((ImageFormat::Png, width, height))
}

fn gif_size(data: &[u8]) -> Result<(ImageFormat, u32, u32)> {
    if data.len() < 10 {
        return Err(Error::Invalid("GIF 头不完整".to_owned()));
    }
    let width = u16::from_le_bytes([data[6], data[7]]) as u32;
    let height = u16::from_le_bytes([data[8], data[9]]) as u32;
    Ok((ImageFormat::Gif, width, height))
}

fn jpeg_size(data: &[u8]) -> Result<(ImageFormat, u32, u32)> {
    let mut offset = 2usize;
    while offset + 3 < data.len() {
        if data[offset] != 0xFF {
            offset += 1;
            continue;
        }
        let marker = data[offset + 1];
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) {
            offset += 2;
            continue;
        }
        let length = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
        if length < 2 || offset + 2 + length > data.len() {
            break;
        }
        let is_sof = matches!(
            marker,
            0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF
        );
        if is_sof {
            if offset + 9 > data.len() {
                break;
            }
            let height = u16::from_be_bytes([data[offset + 5], data[offset + 6]]) as u32;
            let width = u16::from_be_bytes([data[offset + 7], data[offset + 8]]) as u32;
            return Ok((ImageFormat::Jpeg, width, height));
        }
        offset += 2 + length;
    }
    Err(Error::Invalid("JPEG 中未找到尺寸信息".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_png_dimensions() {
        let data = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x20,
        ];
        let image = Image::from_bytes(data.to_vec()).unwrap();
        assert_eq!(image.format(), ImageFormat::Png);
        assert!((image.width_mm() - 16.0 * 25.4 / 96.0).abs() < 0.01);
        assert!((image.height_mm() - 32.0 * 25.4 / 96.0).abs() < 0.01);
    }

    #[test]
    fn reads_gif_dimensions() {
        let data = [b'G', b'I', b'F', b'8', b'9', b'a', 0x64, 0x00, 0x32, 0x00];
        let image = Image::from_bytes(data.to_vec()).unwrap();
        assert_eq!(image.format(), ImageFormat::Gif);
        assert!((image.width_mm() - 100.0 * 25.4 / 96.0).abs() < 0.01);
        assert!((image.height_mm() - 50.0 * 25.4 / 96.0).abs() < 0.01);
    }

    #[test]
    fn reads_jpeg_dimensions() {
        let mut data = vec![
            0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x64, 0x00, 0xC8,
        ];
        data.resize(24, 0);
        let image = Image::from_bytes(data).unwrap();
        assert_eq!(image.format(), ImageFormat::Jpeg);
        assert!((image.width_mm() - 200.0 * 25.4 / 96.0).abs() < 0.01);
        assert!((image.height_mm() - 100.0 * 25.4 / 96.0).abs() < 0.01);
    }

    #[test]
    fn rejects_unknown_format() {
        assert!(Image::from_bytes(b"not an image".to_vec()).is_err());
    }

    #[test]
    fn fit_within_never_upscales() {
        let mut data = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D,
        ];
        data.extend_from_slice(b"IHDR");
        data.extend_from_slice(&10u32.to_be_bytes());
        data.extend_from_slice(&10u32.to_be_bytes());
        let image = Image::from_bytes(data).unwrap().fit_within(100.0, 100.0);
        assert!((image.width_mm() - 10.0 * 25.4 / 96.0).abs() < 0.01);
    }
}
