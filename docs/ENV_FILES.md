# 環境ファイルの管理ガイド

このドキュメントでは、プロジェクトの環境ファイル（`.env`ファイル）の管理方法について説明します。

## 📁 環境ファイルの種類

### コミットするファイル（テンプレート）✅

これらのファイルは**Gitにコミットされます**。実際のAPIキーやシークレットは含めないでください。

#### `.env.example`
- **用途**: 環境変数のサンプル・ドキュメント
- **内容**: 全ての環境変数の説明とダミー値
- **使用者**: 全員（新規参加者向けのリファレンス）

#### `.env.dev`
- **用途**: 開発環境用のテンプレート
- **内容**: 開発に適したデフォルト値（空欄）
- **使用者**: 開発者

### コミットしないファイル（実際の設定）❌

これらのファイルは**Gitに絶対にコミットしないでください**。`.gitignore`で無視されます。

#### `.env`
- **用途**: 個人の開発環境設定
- **内容**: 実際のAPIキーとシークレット
- **作成方法**: `.env.example`または`.env.dev`をコピーして値を記入

#### `.env.local`
- **用途**: ローカル専用の設定（個人用のオーバーライド）
- **内容**: 個人固有の設定

#### `.env.prod`
- **用途**: 本番環境の設定
- **内容**: 本番用のAPIキーとシークレット
- **管理**: CI/CDシステムまたはシークレット管理ツールで管理

#### `.env.staging`
- **用途**: ステージング環境の設定
- **内容**: ステージング用のAPIキーとシークレット

## 🚀 セットアップ手順

### 初回セットアップ

```bash
# 1. テンプレートをコピー
cp .env.dev .env

# 2. エディタで開く
vim .env  # または nano、code、notepadなど

# 3. 実際のAPIキーを記入
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/YOUR_REAL_ID/YOUR_REAL_TOKEN
GEMINI_API_KEY=YOUR_REAL_API_KEY
GITHUB_TOKEN=ghp_YOUR_REAL_TOKEN

# 4. 保存して閉じる
```

### 環境変数の追加時

新しい環境変数を追加する場合:

1. **`.env.example`に追加**
   ```bash
   # New API Key for XXX service
   XXX_API_KEY=your_xxx_api_key_here
   ```

2. **`.env.dev`に追加**
   ```bash
   # New API Key for XXX service
   XXX_API_KEY=
   ```

3. **自分の`.env`に実際の値を追加**
   ```bash
   XXX_API_KEY=actual_key_value
   ```

4. **ドキュメントを更新**
   - `README.md`
   - `docs/SETUP_DEV.md`

## 🔒 セキュリティベストプラクティス

### ✅ DO（推奨）

- `.env`ファイルには実際のAPIキーを記入する
- `.env.example`にはダミー値のみを記入する
- 新しい環境変数は必ず`.env.example`に追加する
- APIキーの取得方法をドキュメントに記載する
- 不要になったAPIキーは削除する

### ❌ DON'T（禁止）

- `.env`ファイルをGitにコミットしない
- 実際のAPIキーをSlack/Discord/チャットで共有しない
- 実際のAPIキーをスクリーンショットに含めない
- 実際のAPIキーをGitHub Issueに記載しない
- テストコードにハードコードしない

## 🛡️ 漏洩時の対応

もし誤って`.env`ファイルをコミットしてしまった場合:

1. **即座にAPIキーを無効化**
   - Discord: Webhookを削除して再作成
   - Gemini: APIキーを削除して再生成
   - GitHub: トークンを削除して再生成

2. **Gitの履歴から削除**
   ```bash
   # 警告: 履歴を書き換えるため、チームと調整が必要
   git filter-branch --force --index-filter \
     "git rm --cached --ignore-unmatch .env" \
     --prune-empty --tag-name-filter cat -- --all

   # または BFG Repo-Cleaner を使用（推奨）
   bfg --delete-files .env
   ```

3. **強制プッシュ**
   ```bash
   git push origin --force --all
   ```

4. **チームに通知**
   - 全員に`git pull --force`を依頼
   - 新しいAPIキーを共有（安全な方法で）

## 📋 .gitignoreの設定

現在の`.gitignore`設定:

```gitignore
# Environment variables
# Ignore actual environment files with sensitive data
.env
.env.local
.env.*.local
.env.production
.env.staging
.env.prod

# Allow template/example files to be committed
# These files should NOT contain real API keys or secrets
!.env.example
!.env.dev
```

### パターンの説明

| パターン | 説明 | 例 |
|---------|------|---|
| `.env` | メインの環境ファイル | `.env` |
| `.env.local` | ローカル専用設定 | `.env.local` |
| `.env.*.local` | 任意の環境のローカル設定 | `.env.dev.local`, `.env.prod.local` |
| `.env.production` | 本番環境設定 | `.env.production` |
| `.env.staging` | ステージング環境設定 | `.env.staging` |
| `.env.prod` | 本番環境設定（別名） | `.env.prod` |
| `!.env.example` | 例外: コミット許可 | `.env.example` |
| `!.env.dev` | 例外: コミット許可 | `.env.dev` |

## 🧪 gitignoreの動作確認

以下のコマンドで、gitignoreが正しく動作しているか確認できます:

```bash
# 特定のファイルが無視されるか確認
git check-ignore -v .env

# 期待される出力:
# .gitignore:13:.env    .env

# 複数のファイルを一度に確認
git check-ignore -v .env .env.dev .env.example
```

## 🤝 チーム開発での運用

### 新メンバーの onboarding

1. リポジトリをクローン
2. `.env.dev`を`.env`にコピー
3. チームリーダーに各種APIキーを（安全な方法で）依頼
4. `.env`に値を記入
5. `cargo run --example test_scraper`で動作確認

### APIキーの共有方法

**推奨される方法**:
- 1Password、LastPass、Bitwarden などのパスワードマネージャー
- AWS Secrets Manager、HashiCorp Vault などのシークレット管理サービス
- 社内の secure note システム

**避けるべき方法**:
- Slackの public channel
- メール
- GitHub Issue/PR
- Google Docs（権限管理が適切でない場合）

## 📚 参考リンク

- [The Twelve-Factor App: Config](https://12factor.net/config)
- [OWASP: Password Storage Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html)
- [GitHub: Removing sensitive data](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/removing-sensitive-data-from-a-repository)

## ❓ FAQ

### Q: `.env`ファイルを誤ってコミットしてしまった！

A: 即座に以下の対応を:
1. APIキーを全て無効化
2. コミット履歴から削除（上記「漏洩時の対応」参照）
3. 新しいAPIキーを生成

### Q: `.env.dev`には何を書けばいい？

A: 空欄または開発に適したデフォルト値のみ。実際のAPIキーは書かない。

### Q: 本番環境の設定はどうやって管理する？

A: CI/CDシステム（GitHub Actions、CircleCI等）の環境変数機能、またはシークレット管理サービス（AWS Secrets Manager等）を使用してください。

### Q: チームメンバーとAPIキーを共有するには？

A: パスワードマネージャーや社内のシークレット管理システムを使用してください。チャットやメールは避けてください。

### Q: `.env.example`と`.env.dev`の違いは？

A:
- `.env.example`: 全環境変数の完全なドキュメント（説明とダミー値）
- `.env.dev`: 開発環境用のテンプレート（空欄が多い）

---

**重要**: このドキュメントを読んでも不明な点があれば、チームリーダーに相談してください。
