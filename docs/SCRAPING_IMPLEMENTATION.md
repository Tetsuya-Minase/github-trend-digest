# GitHub Trendingスクレイピング機能実装ドキュメント

## 概要

このドキュメントは、GitHub Trendingページからトレンドリポジトリ情報をスクレイピングする機能の実装詳細を説明します。

## 実装内容

### 1. TrendingScraperClient構造体

**場所**: `src/infrastructure/scraper.rs`

#### 主な機能

- HTTPクライアント（reqwest）を使用したGitHub Trendingページの取得
- HTMLパーサー（scraper）を使用したコンテンツの解析
- 包括的なエラーハンドリングとロギング

#### 主要メソッド

```rust
pub fn new() -> AppResult<Self>
pub fn with_base_url(base_url: String) -> AppResult<Self>
async fn fetch_html(&self, language: Option<&str>) -> AppResult<String>
fn parse_trending_page(&self, html: &str) -> AppResult<Vec<Repository>>
```

### 2. CSSセレクタ戦略

GitHubのHTML構造変更に対応するため、プライマリとフォールバックの2段階セレクタを実装：

| 要素 | プライマリセレクタ | フォールバックセレクタ |
|-----|-----------------|-------------------|
| リポジトリ記事 | `article.Box-row` | - |
| リポジトリ名/URL | `h2 a` | `h1.h3 a` |
| 説明 | `p.col-9` | `p` |
| プログラミング言語 | `span[itemprop='programmingLanguage']` | `span.d-inline-block.ml-0.mr-3` |
| 本日のスター数 | `span.d-inline-block.float-sm-right` | `span.float-sm-right` |

### 3. エラーハンドリング

以下のエラーケースを適切に処理：

- **ネットワークエラー**: タイムアウト（30秒）、接続失敗
- **HTTPステータスエラー**: 404, 429（Rate Limit）など
- **HTMLパースエラー**: 要素が見つからない、構造が想定と異なる
- **バリデーションエラー**: Repository作成時のバリデーション失敗

### 4. ロギング

トレーシングレベル別のログ記録：

- **INFO**: スクレイピング開始/完了、取得件数
- **DEBUG**: パース中の詳細情報（リポジトリ情報など）
- **WARN**: フォールバックセレクタの使用、一部データの欠損、パース失敗
- **ERROR**: スクレイピング失敗、ネットワークエラー

### 5. レート制限対策

- **User-Agent設定**: 適切な識別情報を含むUser-Agent
  ```
  Mozilla/5.0 (compatible; GithubTrendingBot/1.0; +https://github.com/yourusername/github-trending-summary)
  ```
- **タイムアウト設定**: 30秒
- **将来的な拡張**: Exponential backoffによるリトライ機構の追加が可能

## テスト実装

### 単体テスト

**場所**: `src/infrastructure/scraper.rs` の `#[cfg(test)]` モジュール

#### テストケース

1. `test_parse_trending_page_success`: 正常系のHTMLパース
2. `test_parse_trending_page_missing_description`: 説明がない場合の処理
3. `test_parse_trending_page_invalid_html`: 不正なHTMLの処理
4. `test_parse_trending_page_empty_html`: 空のHTMLの処理
5. `test_client_creation`: クライアント作成
6. `test_client_with_custom_base_url`: カスタムベースURLでのクライアント作成

### 統合テスト

**場所**: `tests/scraping_integration_test.rs`

**使用ツール**: httpmock（モックHTTPサーバー）

#### テストケース

1. `test_fetch_trending_with_mock_server`: 正常なスクレイピング
2. `test_fetch_trending_with_language_filter`: 言語フィルタ付きリクエスト
3. `test_fetch_trending_http_404_error`: 404エラーのハンドリング
4. `test_fetch_trending_http_429_rate_limit`: Rate Limitエラーのハンドリング
5. `test_fetch_trending_empty_response`: 空のレスポンスの処理
6. `test_fetch_trending_malformed_html`: 不正なHTMLの処理
7. `test_fetch_trending_timeout`: タイムアウトの検証
8. `test_fetch_trending_no_language_field`: 言語情報がない場合の処理

### テスト結果

```
✓ 単体テスト: 6件全て成功
✓ 統合テスト: 8件全て成功
✓ 全体: 44件全て成功
```

## セキュリティ考慮事項

1. **User-Agent設定**: 正当な識別情報を含み、GitHubの利用規約に準拠
2. **データサニタイゼーション**: Repositoryエンティティのバリデーションにより安全性を確保
3. **機密情報の保護**: ログに個人情報や機密情報を含めない設計
4. **Rate Limit対策**: タイムアウト設定とユーザーエージェント識別による適切なアクセス

## 既知の制約事項

### 1. HTMLセレクタの脆弱性

**リスク**: GitHubがHTML構造を変更すると、パースが失敗する可能性があります。

**対策**:
- プライマリ/フォールバックの2段階セレクタ戦略
- パース失敗時の警告ログ出力
- 一部リポジトリのパース失敗時でも処理を継続

### 2. 本日のスター数取得の信頼性

**制約**: GitHubのHTML構造が頻繁に変更されるため、本日のスター数の取得は信頼性が低い可能性があります。

**現状の実装**:
- ベストエフォートで抽出を試みる
- 抽出失敗時は0を返す
- 将来的にはGitHub APIからの取得を検討

### 3. Rate Limit

**制約**: GitHubのスクレイピングには暗黙的なRate Limitがあります。

**現状の対策**:
- タイムアウト設定（30秒）
- 適切なUser-Agent設定

**将来の改善**:
- Exponential backoffによるリトライ機構
- GitHub API併用による負荷分散

## パフォーマンス

### 期待されるパフォーマンス

- **通常時**: 1-3秒以内にレスポンス
- **タイムアウト**: 30秒でタイムアウト

### 最適化の可能性

1. HTTPクライアントの接続プーリング（reqwestでデフォルト有効）
2. 複数言語の並列取得
3. キャッシング機構の追加

## 使用例

```rust
use github_trending_summary::infrastructure::scraper::TrendingScraperClient;
use github_trending_summary::domain::repositories::TrendingRepository;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // クライアント作成
    let client = TrendingScraperClient::new()?;

    // トレンドリポジトリ取得
    let repos = client.fetch_trending(None).await?;

    // 結果表示
    for repo in repos {
        println!("{}: {}", repo.full_name(), repo.description().unwrap_or("No description"));
    }

    Ok(())
}
```

### 言語フィルタ付き

```rust
// Rust言語のトレンドリポジトリを取得
let rust_repos = client.fetch_trending(Some("rust".to_string())).await?;
```

## 今後の改善案

1. **リトライ機構の実装**
   - Exponential backoffによる自動リトライ
   - Rate Limit時の待機処理

2. **GitHub API連携**
   - より信頼性の高いスター数取得
   - 追加のメタデータ取得（コミット数、コントリビューター数など）

3. **キャッシング**
   - 一定時間内の重複リクエスト削減
   - メモリまたはRedisベースのキャッシュ

4. **監視とメトリクス**
   - スクレイピング成功率の追跡
   - パース失敗率の監視
   - レスポンスタイムの記録

5. **HTML構造の自動検出**
   - 機械学習を使用したセレクタの自動調整
   - HTML構造変更の自動検出とアラート

## 依存関係

- **reqwest** (0.12): HTTPクライアント、rustls-tlsを使用
- **scraper** (0.19): HTMLパーサー
- **async-trait** (0.1): 非同期トレイトサポート
- **tracing**: ログ記録
- **httpmock** (0.8): 統合テスト用モックサーバー

## 参考資料

- [scraper crate documentation](https://docs.rs/scraper/latest/scraper/)
- [reqwest documentation](https://docs.rs/reqwest/latest/reqwest/)
- [GitHub Trending Page](https://github.com/trending)
- [Web Scraping Best Practices](https://www.scrapingbee.com/blog/web-scraping-best-practices/)

## 変更履歴

| 日付 | バージョン | 変更内容 |
|-----|----------|---------|
| 2025-10-10 | 1.0.0 | 初回実装完了 |
