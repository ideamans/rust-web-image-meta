// 2026-10-23: JFIF（APP0）の無い JPEG で、オリエンテーション用の最小 EXIF が
// ICC プロファイルの途中に挿入されていた。出力を1バイトずつ走査して最初の FF E0 を
// APP0 とみなしていたため、ICC データ中の FF E0 を誤認していた。
// 実例は Photoshop 書き出しの CMYK（Japan Color 2001 Coated、APP2 9分割）。
// Safari（ImageIO）では画像が表示されなくなる。
use std::fs;
use web_image_meta::jpeg;

/// (marker, payload) の列。SOS に着いたら、SOS 以降を丸ごと最後の要素として返す
fn segments(data: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut out = Vec::new();
    let mut pos = 2;
    loop {
        assert_eq!(data[pos], 0xFF, "marker expected at {pos}");
        let marker = data[pos + 1];
        if marker == 0xDA {
            out.push((marker, data[pos + 2..].to_vec()));
            return out;
        }
        let len = ((data[pos + 2] as usize) << 8) | data[pos + 3] as usize;
        out.push((marker, data[pos + 4..pos + 2 + len].to_vec()));
        pos += 2 + len;
    }
}

fn build(segs: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8];
    for (marker, payload) in segs {
        out.extend_from_slice(&[0xFF, *marker]);
        if *marker != 0xDA {
            let len = payload.len() + 2;
            out.extend_from_slice(&[(len >> 8) as u8, len as u8]);
        }
        out.extend_from_slice(payload);
    }
    out
}

fn icc_chunks(profile: &[u8], chunk: usize) -> Vec<(u8, Vec<u8>)> {
    let parts: Vec<&[u8]> = profile.chunks(chunk).collect();
    parts
        .iter()
        .enumerate()
        .map(|(i, part)| {
            let mut p = b"ICC_PROFILE\0".to_vec();
            p.push(i as u8 + 1);
            p.push(parts.len() as u8);
            p.extend_from_slice(part);
            (0xE2, p)
        })
        .collect()
}

fn icc_profile(data: &[u8]) -> Vec<u8> {
    segments(data)
        .into_iter()
        .filter(|(m, p)| *m == 0xE2 && p.starts_with(b"ICC_PROFILE\0"))
        .flat_map(|(_, p)| p[14..].to_vec())
        .collect()
}

fn orientation(data: &[u8]) -> Option<u32> {
    let (_, p) = segments(data)
        .into_iter()
        .find(|(m, p)| *m == 0xE1 && p.starts_with(b"Exif\0\0"))?;
    let exif = exif::Reader::new().read_raw(p[6..].to_vec()).ok()?;
    exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?
        .value
        .get_uint(0)
}

/// JFIF 無し・Orientation 付き Exif・データ中に FF E0 を含む ICC（2分割）の JPEG
fn jpeg_without_jfif(fake_len: u16) -> (Vec<u8>, Vec<u8>) {
    let base = fs::read("tests/test_data/jpeg/orientation/orientation_6.jpg").unwrap();
    let mut profile: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
    // ICC データの中に「FF E0 + 長さ」に見えるバイト列を置く
    profile[1000..1004].copy_from_slice(&[0xFF, 0xE0, (fake_len >> 8) as u8, fake_len as u8]);

    let mut segs = Vec::new();
    for (marker, payload) in segments(&base) {
        match marker {
            0xE0 | 0xFE => {} // JFIF とコメントを落とす（Photoshop 書き出しに寄せる）
            0xE1 => {
                segs.push((marker, payload));
                segs.extend(icc_chunks(&profile, 2000));
            }
            _ => segs.push((marker, payload)),
        }
    }
    (build(&segs), profile)
}

#[test]
fn test_clean_metadata_without_jfif_keeps_icc_intact() {
    let (data, profile) = jpeg_without_jfif(0x0020);
    assert_eq!(
        orientation(&data),
        Some(6),
        "fixture should carry orientation 6"
    );

    let cleaned = jpeg::clean_metadata(&data).expect("clean_metadata");

    jpeg::validate_structure(&cleaned).expect("cleaned JPEG must be structurally valid");
    assert_eq!(
        icc_profile(&cleaned),
        profile,
        "ICC profile must be preserved byte for byte"
    );
    assert_eq!(
        orientation(&cleaned),
        Some(6),
        "orientation must be preserved"
    );
    // 最小 EXIF は SOI の直後に入る
    assert_eq!(&cleaned[2..4], &[0xFF, 0xE1]);
}

#[test]
fn test_clean_metadata_without_jfif_keeps_scan_data_intact() {
    // 誤認した「長さ」が SOS の先まで届くと、EXIF が画像データの中に入っていた。
    // SOS より前の構造は正しいままなので validate_structure では見つからない
    let (data, profile) = jpeg_without_jfif(0x8000);
    let cleaned = jpeg::clean_metadata(&data).expect("clean_metadata");
    jpeg::validate_structure(&cleaned).expect("structurally valid");
    assert_eq!(icc_profile(&cleaned), profile);
    assert_eq!(
        segments(&cleaned).last(),
        segments(&data).last(),
        "scan data must be untouched"
    );
}

#[test]
fn test_clean_metadata_with_jfif_inserts_exif_after_app0() {
    let base = fs::read("tests/test_data/jpeg/orientation/orientation_6.jpg").unwrap();
    let cleaned = jpeg::clean_metadata(&base).expect("clean_metadata");
    jpeg::validate_structure(&cleaned).expect("structurally valid");
    let markers: Vec<u8> = segments(&cleaned).iter().map(|(m, _)| *m).collect();
    assert_eq!(&markers[..2], &[0xE0, 0xE1], "EXIF goes right after JFIF");
    assert_eq!(orientation(&cleaned), Some(6));
}

#[test]
fn test_validate_structure_rejects_gap_between_segments() {
    let (data, _) = jpeg_without_jfif(0x0020);
    let mut segs = segments(&data);
    // 実例と同じ壊れ方: セグメントの外に 36 バイトがはみ出している
    let sos = segs.pop().unwrap();
    let mut broken = build(&segs);
    broken.extend_from_slice(&[0x5A; 36]);
    broken.extend_from_slice(&[0xFF, 0xDA]);
    broken.extend_from_slice(&sos.1);

    assert!(jpeg::validate_structure(&data).is_ok());
    assert!(jpeg::validate_structure(&broken).is_err());
}

#[test]
fn test_validate_structure_rejects_truncated_segment() {
    let (data, _) = jpeg_without_jfif(0x0020);
    let header_end = data.windows(2).position(|w| w == [0xFF, 0xDA]).unwrap();
    assert!(jpeg::validate_structure(&data[..header_end - 10]).is_err());
}
