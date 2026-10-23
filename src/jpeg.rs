use crate::Error;
use jpeg_decoder::Decoder;

const JPEG_SOI: [u8; 2] = [0xFF, 0xD8];
const MARKER_COM: u8 = 0xFE;
const MARKER_APP1: u8 = 0xE1;
const MARKER_APP2: u8 = 0xE2;
const MARKER_APP14: u8 = 0xEE;

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
    // 最小 EXIF の挿入位置。JFIF（最初の APP0）があればその直後、無ければ SOI の直後。
    // 出力を後からバイト列として走査すると、ICC などのデータ中の FF E0 を
    // APP0 と取り違える（2026-10-23 の CMYK JPEG で ICC の途中に挿入されていた）
    let mut exif_insert_at = JPEG_SOI.len();
    let mut seen_app0 = false;
    let mut scan_start: Option<usize> = None;

    // JPEGマーカーを解析
    while pos < data.len() - 1 {
        if data[pos] != 0xFF {
            return Err(Error::ParseError("Invalid JPEG marker".to_string()));
        }

        let marker = data[pos + 1];
        pos += 2;

        // SOSマーカー以降は画像データなのでそのままコピー
        if marker == 0xDA {
            scan_start = Some(pos - 2);
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
            // APP14 (Adobe色空間情報) は保持
            MARKER_APP14 => {
                segment_size >= 14 && pos + 7 <= data.len() && &data[pos + 2..pos + 7] == b"Adobe"
            }
            // その他のAPPマーカーは削除 (0xE0, 0xE2, 0xEEは既に処理済みなので除外)
            0xE3..=0xED | 0xEF => false,
            // コメントは削除
            MARKER_COM => false,
            _ => false,
        };

        if keep_segment {
            output.extend_from_slice(&[0xFF, marker]);
            output.extend_from_slice(&data[pos..segment_end]);
            if marker == 0xE0 && !seen_app0 {
                seen_app0 = true;
                exif_insert_at = output.len();
            }
        }

        pos = segment_end;
    }

    // オリエンテーション情報がある場合は最小限のEXIFを追加
    if let Some(orientation_value) = orientation {
        if (1..=8).contains(&orientation_value) {
            let exif_data = create_minimal_exif(orientation_value)?;
            output.splice(exif_insert_at..exif_insert_at, exif_data);
        }
    }

    // 出力が有効なJPEGか検証（デコードできても構造が壊れていることがあるので、両方見る）
    validate_jpeg_decode(&output)?;
    validate_structure(&output)?;

    // 保持すると決めたものが変わっていないこと
    if icc_profile(&output) != icc_profile(data) {
        return Err(Error::ParseError(
            "ICC profile changed while cleaning metadata".to_string(),
        ));
    }
    if let Some(start) = scan_start {
        if !output.ends_with(&data[start..]) {
            return Err(Error::ParseError(
                "Image data changed while cleaning metadata".to_string(),
            ));
        }
    }

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

/// JPEGのマーカー構造を SOI から最初の SOS まで検証します
///
/// # Details
/// - 各セグメントがファイル内に収まり、次のマーカーまで隙間なく続いていること
///   （マーカー前の 0xFF の詰め物は規格どおり許す）
/// - SOS に到達し、その後に画像データがあること
///
/// デコーダはセグメントの外の余分なバイトを読み飛ばすので、デコードできても
/// 構造が壊れていることがある（Safari の ImageIO はそういう JPEG を表示しない）。
/// 画素の比較では見つからないので、上書きの前にはこれも確かめること。
pub fn validate_structure(data: &[u8]) -> Result<(), Error> {
    if data.len() < 4 || data[0..2] != JPEG_SOI {
        return Err(Error::InvalidFormat("Not a valid JPEG file".to_string()));
    }

    let mut pos = 2;
    loop {
        if pos >= data.len() {
            return Err(Error::ParseError("JPEG ends before SOS".to_string()));
        }
        if data[pos] != 0xFF {
            return Err(Error::ParseError(format!(
                "Unexpected byte 0x{:02X} at offset {pos} (expected a marker)",
                data[pos]
            )));
        }
        // 0xFF の詰め物を読み飛ばす
        while pos < data.len() && data[pos] == 0xFF {
            pos += 1;
        }
        if pos >= data.len() {
            return Err(Error::ParseError("JPEG ends before SOS".to_string()));
        }
        let marker = data[pos];
        pos += 1;

        match marker {
            0x00 => {
                return Err(Error::ParseError(format!(
                    "Stuffed byte outside scan data at offset {}",
                    pos - 2
                )))
            }
            0xD8 | 0xD9 => {
                return Err(Error::ParseError(format!(
                    "Unexpected marker 0x{marker:02X} before SOS at offset {}",
                    pos - 2
                )))
            }
            0x01 | 0xD0..=0xD7 => continue,
            _ => {}
        }

        if pos + 2 > data.len() {
            return Err(Error::ParseError("Unexpected end of JPEG data".to_string()));
        }
        let segment_size = ((data[pos] as usize) << 8) | (data[pos + 1] as usize);
        if segment_size < 2 {
            return Err(Error::ParseError("Invalid segment size".to_string()));
        }
        let segment_end = pos + segment_size;
        if segment_end > data.len() {
            return Err(Error::ParseError(format!(
                "Segment 0x{marker:02X} at offset {} extends beyond file",
                pos - 2
            )));
        }

        if marker == 0xDA {
            if segment_end >= data.len() {
                return Err(Error::ParseError("No image data after SOS".to_string()));
            }
            return Ok(());
        }
        pos = segment_end;
    }
}

/// APP2 に分割して入っている ICC プロファイルを連結して返します（無ければ None）
fn icc_profile(data: &[u8]) -> Option<Vec<u8>> {
    let mut chunks: Vec<(u8, &[u8])> = Vec::new();
    let mut pos = 2;
    while pos + 4 <= data.len() && data[pos] == 0xFF {
        let marker = data[pos + 1];
        if marker == 0xDA {
            break;
        }
        if (0xD0..=0xD9).contains(&marker) {
            pos += 2;
            continue;
        }
        let size = ((data[pos + 2] as usize) << 8) | (data[pos + 3] as usize);
        let end = pos + 2 + size;
        if size < 2 || end > data.len() {
            break;
        }
        let payload = &data[pos + 4..end];
        if marker == MARKER_APP2 && payload.len() >= 14 && &payload[..12] == b"ICC_PROFILE\0" {
            chunks.push((payload[12], &payload[14..]));
        }
        pos = end;
    }
    if chunks.is_empty() {
        return None;
    }
    chunks.sort_by_key(|(seq, _)| *seq);
    Some(chunks.iter().flat_map(|(_, p)| p.iter().copied()).collect())
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
        Err(e) => Err(Error::InvalidFormat(format!("Invalid JPEG: {e}"))),
    }
}

/// コメント追加によるファイルサイズの増加量を見積もります
///
/// # Arguments
/// * `comment` - 追加するコメント文字列
///
/// # Returns
/// * 追加されるバイト数（マーカー、サイズフィールド、コメントデータの合計）
///
/// # Details
/// JPEGコメントセグメントの構造:
/// - マーカー (0xFF 0xFE): 2バイト
/// - セグメントサイズ: 2バイト
/// - コメントデータ: comment.len()バイト
pub fn estimate_text_comment(comment: &str) -> usize {
    let comment_bytes = comment.as_bytes();
    // マーカー(2) + サイズフィールド(2) + コメントデータ
    2 + 2 + comment_bytes.len()
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
    validate_structure(&output)?;

    Ok(output)
}
