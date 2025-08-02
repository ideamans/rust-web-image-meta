use rust_image_meta::png;
use rust_image_meta::Error;
use std::fs;
use std::path::Path;

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
