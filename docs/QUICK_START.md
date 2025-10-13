# クイックスタートガイド

このガイドでは、最速でGitHub Trending Summaryを動作確認する手順を説明します。

## 📋 必要なもの

1. **Discord Webhook URL**
2. **Gemini API Key**
3. GitHub Token（オプション、推奨）

## 🚀 5分でセットアップ

### 1. APIキーの取得

#### Discord Webhook URL（2分）
1. Discordサーバーで右クリック → 「サーバー設定」
2. 「連携サービス」→「ウェブフック」
3. 「新しいウェブフック」をクリック
4. ウェブフックURLをコピー

#### Gemini API Key（2分）
1. https://aistudio.google.com/app/apikey にアクセス
2. 「Create API Key」をクリック
3. APIキーをコピー

#### GitHub Token（1分、オプション）
1. https://github.com/settings/tokens にアクセス
2. 「Generate new token (classic)」
3. `public_repo` にチェック
4. トークンを生成してコピー

### 2. 環境変数の設定

```bash
# プロジェクトディレクトリに移動
cd github-trending-summary

# .envファイルを作成
cp .env.example .env

# エディタで.envファイルを開く
nano .env
# または
code .env
# または
vim .env
```

`.env`ファイルに以下を記入:

```bash
# 必須: あなたのDiscord Webhook URLを貼り付け
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/YOUR_WEBHOOK_ID/YOUR_WEBHOOK_TOKEN

# 必須: あなたのGemini API Keyを貼り付け
GEMINI_API_KEY=YOUR_GEMINI_API_KEY_HERE

# オプション: GitHubトークン（推奨）
GITHUB_TOKEN=ghp_YOUR_GITHUB_TOKEN_HERE

# オプション: 言語フィルタ（例: rust, python, javascript）
TARGET_LANGUAGE=

# オプション: ログレベル
LOG_LEVEL=info
```

保存して閉じる（nanoの場合: `Ctrl+X` → `Y` → `Enter`）

### 3. 動作確認

```bash
# スクレイパーのテスト（全言語）
cargo run --example test_scraper

# 特定の言語でテスト
cargo run --example test_scraper -- rust
```

成功すると、以下のような出力が表示されます:

```
==============================================
  GitHub Trending Scraper Test
==============================================
Language filter: All languages

Creating scraper client...
Fetching trending repositories...
Successfully fetched 25 repositories!

==============================================
  Trending Repositories
==============================================

1. rust-lang/rust
   URL: https://github.com/rust-lang/rust
   Description: Empowering everyone to build reliable and efficient software.
   Language: Rust
   Stars today: 123

...
```

## ✅ チェックリスト

動作確認が完了したら、以下を確認してください:

- [ ] `.env`ファイルが作成されている
- [ ] `DISCORD_WEBHOOK_URL`が正しく設定されている
- [ ] `GEMINI_API_KEY`が正しく設定されている
- [ ] `cargo run --example test_scraper`が成功する
- [ ] トレンドリポジトリのリストが表示される

## 🎉 次のステップ

動作確認が完了したら、以下のドキュメントを参照してください:

- **詳細なセットアップ**: [docs/SETUP_DEV.md](SETUP_DEV.md)
- **スクレイパー実装詳細**: [docs/SCRAPING_IMPLEMENTATION.md](SCRAPING_IMPLEMENTATION.md)

## ❓ トラブルシューティング

### エラー: "DISCORD_WEBHOOK_URL environment variable is required"

**原因**: 環境変数が読み込まれていない

**解決策**:
1. `.env`ファイルが`github-trending-summary`ディレクトリに存在するか確認
2. ファイル名が`.env`（`.env.example`ではない）であることを確認
3. `DISCORD_WEBHOOK_URL=`の後に値が記入されているか確認

### エラー: "Failed to fetch trending page"

**原因**: ネットワーク接続またはGitHubへのアクセス問題

**解決策**:
1. インターネット接続を確認
2. GitHubにアクセスできるか確認: https://github.com/trending
3. プロキシ設定を確認（企業ネットワークの場合）

### 警告が表示される

**正常です**: 一部の警告は無視して問題ありません

**確認方法**:
```bash
# ログレベルをinfoに設定
LOG_LEVEL=info cargo run --example test_scraper
```

## 📚 使用可能なコマンド

```bash
# テスト実行
cargo test

# スクレイパーテスト（全言語）
cargo run --example test_scraper

# 言語フィルタ付きテスト
cargo run --example test_scraper -- rust
cargo run --example test_scraper -- python
cargo run --example test_scraper -- javascript
cargo run --example test_scraper -- go
cargo run --example test_scraper -- typescript

# デバッグログ付き実行
LOG_LEVEL=debug cargo run --example test_scraper

# リリースビルド
cargo build --release

# コードフォーマット
cargo fmt

# Lintチェック
cargo clippy
```

## 💡 Tips

### VSCodeを使用している場合

1. `rust-analyzer`拡張機能をインストール
2. `.env`ファイルの編集に`DotENV`拡張機能が便利です

### 複数の環境を管理する場合

```bash
# 開発環境
cp .env.dev .env

# 本番環境
cp .env.prod .env
```

### GitHubトークンの取得を忘れた場合

トークンなしでも動作しますが、以下の制限があります:
- APIレート制限が厳しい（1時間60リクエスト → 5000リクエスト）
- プライベートリポジトリにアクセスできない

後から追加することもできます。

## 🔒 セキュリティ注意事項

- `.env`ファイルは**絶対にGitにコミットしないでください**
- `.gitignore`に`.env`が含まれていることを確認してください
- APIキーやトークンは他人と共有しないでください
- 不要になったトークンは削除してください

## 📞 サポート

問題が解決しない場合は、以下の情報を含めてIssueを作成してください:

1. エラーメッセージ全文
2. 実行したコマンド
3. 環境情報:
   ```bash
   rustc --version
   cargo --version
   uname -a  # macOS/Linux
   ```
4. `.env`ファイルの内容（**APIキーは伏せ字にしてください**）

---

**Happy Coding! 🎉**
