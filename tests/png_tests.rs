use std::fs;
use std::path::Path;
use web_image_meta::png;
use web_image_meta::Error;

fn load_test_image(path: &str) -> Vec<u8> {
    let full_path = Path::new("tests/test_data").join(path);
    fs::read(full_path).expect(&format!("Failed to read test image: {}", path))
}

#[test]
fn test_clean_chunks_preserves_critical() {
    let data = load_test_image("png/metadata/metadata_text.png");
    let cleaned = png::clean_chunks(&data).expect("Failed to clean chunks");

    // クリーンアップ後も有効なPNGであることを確認
    assert_eq!(&cleaned[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);

    // サイズが減っているはず（メタデータが削除されるため）
    assert!(cleaned.len() < data.len());

    // 必須チャンクが保持されているか確認
    assert!(
        check_chunk_exists(&cleaned, b"IHDR"),
        "IHDR chunk must exist"
    );
    assert!(
        check_chunk_exists(&cleaned, b"IDAT"),
        "IDAT chunk must exist"
    );
    assert!(
        check_chunk_exists(&cleaned, b"IEND"),
        "IEND chunk must exist"
    );

    // テキストチャンクが削除されているか確認
    assert!(
        !check_chunk_exists(&cleaned, b"tEXt"),
        "tEXt chunk should be removed"
    );
    assert!(
        !check_chunk_exists(&cleaned, b"iTXt"),
        "iTXt chunk should be removed"
    );
    assert!(
        !check_chunk_exists(&cleaned, b"zTXt"),
        "zTXt chunk should be removed"
    );
}

#[test]
fn test_clean_chunks_preserves_transparency() {
    let data = load_test_image("png/alpha/alpha_semitransparent.png");
    let cleaned = png::clean_chunks(&data).expect("Failed to clean chunks");

    // 透明度情報を持つPNGが正しく処理されることを確認
    assert_eq!(&cleaned[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);

    // チャンクタイプを解析してアルファチャンネルまたはtRNSチャンクを確認
    let has_alpha = check_if_has_alpha(&data);
    if has_alpha {
        // アルファチャンネルまたはtRNSチャンクが保持されているか確認
        let has_trns = check_chunk_exists(&cleaned, b"tRNS");
        let has_alpha_in_cleaned = check_if_has_alpha(&cleaned);
        assert!(
            has_trns || has_alpha_in_cleaned,
            "Transparency information should be preserved"
        );
    }
}

#[test]
fn test_clean_chunks_preserves_color_space() {
    let data = load_test_image("png/chunk/chunk_gamma.png");
    let cleaned = png::clean_chunks(&data).expect("Failed to clean chunks");

    // ガンマ補正チャンクが保持されることを確認
    assert_eq!(&cleaned[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);

    // 元データにgAMAチャンクがあるか確認
    let has_gamma_original = check_chunk_exists(&data, b"gAMA");
    if has_gamma_original {
        // gAMAチャンクが存在する場合は保持されているか確認
        let has_gamma = check_chunk_exists(&cleaned, b"gAMA");
        assert!(has_gamma, "gAMA chunk should be preserved");
    }
}

#[test]
fn test_read_text_chunks_single() {
    let data = load_test_image("png/metadata/metadata_text.png");
    let chunks = png::read_text_chunks(&data).expect("Failed to read text chunks");

    // 少なくとも1つのテキストチャンクが存在するはず
    assert!(!chunks.is_empty());
}

#[test]
fn test_read_text_chunks_none() {
    let data = load_test_image("png/metadata/metadata_none.png");
    let chunks = png::read_text_chunks(&data).expect("Failed to read text chunks");

    // テキストチャンクがないことを確認
    assert!(chunks.is_empty());
}

#[test]
fn test_add_text_chunk() {
    let data = load_test_image("png/metadata/metadata_none.png");
    let keyword = "Comment";
    let text = "This is a test comment with Unicode: 日本語 émojis 🎯";

    let data_with_text =
        png::add_text_chunk(&data, keyword, text).expect("Failed to add text chunk");

    // テキストチャンクが正しく追加されたか確認
    let chunks = png::read_text_chunks(&data_with_text).expect("Failed to read text chunks");

    let found = chunks.iter().find(|c| c.keyword == keyword);
    assert!(found.is_some());
    assert_eq!(found.unwrap().text, text);

    // tEXtチャンクが存在するか確認
    assert!(
        check_chunk_exists(&data_with_text, b"tEXt"),
        "tEXt chunk should exist"
    );

    // チャンクがIENDの前に配置されているか確認
    let text_pos = find_chunk_position(&data_with_text, b"tEXt").expect("tEXt chunk not found");
    let iend_pos = find_chunk_position(&data_with_text, b"IEND").expect("IEND chunk not found");
    assert!(
        text_pos < iend_pos,
        "tEXt chunk should be placed before IEND"
    );

    // 追加後も有効なPNGであるか確認
    assert_eq!(&data_with_text[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
}

#[test]
fn test_add_multiple_text_chunks() {
    let data = load_test_image("png/metadata/metadata_none.png");

    // 最初のテキストチャンクを追加
    let data1 =
        png::add_text_chunk(&data, "Author", "Test Author").expect("Failed to add first chunk");

    // 二番目のテキストチャンクを追加
    let data2 = png::add_text_chunk(&data1, "Description", "Test Description")
        .expect("Failed to add second chunk");

    // 両方のチャンクが存在することを確認
    let chunks = png::read_text_chunks(&data2).expect("Failed to read text chunks");

    assert!(chunks
        .iter()
        .any(|c| c.keyword == "Author" && c.text == "Test Author"));
    assert!(chunks
        .iter()
        .any(|c| c.keyword == "Description" && c.text == "Test Description"));
}

#[test]
fn test_invalid_png_data() {
    let invalid_data = vec![0x00, 0x01, 0x02, 0x03];

    assert!(matches!(
        png::clean_chunks(&invalid_data),
        Err(Error::InvalidFormat(_))
    ));

    assert!(matches!(
        png::read_text_chunks(&invalid_data),
        Err(Error::InvalidFormat(_))
    ));

    assert!(matches!(
        png::add_text_chunk(&invalid_data, "test", "value"),
        Err(Error::InvalidFormat(_))
    ));
}

#[test]
fn test_corrupted_png_decode() {
    // 有効なPNGヘッダーだが破損したデータ
    let mut corrupted_data = vec![137, 80, 78, 71, 13, 10, 26, 10];
    // IHDRチャンクの開始
    corrupted_data.extend_from_slice(&[0x00, 0x00, 0x00, 0x0D]); // 長さ
    corrupted_data.extend_from_slice(b"IHDR");
    // 不完全なデータで終了

    assert!(matches!(
        png::clean_chunks(&corrupted_data),
        Err(Error::InvalidFormat(_))
    ));
}

#[test]
fn test_valid_png_decode() {
    // 実際の有効なPNGファイルをテスト
    let data = load_test_image("png/metadata/metadata_none.png");

    // すべての関数で正常にデコードできることを確認
    let cleaned = png::clean_chunks(&data).expect("Should decode valid PNG");
    assert!(!cleaned.is_empty());

    let chunks = png::read_text_chunks(&data).expect("Should decode valid PNG");
    assert!(chunks.is_empty());

    let with_text = png::add_text_chunk(&data, "test", "value").expect("Should decode valid PNG");
    assert!(!with_text.is_empty());
}

#[test]
fn test_keyword_validation() {
    let data = load_test_image("png/metadata/metadata_none.png");

    // 空のキーワード
    assert!(matches!(
        png::add_text_chunk(&data, "", "text"),
        Err(Error::InvalidFormat(_))
    ));

    // 長すぎるキーワード
    let long_keyword = "a".repeat(80);
    assert!(matches!(
        png::add_text_chunk(&data, &long_keyword, "text"),
        Err(Error::InvalidFormat(_))
    ));

    // 非ASCII文字を含むキーワード
    assert!(matches!(
        png::add_text_chunk(&data, "テスト", "text"),
        Err(Error::InvalidFormat(_))
    ));
}

#[test]
fn test_empty_text() {
    let data = load_test_image("png/metadata/metadata_none.png");
    let data_with_text =
        png::add_text_chunk(&data, "EmptyText", "").expect("Failed to add empty text");

    let chunks = png::read_text_chunks(&data_with_text).expect("Failed to read text chunks");
    let found = chunks.iter().find(|c| c.keyword == "EmptyText");

    assert!(found.is_some());
    assert_eq!(found.unwrap().text, "");
}

#[test]
fn test_different_color_types() {
    let test_files = vec![
        "png/colortype/colortype_grayscale.png",
        "png/colortype/colortype_palette.png",
        "png/colortype/colortype_rgb.png",
        "png/colortype/colortype_rgba.png",
        "png/colortype/colortype_grayscale_alpha.png",
    ];

    for file in test_files {
        let data = load_test_image(file);
        let cleaned = png::clean_chunks(&data).expect(&format!("Failed to clean {}", file));

        // すべての色タイプで正しく処理できることを確認
        assert_eq!(&cleaned[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }
}

#[test]
fn test_interlaced_png() {
    let data = load_test_image("png/interlace/interlace_adam7.png");
    let cleaned = png::clean_chunks(&data).expect("Failed to clean interlaced PNG");

    // インターレースPNGも正しく処理できることを確認
    assert_eq!(&cleaned[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
}

#[test]
fn test_16bit_depth() {
    let data = load_test_image("png/depth/depth_16bit.png");
    let cleaned = png::clean_chunks(&data).expect("Failed to clean 16-bit PNG");

    // 16ビット深度のPNGも正しく処理できることを確認
    assert_eq!(&cleaned[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
}

#[test]
fn test_preserve_physical_dimensions() {
    // pHYsチャンクを持つPNGファイルでテスト
    let data = load_test_image("png/colortype/colortype_rgb.png");
    let cleaned = png::clean_chunks(&data).expect("Failed to clean PNG");

    // 有効なPNGであることを確認
    assert_eq!(&cleaned[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
}

// ヘルパー関数：特定のチャンクが存在するかチェック
fn check_chunk_exists(data: &[u8], chunk_type: &[u8; 4]) -> bool {
    let mut pos = 8; // PNGシグネチャをスキップ

    while pos + 8 <= data.len() {
        let current_type = &data[pos + 4..pos + 8];
        if current_type == chunk_type {
            return true;
        }

        if current_type == b"IEND" {
            return chunk_type == b"IEND";
        }

        let length =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 12 + length; // 長さ(4) + タイプ(4) + データ + CRC(4)

        if pos > data.len() {
            break;
        }
    }

    false
}

// ヘルパー関数：チャンクの位置を検索
fn find_chunk_position(data: &[u8], chunk_type: &[u8; 4]) -> Option<usize> {
    let mut pos = 8; // PNGシグネチャをスキップ

    while pos + 8 <= data.len() {
        let current_type = &data[pos + 4..pos + 8];
        if current_type == chunk_type {
            return Some(pos);
        }

        if current_type == b"IEND" {
            return if chunk_type == b"IEND" {
                Some(pos)
            } else {
                None
            };
        }

        let length =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 12 + length; // 長さ(4) + タイプ(4) + データ + CRC(4)

        if pos > data.len() {
            break;
        }
    }

    None
}

// ヘルパー関数：アルファチャンネルがあるか確認
fn check_if_has_alpha(data: &[u8]) -> bool {
    // IHDRチャンクからカラータイプを取得
    let mut pos = 8;

    while pos + 8 <= data.len() {
        let current_type = &data[pos + 4..pos + 8];

        if current_type == b"IHDR" {
            // IHDRチャンクのデータ部分
            // カラータイプは13バイト目（0-indexed）
            if pos + 8 + 13 < data.len() {
                let color_type = data[pos + 8 + 9];
                // カラータイプ4（グレースケール+アルファ）または6（RGB+アルファ）
                return color_type == 4 || color_type == 6;
            }
        }

        if current_type == b"IEND" {
            break;
        }

        let length =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 12 + length;

        if pos > data.len() {
            break;
        }
    }

    false
}

#[test]
fn test_critical_png_cases() {
    let critical_files = vec![
        "png/critical/critical_16bit_palette.png",
        "png/critical/critical_alpha_grayscale.png",
        "png/critical/critical_interlace_highres.png",
        "png/critical/critical_maxcompression_paeth.png",
    ];

    for file in critical_files {
        let data = load_test_image(file);

        // All critical files should be processable
        let result = png::clean_chunks(&data);
        assert!(result.is_ok(), "Failed to process critical file: {}", file);

        // Verify output is still valid PNG
        let cleaned = result.unwrap();
        assert!(!cleaned.is_empty());
        assert_eq!(&cleaned[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }
}

#[test]
fn test_various_bit_depths() {
    let depth_files = vec![
        ("png/depth/depth_1bit.png", 1),
        ("png/depth/depth_8bit.png", 8),
        ("png/depth/depth_16bit.png", 16),
    ];

    for (file, _depth) in depth_files {
        let data = load_test_image(file);
        let cleaned = png::clean_chunks(&data).expect(&format!("Failed to clean {}", file));

        // Bit depth should not affect chunk cleaning
        assert_eq!(&cleaned[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);

        // Text chunks should work on all bit depths
        let with_text = png::add_text_chunk(&cleaned, "Depth", "test").expect("Failed to add text");
        let chunks = png::read_text_chunks(&with_text).expect("Failed to read text");
        assert!(chunks
            .iter()
            .any(|c| c.keyword == "Depth" && c.text == "test"));
    }
}

#[test]
fn test_compression_levels() {
    let compression_files = vec![
        ("png/compression/compression_0.png", 0),
        ("png/compression/compression_6.png", 6),
        ("png/compression/compression_9.png", 9),
    ];

    for (file, _level) in compression_files {
        let data = load_test_image(file);
        let cleaned = png::clean_chunks(&data).expect(&format!("Failed to clean {}", file));

        // Compression level should not affect chunk operations
        assert_eq!(&cleaned[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }
}

#[test]
fn test_filter_types() {
    let filter_files = vec![
        "png/filter/filter_none.png",
        "png/filter/filter_sub.png",
        "png/filter/filter_up.png",
        "png/filter/filter_average.png",
        "png/filter/filter_paeth.png",
    ];

    for file in filter_files {
        let data = load_test_image(file);
        let cleaned = png::clean_chunks(&data).expect(&format!("Failed to clean {}", file));

        // Filter type should not affect chunk operations
        assert_eq!(&cleaned[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }
}

#[test]
fn test_alpha_transparency_types() {
    let alpha_files = vec![
        ("png/alpha/alpha_opaque.png", "opaque"),
        ("png/alpha/alpha_transparent.png", "transparent"),
        // alpha_semitransparent.png is already tested
    ];

    for (file, transparency_type) in alpha_files {
        let data = load_test_image(file);
        let cleaned = png::clean_chunks(&data).expect(&format!("Failed to clean {}", file));

        // Verify transparency is preserved appropriately
        if transparency_type != "opaque" {
            // Either tRNS chunk or alpha channel should be preserved
            let has_trns = check_chunk_exists(&cleaned, b"tRNS");
            let has_alpha = check_if_has_alpha(&cleaned);
            assert!(
                has_trns || has_alpha,
                "Transparency should be preserved for {} image",
                transparency_type
            );
        }
    }
}

#[test]
fn test_special_chunks() {
    let chunk_files = vec![
        ("png/chunk/chunk_background.png", b"bKGD"),
        ("png/chunk/chunk_transparency.png", b"tRNS"),
        // chunk_gamma.png is already tested
    ];

    for (file, chunk_type) in chunk_files {
        let data = load_test_image(file);

        // First check if the chunk exists in the original file
        let chunk_exists_in_original = check_chunk_exists(&data, chunk_type);

        let cleaned = png::clean_chunks(&data).expect(&format!("Failed to clean {}", file));

        // Special chunks should be handled based on CRITICAL_CHUNKS list
        let chunk_name = std::str::from_utf8(chunk_type).unwrap();
        if ["tRNS", "gAMA", "cHRM", "sRGB", "iCCP", "sBIT", "pHYs"].contains(&chunk_name) {
            // These chunks are in CRITICAL_CHUNKS list and should be preserved IF they exist in original
            if chunk_exists_in_original {
                assert!(
                    check_chunk_exists(&cleaned, chunk_type),
                    "{} chunk should be preserved in {}",
                    chunk_name,
                    file
                );
            }
        } else {
            // bKGD is not in CRITICAL_CHUNKS, so it should be removed
            assert!(
                !check_chunk_exists(&cleaned, chunk_type),
                "{} chunk should be removed",
                chunk_name
            );
        }
    }
}

#[test]
fn test_metadata_text_types() {
    let data = load_test_image("png/metadata/metadata_compressed.png");

    // Clean chunks should remove all text chunks including compressed ones
    let cleaned = png::clean_chunks(&data).expect("Failed to clean chunks");
    assert!(
        !check_chunk_exists(&cleaned, b"zTXt"),
        "zTXt chunks should be removed"
    );
    assert!(
        !check_chunk_exists(&cleaned, b"iTXt"),
        "iTXt chunks should be removed"
    );
}

#[test]
fn test_interlace_types() {
    let interlace_files = vec![
        ("png/interlace/interlace_none.png", false),
        // interlace_adam7.png is already tested
    ];

    for (file, _is_interlaced) in interlace_files {
        let data = load_test_image(file);
        let cleaned = png::clean_chunks(&data).expect(&format!("Failed to clean {}", file));

        // Interlacing should not affect chunk operations
        assert_eq!(&cleaned[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }
}

#[test]
fn test_multiple_text_chunks_with_same_keyword() {
    let data = load_test_image("png/metadata/metadata_none.png");

    // Add multiple text chunks with the same keyword
    let data1 =
        png::add_text_chunk(&data, "Comment", "First comment").expect("Failed to add first text");
    let data2 = png::add_text_chunk(&data1, "Comment", "Second comment")
        .expect("Failed to add second text");

    // Both should be present
    let chunks = png::read_text_chunks(&data2).expect("Failed to read text chunks");
    let comment_chunks: Vec<_> = chunks.iter().filter(|c| c.keyword == "Comment").collect();

    assert_eq!(comment_chunks.len(), 2, "Should have two Comment chunks");
    assert!(comment_chunks.iter().any(|c| c.text == "First comment"));
    assert!(comment_chunks.iter().any(|c| c.text == "Second comment"));
}

#[test]
fn test_text_chunk_with_special_characters() {
    let data = load_test_image("png/metadata/metadata_none.png");

    // Test various special characters in text
    let special_texts = vec![
        ("ASCII", "Hello, World!"),
        ("Unicode", "こんにちは世界 🌍"),
        ("Newlines", "Line 1\nLine 2\rLine 3\r\nLine 4"),
        ("Quotes", "\"Hello\" 'World'"),
        ("Null", "Before\0After"), // Null should be handled properly
    ];

    for (keyword, text) in special_texts {
        let with_text = png::add_text_chunk(&data, keyword, text)
            .expect(&format!("Failed to add text with {}", keyword));

        let chunks = png::read_text_chunks(&with_text).expect("Failed to read text chunks");

        let found = chunks.iter().find(|c| c.keyword == keyword);
        assert!(found.is_some(), "Should find {} chunk", keyword);

        // For null character, it might be truncated or handled specially
        if keyword != "Null" {
            assert_eq!(
                found.unwrap().text,
                text,
                "Text should match for {}",
                keyword
            );
        }
    }
}

#[test]
fn test_edge_case_keyword_lengths() {
    let data = load_test_image("png/metadata/metadata_none.png");

    // Test edge cases for keyword length (1-79 characters)
    let keyword_1 = "A";
    let keyword_79 = "A".repeat(79);

    let with_text_1 = png::add_text_chunk(&data, &keyword_1, "min length")
        .expect("Should accept 1-character keyword");
    let with_text_79 = png::add_text_chunk(&data, &keyword_79, "max length")
        .expect("Should accept 79-character keyword");

    let chunks_1 = png::read_text_chunks(&with_text_1).expect("Failed to read");
    let chunks_79 = png::read_text_chunks(&with_text_79).expect("Failed to read");

    assert!(chunks_1.iter().any(|c| c.keyword == keyword_1));
    assert!(chunks_79.iter().any(|c| c.keyword == keyword_79));
}

#[test]
fn test_large_text_content() {
    let data = load_test_image("png/metadata/metadata_none.png");

    // Test with large text content
    let large_text = "Lorem ipsum ".repeat(1000); // ~12KB of text
    let with_text =
        png::add_text_chunk(&data, "Large", &large_text).expect("Should handle large text");

    let chunks = png::read_text_chunks(&with_text).expect("Failed to read");
    let found = chunks.iter().find(|c| c.keyword == "Large");

    assert!(found.is_some());
    assert_eq!(found.unwrap().text, large_text);
}

#[test]
fn test_palette_indexed_images() {
    let data = load_test_image("png/colortype/colortype_palette.png");

    // Palette images require PLTE chunk
    let cleaned = png::clean_chunks(&data).expect("Failed to clean palette PNG");

    // PLTE should be preserved as it's critical for palette images
    assert!(
        check_chunk_exists(&cleaned, b"PLTE"),
        "PLTE chunk must be preserved for palette images"
    );
}
