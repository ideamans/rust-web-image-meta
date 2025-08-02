# このプロジェクトは

JPEG と PNG のメタデータを操作するための Rust ライブラリです。

# ユースケース

- JPEG の EXIF においてはオリエンテーション以外の情報を削除、JPEG メタデータにおいては基本的な情報と EXIF・ICC 以外を削除することによる軽量化を提供する
  - 入力は JPEG データの []byte、出力は JPEG データの []byte とする
- JPEG のコメントを読み込む
  - 入力は JPEG データの[]byte、出力はコメントの string とする
- JPEG のコメントを書き込む
  - 入力は JPEG データの []byte とコメントの string、出力は JPEG データの []byte とする
- PNG の重要なチャンク以外を削除する
  - 入力は PNG データの []byte、出力は PNG データの []byte とする
- PNG のテキストコメントを読み込む
  - 入力は PNG データの []byte、出力はテキストコメントの string とする
- PNG のテキストコメント(チャンク)を書き込む
  - 入力は PNG データの []byte とテキストコメントの string、出力は PNG データの []byte とする

## PNG の重要なチャンク

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

# API

## モジュール構造

```rust
pub mod jpeg;
pub mod png;
```

## JPEG API (`jpeg`モジュール)

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

## PNG API (`png`モジュール)

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

## エラー型

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
            Error::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
            Error::Io(err) => write!(f, "IO error: {}", err),
            Error::ParseError(msg) => write!(f, "Parse error: {}", msg),
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

# 依存ライブラリ

JPEG と PNG の構造データを操作するライブラリで信頼の高いものがあれば積極的に利用する。

# テスト

@tests/test_data 以下に事前に作成したテストデータがあり、@tests/test_data/index.json に説明があります。

これらを使い、メタデータのテストを行うことができます。

テストは、削除されるべきメタデータやチャンクが削除され、必要なものが残っていることを特に確認してください。

コメントの読み書きについても、既存のコメントを読めること、書き込んだコメントが正しく保存され、それを読み取ることができるか確認します。
