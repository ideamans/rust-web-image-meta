use crate::Error;
use jpeg_decoder::Decoder;

const JPEG_SOI: [u8; 2] = [0xFF, 0xD8];
const MARKER_COM: u8 = 0xFE;
const MARKER_APP1: u8 = 0xE1;
const MARKER_APP2: u8 = 0xE2;

/// JPEG画像のメタデータを軽量化します
///
/// # Arguments
/// * `data` - JPEG画像のバイトデータ
///
/// # Returns
/// * `Ok(Vec<u8>)` - 軽量化されたJPEG画像データ
/// * `Err(Error)` - エラー
///
/// # Details
/// - EXIFのオリエンテーション情報は保持
/// - その他のEXIF情報を削除
/// - 基本的なメタデータとEXIF・ICC以外を削除
pub fn clean_metadata(data: &[u8]) -> Result<Vec<u8>, Error> {
    if data.len() < 4 || data[0..2] != JPEG_SOI {
        return Err(Error::InvalidFormat("Not a valid JPEG file".to_string()));
    }

    // JPEGが正常にデコードできるか検証
    validate_jpeg_decode(data)?;

    let mut output = Vec::new();
    output.extend_from_slice(&JPEG_SOI);

    let mut pos = 2;
    let mut has_exif = false;
    let mut orientation: Option<u16> = None;

    // JPEGマーカーを解析
    while pos < data.len() - 1 {
        if data[pos] != 0xFF {
            return Err(Error::ParseError("Invalid JPEG marker".to_string()));
        }

        let marker = data[pos + 1];
        pos += 2;

        // SOSマーカー以降は画像データなのでそのままコピー
        if marker == 0xDA {
            output.extend_from_slice(&[0xFF, marker]);
            output.extend_from_slice(&data[pos..]);
            break;
        }

        // スタンドアロンマーカーの場合
        if (0xD0..=0xD9).contains(&marker) {
            output.extend_from_slice(&[0xFF, marker]);
            continue;
        }

        // セグメントサイズを読み取る
        if pos + 2 > data.len() {
            return Err(Error::ParseError("Unexpected end of JPEG data".to_string()));
        }

        let segment_size = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
        if segment_size < 2 {
            return Err(Error::ParseError("Invalid segment size".to_string()));
        }

        let segment_end = pos + segment_size as usize;
        if segment_end > data.len() {
            return Err(Error::ParseError("Segment extends beyond file".to_string()));
        }

        // 保持するマーカーを判定
        let keep_segment = match marker {
            // 基本的な構造に必要なマーカー
            0xC0..=0xC3 | 0xC5..=0xCF => true, // SOF markers
            0xC4 => true,                      // DHT (Huffman tables)
            0xDB => true,                      // DQT (Quantization tables)
            0xDD => true,                      // DRI (Restart interval)
            // APP0 (JFIF) は保持
            0xE0 => true,
            // APP1 (EXIF) はオリエンテーション情報を抽出
            MARKER_APP1 => {
                if !has_exif && segment_size > 8 && &data[pos + 2..pos + 6] == b"Exif" {
                    has_exif = true;
                    // EXIFからオリエンテーションを抽出
                    // EXIFデータを簡易的に解析してオリエンテーションを取得
                    orientation = extract_orientation_from_exif(&data[pos + 8..segment_end]);
                }
                false
            }
            // APP2 (ICC Profile) は保持
            MARKER_APP2 => segment_size > 14 && &data[pos + 2..pos + 14] == b"ICC_PROFILE\0",
            // その他のAPPマーカーは削除 (0xE0は既に処理済みなので除外)
            0xE3..=0xEF => false,
            // コメントは削除
            MARKER_COM => false,
            _ => false,
        };

        if keep_segment {
            output.extend_from_slice(&[0xFF, marker]);
            output.extend_from_slice(&data[pos..segment_end]);
        }

        pos = segment_end;
    }

    // オリエンテーション情報がある場合は最小限のEXIFを追加
    if let Some(orientation_value) = orientation {
        if (1..=8).contains(&orientation_value) {
            let exif_data = create_minimal_exif(orientation_value)?;
            // JFIFマーカーの直後に挿入
            let mut final_output = Vec::new();
            let mut inserted = false;
            let mut i = 0;

            while i < output.len() - 1 {
                if output[i] == 0xFF && output[i + 1] == 0xE0 && !inserted {
                    // JFIFマーカーを見つけた
                    let marker_size = ((output[i + 2] as u16) << 8) | (output[i + 3] as u16);
                    let marker_end = i + 2 + marker_size as usize;
                    final_output.extend_from_slice(&output[i..marker_end]);
                    final_output.extend_from_slice(&exif_data);
                    inserted = true;
                    i = marker_end;
                } else {
                    final_output.push(output[i]);
                    i += 1;
                }
            }
            if i < output.len() {
                final_output.push(output[i]);
            }

            if !inserted {
                // JFIFマーカーがない場合はSOIの直後に挿入
                let mut temp = vec![0xFF, 0xD8];
                temp.extend_from_slice(&exif_data);
                temp.extend_from_slice(&output[2..]);
                return Ok(temp);
            }

            return Ok(final_output);
        }
    }

    // 出力が有効なJPEGか検証
    validate_jpeg_decode(&output)?;

    Ok(output)
}

/// 最小限のEXIFデータを作成（オリエンテーションのみ）
fn create_minimal_exif(orientation: u16) -> Result<Vec<u8>, Error> {
    let mut exif = Vec::new();

    // APP1マーカー
    exif.extend_from_slice(&[0xFF, MARKER_APP1]);

    // サイズは後で設定
    exif.extend_from_slice(&[0x00, 0x00]);

    // Exif識別子
    exif.extend_from_slice(b"Exif\0\0");

    // TIFF header (Little Endian)
    exif.extend_from_slice(&[0x49, 0x49]); // "II"
    exif.extend_from_slice(&[0x2A, 0x00]); // 42
    exif.extend_from_slice(&[0x08, 0x00, 0x00, 0x00]); // IFD0 offset

    // IFD0
    exif.extend_from_slice(&[0x01, 0x00]); // 1 entry

    // Orientation tag
    exif.extend_from_slice(&[0x12, 0x01]); // Tag 0x0112
    exif.extend_from_slice(&[0x03, 0x00]); // Type: SHORT
    exif.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // Count: 1
    exif.extend_from_slice(&[orientation as u8, (orientation >> 8) as u8, 0x00, 0x00]); // Value

    // Next IFD offset (none)
    exif.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);

    // サイズを設定
    let size = (exif.len() - 2) as u16;
    exif[2] = (size >> 8) as u8;
    exif[3] = size as u8;

    Ok(exif)
}

/// JPEG画像からコメントを読み取ります
pub fn read_comment(data: &[u8]) -> Result<Option<String>, Error> {
    if data.len() < 4 || data[0..2] != JPEG_SOI {
        return Err(Error::InvalidFormat("Not a valid JPEG file".to_string()));
    }

    // JPEGが正常にデコードできるか検証
    validate_jpeg_decode(data)?;

    let mut pos = 2;

    while pos < data.len() - 1 {
        if data[pos] != 0xFF {
            return Err(Error::ParseError("Invalid JPEG marker".to_string()));
        }

        let marker = data[pos + 1];
        pos += 2;

        // SOSマーカー以降は画像データ
        if marker == 0xDA {
            break;
        }

        // スタンドアロンマーカーの場合
        if (0xD0..=0xD9).contains(&marker) {
            continue;
        }

        // セグメントサイズを読み取る
        if pos + 2 > data.len() {
            return Err(Error::ParseError("Unexpected end of JPEG data".to_string()));
        }

        let segment_size = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
        if segment_size < 2 {
            return Err(Error::ParseError("Invalid segment size".to_string()));
        }

        let segment_end = pos + segment_size as usize;
        if segment_end > data.len() {
            return Err(Error::ParseError("Segment extends beyond file".to_string()));
        }

        // コメントマーカーの場合
        if marker == MARKER_COM {
            if segment_size > 2 {
                let comment_data = &data[pos + 2..segment_end];
                let comment = String::from_utf8_lossy(comment_data).to_string();
                return Ok(Some(comment));
            } else {
                // 空のコメント（セグメントサイズが2の場合）
                return Ok(Some(String::new()));
            }
        }

        pos = segment_end;
    }

    Ok(None)
}

/// EXIFデータからオリエンテーション値を抽出する簡易実装
fn extract_orientation_from_exif(exif_data: &[u8]) -> Option<u16> {
    // 最小限のEXIF解析
    if exif_data.len() < 8 {
        return None;
    }

    // Tiffヘッダーを確認 (II or MM)
    let endian = if &exif_data[0..2] == b"II" {
        // Little Endian
        true
    } else if &exif_data[0..2] == b"MM" {
        // Big Endian
        false
    } else {
        return None;
    };

    // 42のマジックナンバーを確認
    let magic = if endian {
        u16::from_le_bytes([exif_data[2], exif_data[3]])
    } else {
        u16::from_be_bytes([exif_data[2], exif_data[3]])
    };

    if magic != 42 {
        return None;
    }

    // IFD0のオフセットを取得
    let ifd0_offset = if endian {
        u32::from_le_bytes([exif_data[4], exif_data[5], exif_data[6], exif_data[7]]) as usize
    } else {
        u32::from_be_bytes([exif_data[4], exif_data[5], exif_data[6], exif_data[7]]) as usize
    };

    if ifd0_offset + 2 > exif_data.len() {
        return None;
    }

    // エントリ数を取得
    let entry_count = if endian {
        u16::from_le_bytes([exif_data[ifd0_offset], exif_data[ifd0_offset + 1]]) as usize
    } else {
        u16::from_be_bytes([exif_data[ifd0_offset], exif_data[ifd0_offset + 1]]) as usize
    };

    // 各エントリをチェック
    for i in 0..entry_count {
        let entry_offset = ifd0_offset + 2 + (i * 12);
        if entry_offset + 12 > exif_data.len() {
            break;
        }

        // タグを確認 (0x0112 = Orientation)
        let tag = if endian {
            u16::from_le_bytes([exif_data[entry_offset], exif_data[entry_offset + 1]])
        } else {
            u16::from_be_bytes([exif_data[entry_offset], exif_data[entry_offset + 1]])
        };

        if tag == 0x0112 {
            // オリエンテーション値を取得
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

/// JPEGデータが正常にデコードできるか検証
fn validate_jpeg_decode(data: &[u8]) -> Result<(), Error> {
    let mut decoder = Decoder::new(data);

    // ヘッダーを読み込んでデコード可能か確認
    match decoder.read_info() {
        Ok(_) => {
            // 基本情報の取得を試みる
            let info = decoder.info();
            if info.is_none() {
                return Err(Error::InvalidFormat("Failed to get JPEG info".to_string()));
            }

            // 画像の基本パラメータを検証
            let info = info.unwrap();
            if info.width == 0 || info.height == 0 {
                return Err(Error::InvalidFormat("Invalid image dimensions".to_string()));
            }

            Ok(())
        }
        Err(e) => Err(Error::InvalidFormat(format!("Invalid JPEG: {}", e))),
    }
}

/// JPEG画像にコメントを書き込みます
pub fn write_comment(data: &[u8], comment: &str) -> Result<Vec<u8>, Error> {
    if data.len() < 4 || data[0..2] != JPEG_SOI {
        return Err(Error::InvalidFormat("Not a valid JPEG file".to_string()));
    }

    // JPEGが正常にデコードできるか検証
    validate_jpeg_decode(data)?;

    let comment_bytes = comment.as_bytes();
    if comment_bytes.len() > 65533 {
        return Err(Error::InvalidFormat("Comment too long".to_string()));
    }

    let mut output = Vec::new();
    output.extend_from_slice(&JPEG_SOI);

    // コメントセグメントを作成
    let mut comment_segment = Vec::new();
    comment_segment.extend_from_slice(&[0xFF, MARKER_COM]);
    let segment_size = (comment_bytes.len() + 2) as u16;
    comment_segment.push((segment_size >> 8) as u8);
    comment_segment.push(segment_size as u8);
    comment_segment.extend_from_slice(comment_bytes);

    let mut pos = 2;
    let mut comment_inserted = false;

    // 既存のコメントを削除しつつ、適切な位置に新しいコメントを挿入
    while pos < data.len() - 1 {
        if data[pos] != 0xFF {
            return Err(Error::ParseError("Invalid JPEG marker".to_string()));
        }

        let marker = data[pos + 1];
        pos += 2;

        // APPマーカーの後、SOSマーカーの前にコメントを挿入
        if !comment_inserted && (marker == 0xDA || marker == 0xDB) {
            output.extend_from_slice(&comment_segment);
            comment_inserted = true;
        }

        // SOSマーカー以降は画像データなのでそのままコピー
        if marker == 0xDA {
            output.extend_from_slice(&[0xFF, marker]);
            output.extend_from_slice(&data[pos..]);
            break;
        }

        // スタンドアロンマーカーの場合
        if (0xD0..=0xD9).contains(&marker) {
            output.extend_from_slice(&[0xFF, marker]);
            continue;
        }

        // セグメントサイズを読み取る
        if pos + 2 > data.len() {
            return Err(Error::ParseError("Unexpected end of JPEG data".to_string()));
        }

        let segment_size = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
        if segment_size < 2 {
            return Err(Error::ParseError("Invalid segment size".to_string()));
        }

        let segment_end = pos + segment_size as usize;
        if segment_end > data.len() {
            return Err(Error::ParseError("Segment extends beyond file".to_string()));
        }

        // 既存のコメントは削除
        if marker != MARKER_COM {
            output.extend_from_slice(&[0xFF, marker]);
            output.extend_from_slice(&data[pos..segment_end]);
        }

        pos = segment_end;
    }

    // コメントがまだ挿入されていない場合（画像データがない場合）
    if !comment_inserted {
        output.extend_from_slice(&comment_segment);
    }

    // 出力が有効なJPEGか検証
    validate_jpeg_decode(&output)?;

    Ok(output)
}
