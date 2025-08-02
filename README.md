# rust-image-meta

JPEGとPNGのメタデータを操作するためのRustライブラリです。

## 機能

### JPEG
- メタデータの軽量化（EXIFのオリエンテーション情報は保持）
- コメントの読み書き
- ICCプロファイルの保持

### PNG
- 重要なチャンクのみを保持してファイルサイズを削減
- tEXtチャンクの読み取りと追加

## 使用方法

```rust
use rust_image_meta::{jpeg, png};

// JPEG のメタデータ軽量化
let jpeg_data = std::fs::read("input.jpg")?;
let cleaned_jpeg = jpeg::clean_metadata(&jpeg_data)?;
std::fs::write("output.jpg", cleaned_jpeg)?;

// PNG のチャンク削除
let png_data = std::fs::read("input.png")?;
let cleaned_png = png::clean_chunks(&png_data)?;
std::fs::write("output.png", cleaned_png)?;
```

## テスト

テストを実行する前にキャッシュをクリアして再実行：

```bash
make test
```

その他のコマンド：

```bash
make test-verbose  # 詳細出力でテスト
make test-jpeg     # JPEGテストのみ
make test-png      # PNGテストのみ
make help          # 利用可能なコマンド一覧
```

## 開発

```bash
make fmt           # コードフォーマット
make lint          # Lintチェック
make doc           # ドキュメント生成
make ci            # CI用チェック（fmt-check, lint, test）
```

## ライセンス

MIT License