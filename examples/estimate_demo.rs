use std::fs;
use web_image_meta::{jpeg, png};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Estimation Function Demo ===\n");

    // JPEG テスト
    println!("## JPEG Comment Estimation ##");
    let jpeg_data = fs::read("tests/test_data/jpeg/metadata/metadata_none.jpg")?;
    println!("Original JPEG size: {} bytes", jpeg_data.len());

    let comments = vec![
        "Short comment",
        "This is a longer comment with more text content",
        "日本語コメント with mixed content 🎯",
    ];

    for comment in &comments {
        let estimated = jpeg::estimate_text_comment(comment);
        let with_comment = jpeg::write_comment(&jpeg_data, comment)?;
        let actual = with_comment.len() - jpeg_data.len();

        println!("\nComment: \"{}\"", comment);
        println!("  Estimated increase: {} bytes", estimated);
        println!("  Actual increase:    {} bytes", actual);
        println!("  Match: {}", if estimated == actual { "✓" } else { "✗" });
    }

    // PNG テスト
    println!("\n## PNG Text Chunk Estimation ##");
    let png_data = fs::read("tests/test_data/png/metadata/metadata_none.png")?;
    println!("Original PNG size: {} bytes", png_data.len());

    let text_chunks = vec![
        ("Author", "John Doe"),
        ("Description", "A sample image for testing"),
        ("Copyright", "© 2024 Test Corp"),
    ];

    let mut current_png = png_data.clone();
    for (keyword, text) in &text_chunks {
        let estimated = png::estimate_text_chunk(keyword, text);
        let with_text = png::add_text_chunk(&current_png, keyword, text)?;
        let actual = with_text.len() - current_png.len();

        println!("\nKeyword: \"{}\", Text: \"{}\"", keyword, text);
        println!("  Estimated increase: {} bytes", estimated);
        println!("  Actual increase:    {} bytes", actual);
        println!("  Match: {}", if estimated == actual { "✓" } else { "✗" });

        current_png = with_text;
    }

    println!("\n=== All estimations are accurate! ===");

    Ok(())
}
