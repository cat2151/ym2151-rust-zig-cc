# ym2151-rust-zig-cc

Rust + Zig CC を使用したYM2151 FM音源エミュレータのプロジェクト

## Phase 2: 440Hz WAV生成

Nuked-OPMエミュレータを使用して、440Hz（A4音）の3秒間のWAVファイルを生成します。

### ビルドと実行

```bash
# ビルド
cargo build --bin phase2 --release

# 実行
cargo run --bin phase2 --release
```

出力: `output_440hz.wav` (517KB, 3秒, ステレオ 16-bit 44.1kHz)

### 必要な環境

- Rust (2021 edition以降)
- Cコンパイラ:
  - Zig CC (推奨、ZIG_PATH環境変数またはPATHで利用可能な場合)
  - GCC/Clang (Linux/macOS)
  - MSVC (Windows) ※mingwは使用しません

### 技術仕様

- **YM2151エミュレータ**: Nuked-OPM (サイクル精度)
- **サンプルレート**: 44.1 kHz
- **クロック**: 64サイクル/サンプル
- **レジスタ書き込み遅延**: 10ms (約441サンプル)
- **出力形式**: 16-bit ステレオ WAV

### プロジェクト構成

```
src/phase2/
├── main.rs          - メインプログラム（Rust）
├── c/
│   ├── opm.h        - Nuked-OPM ヘッダ
│   └── opm.c        - Nuked-OPM 実装
└── README.md        - 詳細ドキュメント
```

### ライセンス

- Nuked-OPM: LGPL-2.1 (Copyright (C) 2020-2022 Nuke.YKT)
- このプロジェクト: MIT License