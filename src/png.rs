use crate::Error;
use png::{ColorType, Decoder};
use std::collections::HashSet;
use std::io::Cursor;

/// PNG tEXtチャンク
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChunk {
    pub keyword: String, // 1-79文字のラテン文字キーワード
    pub text: String,    // テキスト内容
}

// 保持すべき重要なチャンクタイプ
const CRITICAL_CHUNKS: &[&str] = &[
    // Core
    "IHDR", "PLTE", "IDAT", "IEND", // Transparency
    "tRNS", // Color space
    "gAMA", "cHRM", "sRGB", "iCCP", "sBIT", // Physical dimensions
    "pHYs",
];

/// PNG画像から重要なチャンク以外を削除します
pub fn clean_chunks(data: &[u8]) -> Result<Vec<u8>, Error> {
    // PNGシグネチャの確認
    if data.len() < 8 || data[0..8] != [137, 80, 78, 71, 13, 10, 26, 10] {
        return Err(Error::InvalidFormat("Not a valid PNG file".to_string()));
    }

    // PNGが正常にデコードできるか検証
    validate_png_decode(data)?;

    let critical_set: HashSet<&str> = CRITICAL_CHUNKS.iter().cloned().collect();
    let mut output = Vec::new();

    // PNGシグネチャをコピー
    output.extend_from_slice(&data[0..8]);

    let mut pos = 8;

    while pos < data.len() {
        // チャンクの長さを読み取る
        if pos + 4 > data.len() {
            return Err(Error::ParseError("Unexpected end of PNG data".to_string()));
        }

        let length =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;

        // チャンクタイプを読み取る
        if pos + 8 > data.len() {
            return Err(Error::ParseError("Unexpected end of PNG data".to_string()));
        }

        let chunk_type = std::str::from_utf8(&data[pos + 4..pos + 8])
            .map_err(|_| Error::ParseError("Invalid chunk type".to_string()))?;

        // チャンク全体のサイズ（長さ + タイプ + データ + CRC）
        let chunk_size = 12 + length;
        if pos + chunk_size > data.len() {
            return Err(Error::ParseError("Chunk extends beyond file".to_string()));
        }

        // 重要なチャンクのみコピー
        if critical_set.contains(chunk_type) {
            output.extend_from_slice(&data[pos..pos + chunk_size]);
        }

        pos += chunk_size;

        // IENDチャンクに到達したら終了
        if chunk_type == "IEND" {
            break;
        }
    }

    // 出力が有効なPNGか検証
    validate_png_decode(&output)?;

    Ok(output)
}

/// PNG画像から全てのtEXtチャンクを読み取ります
pub fn read_text_chunks(data: &[u8]) -> Result<Vec<TextChunk>, Error> {
    // PNGシグネチャの確認
    if data.len() < 8 || data[0..8] != [137, 80, 78, 71, 13, 10, 26, 10] {
        return Err(Error::InvalidFormat("Not a valid PNG file".to_string()));
    }

    // PNGが正常にデコードできるか検証
    validate_png_decode(data)?;

    let mut text_chunks = Vec::new();
    let mut pos = 8;

    while pos < data.len() {
        // チャンクの長さを読み取る
        if pos + 4 > data.len() {
            break;
        }

        let length =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;

        // チャンクタイプを読み取る
        if pos + 8 > data.len() {
            break;
        }

        let chunk_type = &data[pos + 4..pos + 8];

        // チャンク全体のサイズ
        let chunk_size = 12 + length;
        if pos + chunk_size > data.len() {
            break;
        }

        // tEXtチャンクの場合
        if chunk_type == b"tEXt" && length > 0 {
            let chunk_data = &data[pos + 8..pos + 8 + length];

            // null終端でキーワードとテキストを分離
            if let Some(null_pos) = chunk_data.iter().position(|&b| b == 0) {
                let keyword = String::from_utf8_lossy(&chunk_data[..null_pos]).to_string();
                let text = if null_pos + 1 < chunk_data.len() {
                    String::from_utf8_lossy(&chunk_data[null_pos + 1..]).to_string()
                } else {
                    String::new()
                };

                text_chunks.push(TextChunk { keyword, text });
            }
        }

        pos += chunk_size;

        // IENDチャンクに到達したら終了
        if chunk_type == b"IEND" {
            break;
        }
    }

    Ok(text_chunks)
}

/// PNG画像に新しいtEXtチャンクを追加します
pub fn add_text_chunk(data: &[u8], keyword: &str, text: &str) -> Result<Vec<u8>, Error> {
    // PNGシグネチャの確認
    if data.len() < 8 || data[0..8] != [137, 80, 78, 71, 13, 10, 26, 10] {
        return Err(Error::InvalidFormat("Not a valid PNG file".to_string()));
    }

    // PNGが正常にデコードできるか検証
    validate_png_decode(data)?;

    // キーワードの検証
    if keyword.is_empty() || keyword.len() > 79 {
        return Err(Error::InvalidFormat(
            "Keyword must be 1-79 characters".to_string(),
        ));
    }

    // キーワードがラテン文字のみか確認
    if !keyword
        .chars()
        .all(|c| c.is_ascii() && (c.is_alphanumeric() || c == ' '))
    {
        return Err(Error::InvalidFormat(
            "Keyword must contain only Latin characters".to_string(),
        ));
    }

    let mut output = Vec::new();
    output.extend_from_slice(&data[0..8]); // PNGシグネチャ

    let mut pos = 8;
    let mut iend_pos = None;

    // IENDチャンクの位置を探す
    while pos < data.len() {
        if pos + 8 > data.len() {
            break;
        }

        let length =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let chunk_type = &data[pos + 4..pos + 8];
        let chunk_size = 12 + length;

        if chunk_type == b"IEND" {
            iend_pos = Some(pos);
            break;
        }

        if pos + chunk_size > data.len() {
            break;
        }

        pos += chunk_size;
    }

    let iend_start =
        iend_pos.ok_or_else(|| Error::ParseError("IEND chunk not found".to_string()))?;

    // IENDチャンクの前までコピー
    output.extend_from_slice(&data[8..iend_start]);

    // 新しいtEXtチャンクを作成
    let mut chunk_data = Vec::new();
    chunk_data.extend_from_slice(keyword.as_bytes());
    chunk_data.push(0); // null separator
    chunk_data.extend_from_slice(text.as_bytes());

    // チャンクを書き込む
    output.extend_from_slice(&(chunk_data.len() as u32).to_be_bytes()); // 長さ
    output.extend_from_slice(b"tEXt"); // タイプ
    output.extend_from_slice(&chunk_data); // データ

    // CRCを計算
    let crc = calculate_crc(b"tEXt", &chunk_data);
    output.extend_from_slice(&crc.to_be_bytes());

    // IENDチャンク以降をコピー
    output.extend_from_slice(&data[iend_start..]);

    // 出力が有効なPNGか検証
    validate_png_decode(&output)?;

    Ok(output)
}

/// CRC-32を計算
fn calculate_crc(chunk_type: &[u8], data: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(chunk_type);
    hasher.update(data);
    hasher.finalize()
}

/// PNGデータが正常にデコードできるか検証
fn validate_png_decode(data: &[u8]) -> Result<(), Error> {
    let cursor = Cursor::new(data);
    let decoder = Decoder::new(cursor);

    // ヘッダーを読み込んでデコード可能か確認
    match decoder.read_info() {
        Ok(reader) => {
            // 基本情報の取得を試みる
            let info = reader.info();

            // 画像の基本パラメータを検証
            if info.width == 0 || info.height == 0 {
                return Err(Error::InvalidFormat("Invalid image dimensions".to_string()));
            }

            // ビット深度の検証
            let bit_depth = info.bit_depth;
            if !matches!(
                bit_depth,
                png::BitDepth::One
                    | png::BitDepth::Two
                    | png::BitDepth::Four
                    | png::BitDepth::Eight
                    | png::BitDepth::Sixteen
            ) {
                return Err(Error::InvalidFormat("Invalid bit depth".to_string()));
            }

            // カラータイプの検証
            let color_type = info.color_type;
            if !matches!(
                color_type,
                ColorType::Grayscale
                    | ColorType::Rgb
                    | ColorType::Indexed
                    | ColorType::GrayscaleAlpha
                    | ColorType::Rgba
            ) {
                return Err(Error::InvalidFormat("Invalid color type".to_string()));
            }

            Ok(())
        }
        Err(e) => Err(Error::InvalidFormat(format!("Invalid PNG: {}", e))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_png_signature_validation() {
        let invalid_data = vec![0, 1, 2, 3];
        assert!(clean_chunks(&invalid_data).is_err());
        assert!(read_text_chunks(&invalid_data).is_err());
        assert!(add_text_chunk(&invalid_data, "test", "value").is_err());
    }

    #[test]
    fn test_keyword_validation() {
        let valid_png = vec![137, 80, 78, 71, 13, 10, 26, 10];

        // キーワードが空
        assert!(add_text_chunk(&valid_png, "", "text").is_err());

        // キーワードが長すぎる
        let long_keyword = "a".repeat(80);
        assert!(add_text_chunk(&valid_png, &long_keyword, "text").is_err());

        // 非ラテン文字
        assert!(add_text_chunk(&valid_png, "テスト", "text").is_err());
    }
}
