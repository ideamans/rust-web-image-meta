# web-image-meta

[![Crates.io](https://img.shields.io/crates/v/web-image-meta.svg)](https://crates.io/crates/web-image-meta)
[![Documentation](https://docs.rs/web-image-meta/badge.svg)](https://docs.rs/web-image-meta)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![CI](https://github.com/ideamans/rust-web-image-meta/workflows/CI/badge.svg)](https://github.com/ideamans/rust-web-image-meta/actions)

Web画像に最適化された、JPEGとPNGのメタデータを操作するための軽量なRustライブラリです。

## 機能

- **JPEGサポート**
  - オリエンテーション情報を保持しながらメタデータをクリーニング
  - JPEGコメントの読み書き
  - ICCプロファイルの保持
  - EXIF、XMP、IPTCおよびその他のメタデータの削除
  
- **PNGサポート**
  - 非重要チャンクの削除
  - テキストチャンク（tEXt）の読み書き
  - 透明度と色情報の保持

## インストール

`Cargo.toml`に以下を追加してください：

```toml
[dependencies]
web-image-meta = "0.1.0"
```

## 使用方法

### JPEG操作の例

```rust
use web_image_meta::jpeg;

// オリエンテーション情報を保持しながらJPEGメタデータをクリーニング
let input_data = std::fs::read("input.jpg")?;
let cleaned_data = jpeg::clean_metadata(&input_data)?;
std::fs::write("cleaned.jpg", cleaned_data)?;

// JPEGコメントの読み取り
let comment = jpeg::read_comment(&input_data)?;
if let Some(text) = comment {
    println!("コメント: {}", text);
}

// JPEGコメントの書き込み
let data_with_comment = jpeg::write_comment(&input_data, "Copyright 2024")?;
std::fs::write("commented.jpg", data_with_comment)?;
```

### PNG操作の例

```rust
use web_image_meta::png;

// PNGから非重要チャンクを削除
let input_data = std::fs::read("input.png")?;
let cleaned_data = png::clean_chunks(&input_data)?;
std::fs::write("cleaned.png", cleaned_data)?;

// PNGテキストチャンクの読み取り
let chunks = png::read_text_chunks(&input_data)?;
for chunk in chunks {
    println!("{}: {}", chunk.keyword, chunk.text);
}

// PNGにテキストチャンクを追加
let data_with_text = png::add_text_chunk(
    &input_data,
    "Copyright",
    "© 2024 Example Corp"
)?;
std::fs::write("tagged.png", data_with_text)?;
```

## APIリファレンス

### JPEG関数

#### `clean_metadata(data: &[u8]) -> Result<Vec<u8>, Error>`
EXIFオリエンテーション情報を除くすべてのメタデータを削除します。

- 保持する項目：JFIF、ICCプロファイル、必須JPEGマーカー、EXIFオリエンテーション（タグ0x0112）
- 削除する項目：その他のEXIFデータ、XMP、IPTC、コメント、APPマーカー（APP0、オリエンテーション付きAPP1、ICC付きAPP2を除く）
- 戻り値：クリーニングされたJPEGデータ

#### `read_comment(data: &[u8]) -> Result<Option<String>, Error>`
JPEGファイルからCOM（コメント）セグメントを読み取ります。

- 戻り値：コメントが存在する場合は`Some(String)`、存在しない場合は`None`
- エンコーディング：UTF-8（非UTF-8データは損失のある変換）

#### `write_comment(data: &[u8], comment: &str) -> Result<Vec<u8>, Error>`
JPEGファイルにコメントを書き込みまたは置き換えます。

- 既存のコメントは置き換えられます
- SOSマーカーの前に配置されます
- 最大長：65,533バイト

### PNG関数

#### `clean_chunks(data: &[u8]) -> Result<Vec<u8>, Error>`
PNGファイルからすべての非重要チャンクを削除します。

- 保持する項目：IHDR、PLTE、IDAT、IEND、tRNS、gAMA、cHRM、sRGB、iCCP、sBIT、pHYs
- 削除する項目：tEXt、zTXt、iTXt、tIME、bKGD、およびその他の補助チャンク
- 戻り値：クリーニングされたPNGデータ

#### `read_text_chunks(data: &[u8]) -> Result<Vec<TextChunk>, Error>`
PNGファイルからすべてのtEXtチャンクを読み取ります。

- 戻り値：`TextChunk`構造体のベクター
- 非圧縮のtEXtチャンクのみを読み取ります（zTXtやiTXtは対象外）

#### `add_text_chunk(data: &[u8], keyword: &str, text: &str) -> Result<Vec<u8>, Error>`
PNGファイルに新しいtEXtチャンクを追加します。

- キーワード：1-79文字のラテン文字（文字、数字、スペース）
- テキスト：任意の長さのUTF-8文字列
- IENDの前に新しいチャンクを配置します

### 型定義

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChunk {
    pub keyword: String,  // 1-79文字のラテン文字キーワード
    pub text: String,     // テキスト内容
}

#[derive(Debug)]
pub enum Error {
    InvalidFormat(String),  // 無効な画像フォーマット
    Io(std::io::Error),    // I/Oエラー
    ParseError(String),    // パースエラー
}
```

## 保持される項目

### JPEG
- 必須の画像データと構造
- EXIFオリエンテーション（タグ0x0112）（存在する場合）
- ICCカラープロファイル（APP2）
- JFIFマーカー（APP0）
- すべてのSOFマーカー（画像エンコーディングパラメータ）
- ハフマンテーブル（DHT）
- 量子化テーブル（DQT）

### PNG
- 重要チャンク：IHDR、PLTE、IDAT、IEND
- 透明度：tRNS
- 色空間：gAMA、cHRM、sRGB、iCCP、sBIT
- 物理寸法：pHYs

## 削除される項目

### JPEG
- EXIFデータ（オリエンテーションを除く）
- XMPメタデータ
- IPTCデータ
- コメント（clean_metadata使用時）
- Photoshopリソース（APP13）
- その他のAPPマーカー（APP3-APP15、ICC付きAPP2を除く）

### PNG
- テキストチャンク：tEXt、zTXt、iTXt
- 時刻チャンク：tIME
- 背景：bKGD
- ヒストグラム：hIST
- 推奨パレット：sPLT
- その他の補助チャンク

## エラーハンドリング

ライブラリは詳細なエラー型を提供します：
- `InvalidFormat`：入力が有効なJPEG/PNGファイルではない
- `ParseError`：ファイル構造が破損または無効
- `Io`：システムI/Oエラー

すべての関数は、出力画像がデコード可能であることを検証します。

## パフォーマンス

このライブラリはWeb画像の最適化のために設計されています：
- ファイルサイズ削減のための高速なメタデータ削除
- 適切な表示に必要な必須情報のみを保持
- メモリ効率的な処理
- 画像が表示可能であることを保証する出力検証

## 安全性

ライブラリはすべての入出力を検証します：
- 有効なJPEG/PNGシグネチャのチェック
- チャンク構造とCRCの検証（PNG）
- 出力画像がデコード可能であることの確認
- 不正な形式の画像の安全な処理

## テストカバレッジ

ライブラリには包括的なテストが含まれています：
- 様々なシナリオをカバーする53のテストケース
- 異なる画像フォーマット、色空間、エッジケースのテスト
- デコーダライブラリを使用した出力画像の検証
- Linux、macOS、Windowsでのテスト実行

## ライセンス

このプロジェクトはMITライセンスの下でライセンスされています - 詳細は[LICENSE-MIT](LICENSE-MIT)ファイルを参照してください。

## 貢献

貢献を歓迎します！お気軽にプルリクエストを送信してください。

## 謝辞

このライブラリは以下の優れたクレートを使用しています：
- [jpeg-decoder](https://crates.io/crates/jpeg-decoder) - JPEG検証用
- [png](https://crates.io/crates/png) - PNG検証用
- [crc32fast](https://crates.io/crates/crc32fast) - CRC計算用