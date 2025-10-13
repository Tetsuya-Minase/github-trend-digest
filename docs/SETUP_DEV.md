# 開発環境セットアップガイド

このドキュメントでは、GitHub Trending Summaryの開発環境をセットアップする方法を説明します。

## 前提条件

- Rust 1.70以上がインストールされていること
- GitHubアカウント
- Discordサーバー（通知テスト用）
- Google Cloudアカウント（Gemini API使用）

## 必要なAPIキーとトークン

### 1. Discord Webhook URL（必須）

1. Discordサーバーを用意
2. サーバー設定 > 連携サービス > ウェブフック
3. 「新しいウェブフック」をクリック
4. ウェブフックURLをコピー
   - 形式: `https://discord.com/api/webhooks/{webhook_id}/{webhook_token}`

### 2. Gemini API Key（必須）

1. [Google AI Studio](https://aistudio.google.com/app/apikey)にアクセス
2. 「Create API Key」をクリック
3. プロジェクトを選択（または新規作成）
4. 生成されたAPIキーをコピー

### 3. GitHub Personal Access Token（オプション、推奨）

1. [GitHub Settings > Tokens](https://github.com/settings/tokens)にアクセス
2. 「Generate new token (classic)」をクリック
3. 必要なスコープを選択:
   - `public_repo`（パブリックリポジトリへのアクセス）
   - または `repo`（プライベートリポジトリも含む）
4. トークンを生成してコピー
   - 形式: `ghp_...`

**注意**: GitHubトークンがない場合でも動作しますが、APIレート制限が厳しくなります。

## セットアップ手順

### 1. リポジトリのクローン

```bash
git clone https://github.com/yourusername/github-trending-summary.git
cd github-trending-summary
```

### 2. 環境変数の設定

**注意**: `.env`ファイルは個人用の設定ファイルです。実際のAPIキーを含むため、Gitにコミットしないでください。（`.gitignore`で自動的に無視されます）

📖 **環境ファイルの詳細**: [docs/ENV_FILES.md](ENV_FILES.md)

#### 方法A: .env.devファイルを使用（推奨）

```bash
# .env.devファイルを作成（既に用意されています）
cp .env.dev .env

# .envファイルを編集して、取得したAPIキーとトークンを記入
vim .env  # または nano .env、code .env など
```

`.env`ファイルの内容:

```bash
# Discord Webhook URL（必須）
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/YOUR_WEBHOOK_ID/YOUR_WEBHOOK_TOKEN

# Gemini API Key（必須）
GEMINI_API_KEY=YOUR_GEMINI_API_KEY_HERE

# GitHub Personal Access Token（オプション）
GITHUB_TOKEN=ghp_YOUR_GITHUB_TOKEN_HERE

# Target language（オプション）
# 例: rust, python, javascript, go, typescript
TARGET_LANGUAGE=

# Log level（オプション、デフォルト: info）
LOG_LEVEL=debug
```

#### 方法B: 環境変数を直接エクスポート

```bash
export DISCORD_WEBHOOK_URL="https://discord.com/api/webhooks/..."
export GEMINI_API_KEY="your_gemini_api_key"
export GITHUB_TOKEN="ghp_your_github_token"  # オプション
export TARGET_LANGUAGE="rust"  # オプション
export LOG_LEVEL="debug"  # オプション
```

### 3. 依存関係のインストール

```bash
cargo build
```

### 4. テストの実行

```bash
# 全てのテストを実行
cargo test

# 特定のテストを実行
cargo test --lib infrastructure::scraper
cargo test --test scraping_integration_test
```

## 動作確認

### 1. スクレイパーのテスト

GitHub Trendingページのスクレイピング機能をテストします:

```bash
# 全言語のトレンドリポジトリを取得
cargo run --example test_scraper

# 特定の言語でフィルタ
cargo run --example test_scraper -- rust
cargo run --example test_scraper -- python
cargo run --example test_scraper -- javascript
```

期待される出力:

```
==============================================
  GitHub Trending Scraper Test
==============================================
Language filter: rust

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

2. serde-rs/serde
   URL: https://github.com/serde-rs/serde
   Description: Serialization framework for Rust
   Language: Rust
   Stars today: 89

...
```

### 2. アプリケーション全体のテスト（今後実装）

```bash
# メインアプリケーションの実行
cargo run
```

## トラブルシューティング

### 問題: "DISCORD_WEBHOOK_URL environment variable is required"

**原因**: Discord Webhook URLが設定されていない

**解決策**:
1. `.env`ファイルが存在し、正しく編集されているか確認
2. `DISCORD_WEBHOOK_URL`の値が正しい形式か確認
   - 正: `https://discord.com/api/webhooks/...`
   - 正: `https://discordapp.com/api/webhooks/...`
   - 誤: `https://discord.com/webhooks/...`（`/api/`が抜けている）

### 問題: "GEMINI_API_KEY environment variable is required"

**原因**: Gemini API Keyが設定されていない

**解決策**:
1. [Google AI Studio](https://aistudio.google.com/app/apikey)でAPIキーを取得
2. `.env`ファイルに`GEMINI_API_KEY=`の行を追加

### 問題: スクレイピングが失敗する

**原因**:
- ネットワーク接続の問題
- GitHubのHTML構造が変更された
- Rate Limitに達した

**解決策**:
1. インターネット接続を確認
2. ログレベルを`debug`に設定して詳細を確認:
   ```bash
   LOG_LEVEL=debug cargo run --example test_scraper
   ```
3. しばらく待ってから再試行（Rate Limit対策）

### 問題: コンパイルエラー

**原因**: Rustのバージョンが古い

**解決策**:
```bash
# Rustを最新版にアップデート
rustup update stable

# バージョン確認
rustc --version  # 1.70以上が必要
```

## 開発ワークフロー

### コードの変更後

```bash
# フォーマット
cargo fmt

# Lintチェック
cargo clippy -- -D warnings

# テスト実行
cargo test

# ビルド確認
cargo build
```

### 新機能の追加

1. 機能ブランチを作成
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. コードを実装

3. テストを追加・実行

4. コミット
   ```bash
   git add .
   git commit -m "feat: add your feature description"
   ```

5. プッシュしてプルリクエスト作成

## 参考リンク

- [Rust公式ドキュメント](https://doc.rust-lang.org/)
- [Google AI Studio](https://aistudio.google.com/)
- [Discord Developer Portal](https://discord.com/developers/docs/intro)
- [GitHub API Documentation](https://docs.github.com/en/rest)

## サポート

問題が解決しない場合は、以下の情報を含めてIssueを作成してください:

1. エラーメッセージ
2. 実行したコマンド
3. 環境情報（OS、Rustバージョンなど）
4. ログ出力（`LOG_LEVEL=debug`で実行）

```bash
# 環境情報の確認
rustc --version
cargo --version
uname -a  # Linux/macOS
# または
systeminfo  # Windows
```
