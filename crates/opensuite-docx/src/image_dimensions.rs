use std::fmt;

/// A supported image encoding detected from its bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageFormat {
    Png,
    Jpeg,
}

/// Intrinsic raster image dimensions in pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageDimensions {
    pub width_px: u32,
    pub height_px: u32,
}

/// The detected image format and its intrinsic dimensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageInfo {
    pub format: ImageFormat,
    pub dimensions: ImageDimensions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageDimensionError {
    UnsupportedFormat,
    InvalidPng,
    InvalidJpeg,
}

impl fmt::Display for ImageDimensionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::UnsupportedFormat => "unsupported image format",
            Self::InvalidPng => "invalid PNG dimensions",
            Self::InvalidJpeg => "invalid JPEG dimensions",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ImageDimensionError {}

/// Detects a supported PNG or JPEG image and reads its intrinsic dimensions.
pub fn read_image_info(bytes: &[u8]) -> Result<ImageInfo, ImageDimensionError> {
    if bytes.starts_with(PNG_SIGNATURE) {
        return read_png_dimensions(bytes).map(|dimensions| ImageInfo {
            format: ImageFormat::Png,
            dimensions,
        });
    }
    if bytes.starts_with(&[0xff, 0xd8]) {
        return read_jpeg_dimensions(bytes).map(|dimensions| ImageInfo {
            format: ImageFormat::Jpeg,
            dimensions,
        });
    }
    Err(ImageDimensionError::UnsupportedFormat)
}

/// Reads PNG dimensions from its required IHDR chunk.
pub fn read_png_dimensions(bytes: &[u8]) -> Result<ImageDimensions, ImageDimensionError> {
    if !bytes.starts_with(PNG_SIGNATURE)
        || bytes.len() < 33
        || bytes[8..12] != [0, 0, 0, 13]
        || bytes[12..16] != *b"IHDR"
    {
        return Err(ImageDimensionError::InvalidPng);
    }
    dimensions(
        u32_at(bytes, 16),
        u32_at(bytes, 20),
        ImageDimensionError::InvalidPng,
    )
}

/// Reads JPEG dimensions from the first supported Start Of Frame marker.
pub fn read_jpeg_dimensions(bytes: &[u8]) -> Result<ImageDimensions, ImageDimensionError> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return Err(ImageDimensionError::InvalidJpeg);
    }

    let mut index = 2;
    while index < bytes.len() {
        if bytes[index] != 0xff {
            return Err(ImageDimensionError::InvalidJpeg);
        }
        while bytes.get(index) == Some(&0xff) {
            index += 1;
        }
        let Some(&marker) = bytes.get(index) else {
            return Err(ImageDimensionError::InvalidJpeg);
        };
        index += 1;

        if matches!(marker, 0x01 | 0xd8 | 0xd9 | 0xd0..=0xd7) {
            continue;
        }
        if marker == 0x00 || marker == 0xda {
            return Err(ImageDimensionError::InvalidJpeg);
        }
        let Some(length_bytes) = bytes.get(index..index + 2) else {
            return Err(ImageDimensionError::InvalidJpeg);
        };
        let length = u16::from_be_bytes([length_bytes[0], length_bytes[1]]) as usize;
        if length < 2 || index + length > bytes.len() {
            return Err(ImageDimensionError::InvalidJpeg);
        }
        if is_start_of_frame(marker) {
            if length < 8 || length != 8 + 3 * bytes[index + 7] as usize {
                return Err(ImageDimensionError::InvalidJpeg);
            }
            return dimensions(
                u16_at(bytes, index + 5) as u32,
                u16_at(bytes, index + 3) as u32,
                ImageDimensionError::InvalidJpeg,
            );
        }
        index += length;
    }
    Err(ImageDimensionError::InvalidJpeg)
}

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

fn dimensions(
    width_px: u32,
    height_px: u32,
    error: ImageDimensionError,
) -> Result<ImageDimensions, ImageDimensionError> {
    if width_px == 0 || height_px == 0 {
        return Err(error);
    }
    Ok(ImageDimensions {
        width_px,
        height_px,
    })
}

fn is_start_of_frame(marker: u8) -> bool {
    matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf)
}

fn u16_at(bytes: &[u8], index: usize) -> u16 {
    u16::from_be_bytes([bytes[index], bytes[index + 1]])
}

fn u32_at(bytes: &[u8], index: usize) -> u32 {
    u32::from_be_bytes([
        bytes[index],
        bytes[index + 1],
        bytes[index + 2],
        bytes[index + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width_px: u32, height_px: u32) -> Vec<u8> {
        let mut bytes = PNG_SIGNATURE.to_vec();
        bytes.extend_from_slice(&[0, 0, 0, 13]);
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width_px.to_be_bytes());
        bytes.extend_from_slice(&height_px.to_be_bytes());
        bytes.extend_from_slice(&[8, 2, 0, 0, 0]);
        bytes.extend_from_slice(&[0; 4]);
        bytes
    }

    fn segment(marker: u8, data: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0xff, marker];
        bytes.extend_from_slice(&((data.len() + 2) as u16).to_be_bytes());
        bytes.extend_from_slice(data);
        bytes
    }

    #[test]
    fn reads_png_dimensions_and_detects_format() {
        let info = read_image_info(&png(640, 480)).unwrap();
        assert_eq!(info.format, ImageFormat::Png);
        assert_eq!(
            info.dimensions,
            ImageDimensions {
                width_px: 640,
                height_px: 480
            }
        );
    }

    #[test]
    fn rejects_truncated_or_zero_sized_pngs() {
        assert_eq!(
            read_png_dimensions(PNG_SIGNATURE),
            Err(ImageDimensionError::InvalidPng)
        );
        assert_eq!(
            read_png_dimensions(&png(0, 1)),
            Err(ImageDimensionError::InvalidPng)
        );
    }

    #[test]
    fn reads_jpeg_dimensions_after_metadata_segments() {
        let mut bytes = vec![0xff, 0xd8];
        bytes.extend(segment(0xe0, b"JFIF\0"));
        bytes.extend(segment(0xfe, b"metadata"));
        bytes.extend(segment(
            0xc0,
            &[8, 1, 224, 2, 128, 3, 1, 17, 0, 2, 17, 1, 3, 17, 1],
        ));

        assert_eq!(
            read_jpeg_dimensions(&bytes),
            Ok(ImageDimensions {
                width_px: 640,
                height_px: 480
            })
        );
        assert_eq!(read_image_info(&bytes).unwrap().format, ImageFormat::Jpeg);
    }

    #[test]
    fn rejects_malformed_or_zero_sized_jpegs() {
        assert_eq!(
            read_jpeg_dimensions(&[0xff, 0xd8, 0xff, 0xe0]),
            Err(ImageDimensionError::InvalidJpeg)
        );
        let mut zero_width = vec![0xff, 0xd8];
        zero_width.extend(segment(
            0xc0,
            &[8, 0, 1, 0, 0, 3, 1, 17, 0, 2, 17, 1, 3, 17, 1],
        ));
        assert_eq!(
            read_jpeg_dimensions(&zero_width),
            Err(ImageDimensionError::InvalidJpeg)
        );
    }

    #[test]
    fn rejects_unsupported_bytes() {
        assert_eq!(
            read_image_info(b"not an image"),
            Err(ImageDimensionError::UnsupportedFormat)
        );
    }
}
