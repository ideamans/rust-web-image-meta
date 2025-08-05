# web-image-meta

[![Crates.io](https://img.shields.io/crates/v/web-image-meta.svg)](https://crates.io/crates/web-image-meta)
[![Documentation](https://docs.rs/web-image-meta/badge.svg)](https://docs.rs/web-image-meta)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![CI](https://github.com/ideamans/rust-web-image-meta/workflows/CI/badge.svg)](https://github.com/ideamans/rust-web-image-meta/actions)

A lightweight Rust library for manipulating JPEG and PNG metadata, optimized for web images.

## Features

- **JPEG Support**
  - Clean metadata while preserving orientation information
  - Read and write JPEG comments
  - Preserve ICC profiles
  - Remove EXIF, XMP, IPTC and other metadata
  
- **PNG Support**
  - Remove non-critical chunks
  - Read and write text chunks (tEXt, zTXt, iTXt)
  - Preserve transparency and color information
  - Automatic decompression of compressed text chunks

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
web-image-meta = "0.2.0"
```

## Usage

### JPEG Examples

```rust
use web_image_meta::jpeg;

// Clean JPEG metadata while preserving orientation
let input_data = std::fs::read("input.jpg")?;
let cleaned_data = jpeg::clean_metadata(&input_data)?;
std::fs::write("cleaned.jpg", cleaned_data)?;

// Read JPEG comment
let comment = jpeg::read_comment(&input_data)?;
if let Some(text) = comment {
    println!("Comment: {}", text);
}

// Write JPEG comment
let data_with_comment = jpeg::write_comment(&input_data, "Copyright 2024")?;
std::fs::write("commented.jpg", data_with_comment)?;
```

### PNG Examples

```rust
use web_image_meta::png;

// Remove non-critical chunks from PNG
let input_data = std::fs::read("input.png")?;
let cleaned_data = png::clean_chunks(&input_data)?;
std::fs::write("cleaned.png", cleaned_data)?;

// Read PNG text chunks (supports tEXt, zTXt, iTXt)
let chunks = png::read_text_chunks(&input_data)?;
for chunk in chunks {
    println!("{}: {}", chunk.keyword, chunk.text);
}
// zTXt (compressed) and iTXt (international) chunks are automatically handled

// Add text chunk to PNG
let data_with_text = png::add_text_chunk(
    &input_data,
    "Copyright",
    "© 2024 Example Corp"
)?;
std::fs::write("tagged.png", data_with_text)?;
```

## API Reference

### JPEG Functions

#### `clean_metadata(data: &[u8]) -> Result<Vec<u8>, Error>`
Removes all metadata except EXIF orientation information.

- Preserves: JFIF, ICC profiles, essential JPEG markers, EXIF orientation (tag 0x0112)
- Removes: All other EXIF data, XMP, IPTC, comments, APP markers (except APP0, APP1 with orientation, APP2 with ICC)
- Returns: Cleaned JPEG data

#### `read_comment(data: &[u8]) -> Result<Option<String>, Error>`
Reads the COM (comment) segment from a JPEG file.

- Returns: `Some(String)` if a comment exists, `None` otherwise
- Encoding: UTF-8 (lossy conversion for non-UTF-8 data)

#### `write_comment(data: &[u8], comment: &str) -> Result<Vec<u8>, Error>`
Writes or replaces a comment in a JPEG file.

- Replaces any existing comment
- Places comment before SOS marker
- Maximum length: 65,533 bytes

### PNG Functions

#### `clean_chunks(data: &[u8]) -> Result<Vec<u8>, Error>`
Removes all non-critical chunks from a PNG file.

- Preserves: IHDR, PLTE, IDAT, IEND, tRNS, gAMA, cHRM, sRGB, iCCP, sBIT, pHYs
- Removes: tEXt, zTXt, iTXt, tIME, bKGD, and all other ancillary chunks
- Returns: Cleaned PNG data

#### `read_text_chunks(data: &[u8]) -> Result<Vec<TextChunk>, Error>`
Reads all text chunks from a PNG file.

- Returns: Vector of `TextChunk` structs
- Supports: tEXt (uncompressed), zTXt (compressed), iTXt (international)
- Automatically decompresses zTXt chunks
- Handles UTF-8 text in iTXt chunks

#### `add_text_chunk(data: &[u8], keyword: &str, text: &str) -> Result<Vec<u8>, Error>`
Adds a new tEXt chunk to a PNG file.

- Keyword: 1-79 Latin characters (letters, numbers, spaces)
- Text: UTF-8 string of any length
- Places new chunk before IEND

### Types

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChunk {
    pub keyword: String,  // 1-79 character Latin keyword
    pub text: String,     // Text content
}

#[derive(Debug)]
pub enum Error {
    InvalidFormat(String),  // Invalid image format
    Io(std::io::Error),    // I/O error
    ParseError(String),    // Parsing error
}
```

## What Gets Preserved

### JPEG
- Essential image data and structure
- EXIF Orientation (tag 0x0112) when present
- ICC color profiles (APP2)
- JFIF markers (APP0)
- All SOF markers (image encoding parameters)
- Huffman tables (DHT)
- Quantization tables (DQT)

### PNG
- Critical chunks: IHDR, PLTE, IDAT, IEND
- Transparency: tRNS
- Color space: gAMA, cHRM, sRGB, iCCP, sBIT
- Physical dimensions: pHYs

## What Gets Removed

### JPEG
- EXIF data (except orientation)
- XMP metadata
- IPTC data
- Comments (when using clean_metadata)
- Photoshop resources (APP13)
- Other APP markers (APP3-APP15, except APP2 with ICC)

### PNG
- Text chunks: tEXt, zTXt, iTXt
- Time chunks: tIME
- Background: bKGD
- Histogram: hIST
- Suggested palette: sPLT
- Other ancillary chunks

## Error Handling

The library provides detailed error types:
- `InvalidFormat`: The input is not a valid JPEG/PNG file
- `ParseError`: The file structure is corrupted or invalid
- `Io`: System I/O errors

All functions validate their outputs to ensure the resulting images can be decoded.

## Performance

This library is designed for web image optimization:
- Fast metadata stripping for reducing file sizes
- Preserves only essential information needed for proper display
- Memory-efficient processing
- Validates output to ensure images remain viewable

## Safety

The library validates all inputs and outputs:
- Checks for valid JPEG/PNG signatures
- Validates chunk structures and CRCs (PNG)
- Ensures output images can be decoded
- Safe handling of malformed images

## Test Coverage

The library includes comprehensive tests:
- 53 test cases covering various scenarios
- Tests for different image formats, color spaces, and edge cases
- Validation of output images using decoder libraries
- Tests run on Linux, macOS, and Windows

## License

This project is licensed under the MIT License - see the [LICENSE-MIT](LICENSE-MIT) file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Acknowledgments

This library uses the following excellent crates:
- [jpeg-decoder](https://crates.io/crates/jpeg-decoder) for JPEG validation
- [png](https://crates.io/crates/png) for PNG validation
- [crc32fast](https://crates.io/crates/crc32fast) for CRC calculation
- [flate2](https://crates.io/crates/flate2) for zTXt/iTXt decompression