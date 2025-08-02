use std::fs;
use std::path::Path;
use web_image_meta::jpeg;
use web_image_meta::Error;

fn load_test_image(path: &str) -> Vec<u8> {
    let full_path = Path::new("tests/test_data").join(path);
    fs::read(full_path).expect(&format!("Failed to read test image: {}", path))
}

#[test]
fn test_clean_metadata_removes_exif_except_orientation() {
    let data = load_test_image("jpeg/orientation/orientation_6.jpg");
    let cleaned = jpeg::clean_metadata(&data).expect("Failed to clean metadata");

    // クリーンアップ後のサイズは元より小さくなるはず
    assert!(cleaned.len() < data.len());

    // 有効なJPEGファイルであることを確認
    assert_eq!(&cleaned[0..2], &[0xFF, 0xD8]);

    // オリエンテーション情報が保持されているか確認
    assert!(
        has_orientation_in_exif(&cleaned, 6),
        "Orientation value 6 should be preserved"
    );

    // 他のEXIFデータが削除されているか確認
    assert!(
        !has_exif_tag(&cleaned, 0x010F),
        "Make tag should be removed"
    ); // メーカー
    assert!(
        !has_exif_tag(&cleaned, 0x0110),
        "Model tag should be removed"
    ); // モデル
    assert!(
        !has_exif_tag(&cleaned, 0x9003),
        "DateTimeOriginal should be removed"
    ); // 撮影日時

    // コメントは削除されているはず
    let comment = jpeg::read_comment(&cleaned).expect("Failed to read comment");
    assert!(comment.is_none(), "Comments should be removed");
}

#[test]
fn test_clean_metadata_removes_all_metadata_when_no_orientation() {
    let data = load_test_image("jpeg/metadata/metadata_full_exif.jpg");
    let cleaned = jpeg::clean_metadata(&data).expect("Failed to clean metadata");

    // サイズが削減されているはず
    assert!(cleaned.len() < data.len());
    // 削減率は画像により異なる可能性があるため、削減されていることだけ確認

    assert_eq!(&cleaned[0..2], &[0xFF, 0xD8]);

    // EXIFマーカー(APP1)が存在しないか確認
    assert!(
        !has_marker(&cleaned, 0xE1),
        "EXIF marker (APP1) should be removed"
    );

    // その他のAPPマーカーも削除されているか確認
    for marker in 0xE3..=0xEF {
        assert!(
            !has_marker(&cleaned, marker),
            "APP marker 0x{:02X} should be removed",
            marker
        );
    }
}

#[test]
fn test_clean_metadata_preserves_icc_profile() {
    let data = load_test_image("jpeg/icc/icc_srgb.jpg");
    let cleaned = jpeg::clean_metadata(&data).expect("Failed to clean metadata");

    // ICCプロファイルマーカー (APP2) が保持されているか確認
    let mut has_icc = false;
    let mut pos = 2;

    while pos < cleaned.len() - 1 {
        if cleaned[pos] == 0xFF && cleaned[pos + 1] == 0xE2 {
            // APP2マーカーを発見
            if pos + 16 < cleaned.len() && &cleaned[pos + 4..pos + 16] == b"ICC_PROFILE\0" {
                has_icc = true;
                break;
            }
        }

        if cleaned[pos] == 0xFF && cleaned[pos + 1] == 0xDA {
            break; // SOSマーカーに到達
        }

        if cleaned[pos] == 0xFF && cleaned[pos + 1] >= 0xD0 && cleaned[pos + 1] <= 0xD9 {
            pos += 2;
            continue;
        }

        if pos + 4 > cleaned.len() {
            break;
        }

        let size = ((cleaned[pos + 2] as u16) << 8) | (cleaned[pos + 3] as u16);
        pos += 2 + size as usize;
    }

    assert!(has_icc, "ICC profile should be preserved");
}

#[test]
fn test_read_comment_with_existing_comment() {
    // コメント付きのテスト画像を作成
    let data = load_test_image("jpeg/metadata/metadata_none.jpg");
    let data_with_comment =
        jpeg::write_comment(&data, "Test comment 日本語").expect("Failed to write comment");

    let comment = jpeg::read_comment(&data_with_comment).expect("Failed to read comment");
    assert_eq!(comment, Some("Test comment 日本語".to_string()));
}

#[test]
fn test_read_comment_without_comment() {
    let data = load_test_image("jpeg/metadata/metadata_none.jpg");
    let comment = jpeg::read_comment(&data).expect("Failed to read comment");
    assert_eq!(comment, None);
}

#[test]
fn test_write_comment() {
    let data = load_test_image("jpeg/metadata/metadata_none.jpg");
    let comment_text = "This is a test comment with special chars: 日本語 émojis 🎯";

    let data_with_comment =
        jpeg::write_comment(&data, comment_text).expect("Failed to write comment");

    // コメントが正しく書き込まれたか確認
    let read_comment = jpeg::read_comment(&data_with_comment).expect("Failed to read comment");
    assert_eq!(read_comment, Some(comment_text.to_string()));

    // 有効なJPEGファイルであることを確認
    assert_eq!(&data_with_comment[0..2], &[0xFF, 0xD8]);

    // コメントマーカー(0xFE)が存在するか確認
    assert!(
        has_marker(&data_with_comment, 0xFE),
        "Comment marker should exist"
    );

    // コメントマーカーの位置が適切か確認（SOSマーカーの前）
    let com_pos = find_marker_position(&data_with_comment, 0xFE).expect("Comment marker not found");
    let sos_pos = find_marker_position(&data_with_comment, 0xDA);
    if let Some(sos) = sos_pos {
        assert!(com_pos < sos, "Comment should be placed before SOS marker");
    }
}

#[test]
fn test_write_comment_replaces_existing() {
    let data = load_test_image("jpeg/metadata/metadata_none.jpg");

    // 最初のコメントを書き込む
    let data_with_comment1 =
        jpeg::write_comment(&data, "First comment").expect("Failed to write first comment");

    // 最初のコメントが存在するか確認
    let comment1 = jpeg::read_comment(&data_with_comment1).expect("Failed to read first comment");
    assert_eq!(comment1, Some("First comment".to_string()));

    // 二番目のコメントで上書き
    let data_with_comment2 = jpeg::write_comment(&data_with_comment1, "Second comment")
        .expect("Failed to write second comment");

    // 最新のコメントのみが存在することを確認
    let read_comment = jpeg::read_comment(&data_with_comment2).expect("Failed to read comment");
    assert_eq!(read_comment, Some("Second comment".to_string()));

    // コメントマーカーが1つだけ存在するか確認
    let comment_count = count_markers(&data_with_comment2, 0xFE);
    assert_eq!(comment_count, 1, "Should have exactly one comment marker");
}

#[test]
fn test_invalid_jpeg_data() {
    let invalid_data = vec![0x00, 0x01, 0x02, 0x03];

    assert!(matches!(
        jpeg::clean_metadata(&invalid_data),
        Err(Error::InvalidFormat(_))
    ));

    assert!(matches!(
        jpeg::read_comment(&invalid_data),
        Err(Error::InvalidFormat(_))
    ));

    assert!(matches!(
        jpeg::write_comment(&invalid_data, "test"),
        Err(Error::InvalidFormat(_))
    ));
}

#[test]
fn test_corrupted_jpeg_decode() {
    // 有効なJPEGヘッダーだが破損したデータ
    let mut corrupted_data = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
    corrupted_data.extend_from_slice(b"JFIF\0");
    corrupted_data.extend_from_slice(&[0x01, 0x01, 0x00, 0x00, 0x48, 0x00, 0x48]);
    // 不完全なデータで終了

    assert!(matches!(
        jpeg::clean_metadata(&corrupted_data),
        Err(Error::InvalidFormat(_))
    ));
}

#[test]
fn test_valid_jpeg_decode() {
    // 実際の有効なJPEGファイルをテスト
    let data = load_test_image("jpeg/metadata/metadata_none.jpg");

    // すべての関数で正常にデコードできることを確認
    let cleaned = jpeg::clean_metadata(&data).expect("Should decode valid JPEG");
    assert!(!cleaned.is_empty());

    let comment = jpeg::read_comment(&data).expect("Should decode valid JPEG");
    assert!(comment.is_none());

    let with_comment = jpeg::write_comment(&data, "test").expect("Should decode valid JPEG");
    assert!(!with_comment.is_empty());
}

#[test]
fn test_empty_comment() {
    let data = load_test_image("jpeg/metadata/metadata_none.jpg");
    let data_with_comment = jpeg::write_comment(&data, "").expect("Failed to write empty comment");

    let comment = jpeg::read_comment(&data_with_comment).expect("Failed to read comment");
    assert_eq!(comment, Some("".to_string()));
}

#[test]
fn test_progressive_jpeg() {
    let data = load_test_image("jpeg/encoding/encoding_progressive.jpg");
    let cleaned = jpeg::clean_metadata(&data).expect("Failed to clean progressive JPEG");

    // プログレッシブJPEGも正しく処理できることを確認
    assert!(cleaned.len() < data.len());
    assert_eq!(&cleaned[0..2], &[0xFF, 0xD8]);
}

#[test]
fn test_cmyk_colorspace() {
    let data = load_test_image("jpeg/colorspace/colorspace_cmyk.jpg");
    let cleaned = jpeg::clean_metadata(&data).expect("Failed to clean CMYK JPEG");

    // CMYK色空間のJPEGも正しく処理できることを確認
    assert_eq!(&cleaned[0..2], &[0xFF, 0xD8]);
}

#[test]
fn test_all_orientation_values() {
    let orientation_files = vec![
        ("jpeg/orientation/orientation_1.jpg", 1),
        ("jpeg/orientation/orientation_3.jpg", 3),
        ("jpeg/orientation/orientation_6.jpg", 6),
        ("jpeg/orientation/orientation_8.jpg", 8),
    ];

    for (file, expected_orientation) in orientation_files {
        let data = load_test_image(file);
        let cleaned = jpeg::clean_metadata(&data).expect(&format!("Failed to clean {}", file));

        // すべてのオリエンテーション値で正しく処理できることを確認
        assert_eq!(&cleaned[0..2], &[0xFF, 0xD8]);
        assert!(cleaned.len() <= data.len());

        // 元データにオリエンテーション情報があるか確認
        if has_orientation_in_exif(&data, expected_orientation) {
            // クリーンアップ後も保持されているか確認
            assert!(
                has_orientation_in_exif(&cleaned, expected_orientation),
                "Orientation {} should be preserved in {}",
                expected_orientation,
                file
            );
        }
    }
}

// ヘルパー関数：特定のマーカーが存在するかチェック
fn has_marker(data: &[u8], marker: u8) -> bool {
    let mut pos = 2;
    while pos < data.len() - 1 {
        if data[pos] != 0xFF {
            return false;
        }

        let current_marker = data[pos + 1];
        if current_marker == marker {
            return true;
        }

        pos += 2;

        // SOSマーカー以降はスキップ
        if current_marker == 0xDA {
            break;
        }

        // スタンドアロンマーカー
        if current_marker >= 0xD0 && current_marker <= 0xD9 {
            continue;
        }

        // セグメントサイズを読み取る
        if pos + 2 > data.len() {
            break;
        }

        let size = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
        pos += size as usize;
    }
    false
}

// ヘルパー関数：マーカーの位置を検索
fn find_marker_position(data: &[u8], marker: u8) -> Option<usize> {
    let mut pos = 2;
    while pos < data.len() - 1 {
        if data[pos] != 0xFF {
            return None;
        }

        let current_marker = data[pos + 1];
        if current_marker == marker {
            return Some(pos);
        }

        pos += 2;

        // SOSマーカー以降はスキップ
        if current_marker == 0xDA {
            break;
        }

        // スタンドアロンマーカー
        if current_marker >= 0xD0 && current_marker <= 0xD9 {
            continue;
        }

        // セグメントサイズを読み取る
        if pos + 2 > data.len() {
            break;
        }

        let size = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
        pos += size as usize;
    }
    None
}

#[test]
fn test_critical_cases() {
    // Test critical JPEG files that might break processing
    let critical_files = vec![
        "jpeg/critical/critical_cmyk_lowquality.jpg",
        "jpeg/critical/critical_jfif_exif_dpi.jpg",
        "jpeg/critical/critical_orientation_metadata.jpg",
        "jpeg/critical/critical_progressive_fullmeta.jpg",
        "jpeg/critical/critical_thumbnail_progressive.jpg",
        "jpeg/critical/critical_xmp_complex.jpg",
        "jpeg/critical/critical_xmp_iptc_conflict.jpg",
    ];

    for file in critical_files {
        let data = load_test_image(file);

        // All critical files should be processable without panicking
        let result = jpeg::clean_metadata(&data);
        assert!(result.is_ok(), "Failed to process critical file: {}", file);

        // Verify output is still valid JPEG
        let cleaned = result.unwrap();
        assert!(!cleaned.is_empty());
        assert_eq!(&cleaned[0..2], &[0xFF, 0xD8]);
    }
}

#[test]
fn test_various_quality_levels() {
    let quality_files = vec![
        ("jpeg/quality/quality_20.jpg", 20),
        ("jpeg/quality/quality_50.jpg", 50),
        ("jpeg/quality/quality_80.jpg", 80),
        ("jpeg/quality/quality_95.jpg", 95),
    ];

    for (file, _quality) in quality_files {
        let data = load_test_image(file);
        let cleaned = jpeg::clean_metadata(&data).expect(&format!("Failed to clean {}", file));

        // Quality should not affect metadata cleaning
        assert!(
            cleaned.len() < data.len(),
            "Quality {} file should be smaller after cleaning",
            file
        );

        // Verify comments work on all quality levels
        let with_comment =
            jpeg::write_comment(&cleaned, "Quality test").expect("Failed to write comment");
        let read_comment = jpeg::read_comment(&with_comment).expect("Failed to read comment");
        assert_eq!(read_comment, Some("Quality test".to_string()));
    }
}

#[test]
fn test_various_subsampling() {
    let subsampling_files = vec![
        "jpeg/subsampling/subsampling_420.jpg",
        "jpeg/subsampling/subsampling_422.jpg",
        "jpeg/subsampling/subsampling_444.jpg",
    ];

    for file in subsampling_files {
        let data = load_test_image(file);
        let cleaned = jpeg::clean_metadata(&data).expect(&format!("Failed to clean {}", file));

        // Subsampling should not affect metadata operations
        assert_eq!(&cleaned[0..2], &[0xFF, 0xD8]);
    }
}

#[test]
fn test_dpi_metadata_handling() {
    let dpi_files = vec![
        ("jpeg/dpi/dpi_exif_72dpi.jpg", true),
        ("jpeg/dpi/dpi_exif_200dpi.jpg", true),
        ("jpeg/dpi/dpi_jfif_72dpi.jpg", false),
        ("jpeg/dpi/dpi_jfif_200dpi.jpg", false),
        ("jpeg/dpi/dpi_jfif_units0.jpg", false),
    ];

    for (file, has_exif_dpi) in dpi_files {
        let data = load_test_image(file);
        let cleaned = jpeg::clean_metadata(&data).expect(&format!("Failed to clean {}", file));

        // DPI in EXIF should be removed, DPI in JFIF should be preserved
        if has_exif_dpi {
            // EXIF DPI info should be removed with other EXIF data
            assert!(
                !has_marker(&cleaned, 0xE1),
                "EXIF should be removed from {}",
                file
            );
        }

        // JFIF (APP0) should always be preserved
        assert!(
            has_marker(&cleaned, 0xE0),
            "JFIF should be preserved in {}",
            file
        );
    }
}

#[test]
fn test_metadata_types() {
    // Test various metadata types
    let metadata_files = vec![
        ("jpeg/metadata/metadata_basic_exif.jpg", "basic EXIF"),
        ("jpeg/metadata/metadata_gps.jpg", "GPS data"),
        ("jpeg/metadata/metadata_iptc.jpg", "IPTC data"),
        ("jpeg/metadata/metadata_xmp.jpg", "XMP data"),
    ];

    for (file, metadata_type) in metadata_files {
        let data = load_test_image(file);
        let cleaned = jpeg::clean_metadata(&data).expect(&format!("Failed to clean {}", file));

        // All metadata except orientation should be removed
        assert!(
            cleaned.len() < data.len(),
            "{} should be removed from {}",
            metadata_type,
            file
        );

        // Verify specific metadata markers are removed
        match metadata_type {
            "GPS data" | "basic EXIF" => {
                // GPS is stored in EXIF
                assert!(
                    !has_marker(&cleaned, 0xE1) || !has_exif_tag(&cleaned, 0x8825),
                    "GPS IFD pointer should be removed"
                );
            }
            "IPTC data" => {
                // IPTC is often in APP13
                assert!(!has_marker(&cleaned, 0xED), "IPTC marker should be removed");
            }
            "XMP data" => {
                // XMP is in APP1 with different signature
                assert!(!contains_xmp(&cleaned), "XMP data should be removed");
            }
            _ => {}
        }
    }
}

#[test]
fn test_thumbnail_handling() {
    let thumbnail_files = vec![
        ("jpeg/thumbnail/thumbnail_embedded.jpg", true),
        ("jpeg/thumbnail/thumbnail_none.jpg", false),
    ];

    for (file, has_thumbnail) in thumbnail_files {
        let data = load_test_image(file);
        let cleaned = jpeg::clean_metadata(&data).expect(&format!("Failed to clean {}", file));

        if has_thumbnail {
            // Embedded thumbnails in EXIF should be removed
            assert!(
                cleaned.len() < data.len(),
                "File with thumbnail should be smaller after cleaning"
            );

            // Verify no EXIF remains (thumbnails are in EXIF IFD1)
            assert!(
                !has_marker(&cleaned, 0xE1),
                "EXIF with thumbnail should be removed"
            );
        }
    }
}

#[test]
fn test_icc_profile_types() {
    let icc_files = vec![
        ("jpeg/icc/icc_applep3.jpg", "Apple P3"),
        ("jpeg/icc/icc_none.jpg", "No ICC"),
        // icc_srgb.jpg is already tested
    ];

    for (file, profile_type) in icc_files {
        let data = load_test_image(file);
        let cleaned = jpeg::clean_metadata(&data).expect(&format!("Failed to clean {}", file));

        // Check if ICC profile is preserved when present
        if profile_type != "No ICC" {
            assert!(
                has_icc_profile(&cleaned),
                "ICC profile {} should be preserved",
                profile_type
            );
        }
    }
}

#[test]
fn test_grayscale_handling() {
    let data = load_test_image("jpeg/colorspace/colorspace_grayscale.jpg");
    let cleaned = jpeg::clean_metadata(&data).expect("Failed to clean grayscale JPEG");

    // Grayscale JPEGs should work the same as color
    assert_eq!(&cleaned[0..2], &[0xFF, 0xD8]);

    // Test comment functionality on grayscale
    let with_comment =
        jpeg::write_comment(&cleaned, "Grayscale test").expect("Failed to write comment");
    let comment = jpeg::read_comment(&with_comment).expect("Failed to read comment");
    assert_eq!(comment, Some("Grayscale test".to_string()));
}

// Helper function to check for XMP data
fn contains_xmp(data: &[u8]) -> bool {
    let mut pos = 2;
    while pos < data.len() - 1 {
        if data[pos] != 0xFF {
            return false;
        }

        let marker = data[pos + 1];
        pos += 2;

        if marker == 0xDA {
            break;
        }

        if (0xD0..=0xD9).contains(&marker) {
            continue;
        }

        if pos + 2 > data.len() {
            break;
        }

        let size = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
        let segment_end = pos + size as usize;

        // Check for XMP signature in APP1
        if marker == 0xE1 && size > 35 && segment_end <= data.len() {
            if &data[pos + 2..pos + 35] == b"http://ns.adobe.com/xap/1.0/\0" {
                return true;
            }
        }

        pos = segment_end;
    }
    false
}

// Helper function to check for ICC profile
fn has_icc_profile(data: &[u8]) -> bool {
    let mut pos = 2;
    while pos < data.len() - 1 {
        if data[pos] != 0xFF {
            return false;
        }

        let marker = data[pos + 1];
        pos += 2;

        if marker == 0xDA {
            break;
        }

        if (0xD0..=0xD9).contains(&marker) {
            continue;
        }

        if pos + 2 > data.len() {
            break;
        }

        let size = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
        let segment_end = pos + size as usize;

        // Check for ICC_PROFILE in APP2
        if marker == 0xE2 && size > 14 && segment_end <= data.len() {
            if &data[pos + 2..pos + 14] == b"ICC_PROFILE\0" {
                return true;
            }
        }

        pos = segment_end;
    }
    false
}

// ヘルパー関数：マーカーの数をカウント
fn count_markers(data: &[u8], marker: u8) -> usize {
    let mut count = 0;
    let mut pos = 2;
    while pos < data.len() - 1 {
        if data[pos] != 0xFF {
            break;
        }

        let current_marker = data[pos + 1];
        if current_marker == marker {
            count += 1;
        }

        pos += 2;

        // SOSマーカー以降はスキップ
        if current_marker == 0xDA {
            break;
        }

        // スタンドアロンマーカー
        if current_marker >= 0xD0 && current_marker <= 0xD9 {
            continue;
        }

        // セグメントサイズを読み取る
        if pos + 2 > data.len() {
            break;
        }

        let size = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
        pos += size as usize;
    }
    count
}

// ヘルパー関数：EXIF内のオリエンテーション値を確認
fn has_orientation_in_exif(data: &[u8], expected_value: u16) -> bool {
    // APP1 (EXIF) マーカーを探す
    let mut pos = 2;
    while pos < data.len() - 1 {
        if data[pos] != 0xFF {
            return false;
        }

        let marker = data[pos + 1];
        pos += 2;

        if marker == 0xDA {
            break;
        }

        if marker >= 0xD0 && marker <= 0xD9 {
            continue;
        }

        if pos + 2 > data.len() {
            break;
        }

        let size = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
        let segment_end = pos + size as usize;

        if marker == 0xE1 && size > 8 && segment_end <= data.len() {
            if &data[pos + 2..pos + 6] == b"Exif" {
                // EXIFデータを解析
                let exif_data = &data[pos + 8..segment_end];
                if let Some(orientation) = extract_orientation_from_exif(exif_data) {
                    return orientation == expected_value;
                }
            }
        }

        pos = segment_end;
    }
    false
}

// ヘルパー関数：EXIF内の特定タグが存在するか確認
fn has_exif_tag(data: &[u8], tag_id: u16) -> bool {
    // 簡易的な実装：APP1マーカー内でtag_idのバイトパターンを検索
    let mut pos = 2;
    while pos < data.len() - 1 {
        if data[pos] != 0xFF {
            return false;
        }

        let marker = data[pos + 1];
        pos += 2;

        if marker == 0xDA {
            break;
        }

        if marker >= 0xD0 && marker <= 0xD9 {
            continue;
        }

        if pos + 2 > data.len() {
            break;
        }

        let size = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
        let segment_end = pos + size as usize;

        if marker == 0xE1 && size > 8 && segment_end <= data.len() {
            if &data[pos + 2..pos + 6] == b"Exif" {
                // EXIFデータ内でタグを検索（簡易版）
                let tag_bytes_be = tag_id.to_be_bytes();
                let tag_bytes_le = tag_id.to_le_bytes();
                let exif_data = &data[pos + 8..segment_end];

                for i in 0..exif_data.len().saturating_sub(1) {
                    if (exif_data[i] == tag_bytes_be[0] && exif_data[i + 1] == tag_bytes_be[1])
                        || (exif_data[i] == tag_bytes_le[0] && exif_data[i + 1] == tag_bytes_le[1])
                    {
                        return true;
                    }
                }
            }
        }

        pos = segment_end;
    }
    false
}

// jpeg.rsからコピー（テスト用）
fn extract_orientation_from_exif(exif_data: &[u8]) -> Option<u16> {
    if exif_data.len() < 8 {
        return None;
    }

    let endian = if &exif_data[0..2] == b"II" {
        true
    } else if &exif_data[0..2] == b"MM" {
        false
    } else {
        return None;
    };

    let magic = if endian {
        u16::from_le_bytes([exif_data[2], exif_data[3]])
    } else {
        u16::from_be_bytes([exif_data[2], exif_data[3]])
    };

    if magic != 42 {
        return None;
    }

    let ifd0_offset = if endian {
        u32::from_le_bytes([exif_data[4], exif_data[5], exif_data[6], exif_data[7]]) as usize
    } else {
        u32::from_be_bytes([exif_data[4], exif_data[5], exif_data[6], exif_data[7]]) as usize
    };

    if ifd0_offset + 2 > exif_data.len() {
        return None;
    }

    let entry_count = if endian {
        u16::from_le_bytes([exif_data[ifd0_offset], exif_data[ifd0_offset + 1]]) as usize
    } else {
        u16::from_be_bytes([exif_data[ifd0_offset], exif_data[ifd0_offset + 1]]) as usize
    };

    for i in 0..entry_count {
        let entry_offset = ifd0_offset + 2 + (i * 12);
        if entry_offset + 12 > exif_data.len() {
            break;
        }

        let tag = if endian {
            u16::from_le_bytes([exif_data[entry_offset], exif_data[entry_offset + 1]])
        } else {
            u16::from_be_bytes([exif_data[entry_offset], exif_data[entry_offset + 1]])
        };

        if tag == 0x0112 {
            let value_offset = entry_offset + 8;
            let orientation = if endian {
                u16::from_le_bytes([exif_data[value_offset], exif_data[value_offset + 1]])
            } else {
                u16::from_be_bytes([exif_data[value_offset], exif_data[value_offset + 1]])
            };

            return Some(orientation);
        }
    }

    None
}
