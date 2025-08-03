# web-image-meta

Web画像に最適化されたJPEGとPNGのメタデータを操作するためのRustライブラリです。

## プロジェクト概要

このライブラリは、Web配信用の画像ファイルサイズを削減しながら、必要な情報（オリエンテーション、カラープロファイルなど）を保持することを目的としています。

## 実装済み機能

### JPEG処理
- **メタデータクリーニング** (`clean_metadata`)
  - EXIF情報の削除（オリエンテーション情報は保持）
  - 不要なAPPマーカーの削除
  - ICCプロファイルの保持
  - JFIF情報の保持
- **コメント読み取り** (`read_comment`)
  - JPEGファイルからコメントを読み取り
- **コメント書き込み** (`write_comment`)
  - JPEGファイルにコメントを書き込み（既存のコメントは置換）

### PNG処理
- **チャンククリーニング** (`clean_chunks`)
  - 重要でないチャンクの削除
  - 必須チャンク（IHDR、PLTE、IDAT、IEND）の保持
  - 透明度チャンク（tRNS）の保持
  - カラー関連チャンク（gAMA、cHRM、sRGB、iCCP、sBIT）の保持
  - 物理寸法チャンク（pHYs）の保持
- **テキストチャンク読み取り** (`read_text_chunks`)
  - すべてのtEXtチャンクを読み取り
- **テキストチャンク追加** (`add_text_chunk`)
  - 新しいtEXtチャンクを追加

## API仕様

### モジュール構造

```rust
pub mod jpeg;
pub mod png;
```

### JPEG API (`jpeg`モジュール)

```rust
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
pub fn clean_metadata(data: &[u8]) -> Result<Vec<u8>, Error>

/// JPEG画像からコメントを読み取ります
pub fn read_comment(data: &[u8]) -> Result<Option<String>, Error>

/// JPEG画像にコメントを書き込みます
pub fn write_comment(data: &[u8], comment: &str) -> Result<Vec<u8>, Error>
```

### PNG API (`png`モジュール)

```rust
/// PNG画像から重要なチャンク以外を削除します
pub fn clean_chunks(data: &[u8]) -> Result<Vec<u8>, Error>

/// PNG画像から全てのtEXtチャンクを読み取ります
pub fn read_text_chunks(data: &[u8]) -> Result<Vec<TextChunk>, Error>

/// PNG画像に新しいtEXtチャンクを追加します
pub fn add_text_chunk(data: &[u8], keyword: &str, text: &str) -> Result<Vec<u8>, Error>

/// PNG tEXtチャンク
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChunk {
    pub keyword: String,  // 1-79文字のラテン文字キーワード
    pub text: String,     // テキスト内容
}
```

### 保持されるPNGチャンク

```
// Core
"IHDR": true,
"PLTE": true,
"IDAT": true,
"IEND": true,
// Transparency
"tRNS": true,
// Color space
"gAMA": true,
"cHRM": true,
"sRGB": true,
"iCCP": true,
"sBIT": true,
// Physical dimensions
"pHYs": true,
```

### エラー型

```rust
use std::fmt;
use std::error::Error as StdError;

#[derive(Debug)]
pub enum Error {
    /// 無効な画像フォーマット
    InvalidFormat(String),
    /// I/Oエラー
    Io(std::io::Error),
    /// パースエラー
    ParseError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidFormat(msg) => write!(f, "Invalid format: {msg}"),
            Error::Io(err) => write!(f, "IO error: {err}"),
            Error::ParseError(msg) => write!(f, "Parse error: {msg}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}
```

## 依存ライブラリ

- `jpeg-decoder` - JPEG画像のデコード検証
- `jpeg-encoder` - JPEG画像のエンコード
- `kamadak-exif` - EXIFデータの解析
- `png` - PNG画像の処理とデコード検証
- `crc32fast` - PNG CRC計算
- `thiserror` - エラー処理（dev-dependencyのみ）

## テスト

@tests/test_data 以下に事前に作成したテストデータがあり、@tests/test_data/index.json に説明があります。

これらを使い、メタデータのテストを行うことができます。

テストは、削除されるべきメタデータやチャンクが削除され、必要なものが残っていることを特に確認してください。

コメントの読み書きについても、既存のコメントを読めること、書き込んだコメントが正しく保存され、それを読み取ることができるか確認します。

### テストカバレッジ

- 53個の包括的なテストケース
- 様々な画像形式とエッジケースをカバー
- 出力画像の有効性を検証

## パフォーマンス考慮事項

- 最小限のメモリコピー
- ストリーミング処理は未実装（将来の改善点）
- バイナリデータの直接操作による高速処理

## 使用例

### JPEG画像の軽量化

```rust
use web_image_meta::jpeg;

let image_data = std::fs::read("photo.jpg")?;
let cleaned_data = jpeg::clean_metadata(&image_data)?;
std::fs::write("photo_cleaned.jpg", cleaned_data)?;
```

### PNG画像の軽量化

```rust
use web_image_meta::png;

let image_data = std::fs::read("image.png")?;
let cleaned_data = png::clean_chunks(&image_data)?;
std::fs::write("image_cleaned.png", cleaned_data)?;
```