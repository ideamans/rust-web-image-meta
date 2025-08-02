# web-image-meta

A lightweight Rust library for manipulating JPEG and PNG metadata, optimized for web images.

## Features

- **JPEG Processing**
  - Remove all EXIF metadata except orientation information
  - Preserve ICC color profiles
  - Read and write JPEG comments
  - Clean unnecessary metadata while maintaining image quality

- **PNG Processing**
  - Remove non-critical chunks to reduce file size
  - Read and write tEXt chunks
  - Preserve essential chunks (IHDR, PLTE, IDAT, IEND, tRNS, gAMA, etc.)

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
web-image-meta = "0.1.0"
```

## Usage

### JPEG Operations

```rust
use web_image_meta::jpeg;

// Clean JPEG metadata (removes all EXIF except orientation)
let image_data = std::fs::read("photo.jpg")?;
let cleaned_data = jpeg::clean_metadata(&image_data)?;
std::fs::write("photo_cleaned.jpg", cleaned_data)?;

// Read JPEG comment
let comment = jpeg::read_comment(&image_data)?;
println!("Comment: {:?}", comment);

// Write JPEG comment
let data_with_comment = jpeg::write_comment(&image_data, "My photo comment")?;
std::fs::write("photo_with_comment.jpg", data_with_comment)?;
```

### PNG Operations

```rust
use web_image_meta::png;

// Clean PNG chunks (removes non-critical chunks)
let image_data = std::fs::read("image.png")?;
let cleaned_data = png::clean_chunks(&image_data)?;
std::fs::write("image_cleaned.png", cleaned_data)?;

// Read PNG text chunks
let text_chunks = png::read_text_chunks(&image_data)?;
for chunk in text_chunks {
    println!("{}: {}", chunk.keyword, chunk.text);
}

// Add PNG text chunk
let data_with_text = png::add_text_chunk(&image_data, "Author", "John Doe")?;
std::fs::write("image_with_author.png", data_with_text)?;
```

## API Documentation

### JPEG Module

- `clean_metadata(data: &[u8]) -> Result<Vec<u8>, Error>`
  - Removes all metadata except EXIF orientation and ICC profiles
  
- `read_comment(data: &[u8]) -> Result<Option<String>, Error>`
  - Reads the JPEG comment if present
  
- `write_comment(data: &[u8], comment: &str) -> Result<Vec<u8>, Error>`
  - Writes or replaces the JPEG comment

### PNG Module

- `clean_chunks(data: &[u8]) -> Result<Vec<u8>, Error>`
  - Removes all non-critical chunks
  
- `read_text_chunks(data: &[u8]) -> Result<Vec<TextChunk>, Error>`
  - Reads all tEXt chunks from the PNG
  
- `add_text_chunk(data: &[u8], keyword: &str, text: &str) -> Result<Vec<u8>, Error>`
  - Adds a new tEXt chunk to the PNG

### Error Handling

The library uses a custom `Error` enum with the following variants:
- `InvalidFormat(String)` - Input data is not a valid image format
- `Io(std::io::Error)` - I/O operation failed
- `ParseError(String)` - Failed to parse image structure

## Safety and Validation

- All operations validate that input data is a valid JPEG or PNG file
- Output is verified to be decodable before returning
- Malformed or corrupted images are rejected with appropriate errors

## License

This project is licensed under the MIT License - see the [LICENSE-MIT](LICENSE-MIT) file for details.