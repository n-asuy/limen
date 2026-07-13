# Limen v0.1 リリースイシュー管理

最終更新: 2026-07-13 (第5版: R-13 アップデータをコード実装、残りは手動secret登録のみ)
対象: リリース(初回配布)に向けた課題の一覧と状態管理。
ステータス: `open`(未着手) / `decision`(製品判断待ち) / `deferred`(リリース後対応)

注記: 対応済みの項目(R-02/R-03/R-05/R-07/R-09/R-10/R-11/R-12/R-14/R-16 と「修正済みの細目」)は本一覧から削除した。以下は未着手・判断待ち・リリース後対応のみ。
- R-11 Space推定の制約: READMEの Space Detection 節に制約(シングルディスプレイ推奨・9 Spaceまで・空Spaceは曖昧)を明記。
- R-12 トレイメニュー未更新: メニュー構築を build_tray_menu() に切り出し、リネーム(save_state_file)と space-changed(refresh_space_snapshot)で rebuild_menu() により再構築(src-tauri/src/tray.rs, lib.rs)。

## P0: リリースブロッカー

### R-01 配布経路 `decision`(絞り込み済み)
無料配布のため外部ホスティング(R2/S3等)は不要、GitHub Releasesで配る方針。ただしリポジトリがPRIVATEのままではRelease資産も非公開なので、どこかを公開する必要がある。
- 推奨: リリース専用の公開リポジトリ(例: `limen`)を作り、tauri-actionの `owner`/`repo` 入力+fine-grained PATでそこへ公開する。ソースと履歴は非公開のまま。履歴の掃除が不要で最小コスト。
- 代替: 本リポジトリをそのまま公開(履歴の露出を許容するなら最も簡単)。実シークレットの混入は無し(調査済み)。
- 残作業: 公開先リポジトリの作成、workflowへの `owner`/`repo` とPAT追加、LPのDownloadリンク実装、LPデプロイのCI化。

### R-04 リリースCIの再実行 `open`
v0.1.0 のリリースは tauri-action@v1 が当時未公開で失敗(現在はv1.0.0公開済みで修正不要)。タグ v0.1.0 は1コミット古い d4a804a を指す。バージョンを 0.1.1 に上げ(package.json / tauri.conf.json / Cargo.toml の3ファイル一致が必須。CIがタグとの一致を検証)、v0.1.1 タグをpushする。R-01・R-08(手動作業)解決後に実施。

## P1: 初回体験を壊すもの

### R-06 Mission Controlショートカットの前提 `open`(LP記載)
文書・アプリ内案内・初回導線・自動検出は実装済み。残: LPへのセットアップ記載。

### R-08 署名+公証 `open`(手動作業)
CI側(release.ymlの署名+公証env、secrets欠落時のfail-fastバリデーション)は実装済み。残りの手動作業:
1. Apple Developer Program 加入($99/年)
2. Developer ID Application 証明書を作成し、.p12 をbase64化
3. App用パスワード(appleid.apple.com)を発行
4. リポジトリsecretsに6つ登録: `APPLE_CERTIFICATE`(.p12のbase64) / `APPLE_CERTIFICATE_PASSWORD` / `APPLE_SIGNING_IDENTITY`("Developer ID Application: <名前> (<TeamID>)") / `APPLE_ID` / `APPLE_PASSWORD`(App用パスワード) / `APPLE_TEAM_ID`

## P2: 品質・信頼性(リリース後の早期対応)

### R-13 アップデータ `open`(手動作業のみ)
tauri-plugin-updater を導入し、アプリ内更新を実装した。
- 実装: プラグイン登録(lib.rs)、`plugins.updater`(endpoints + pubkey)と `bundle.createUpdaterArtifacts: true`(tauri.conf.json)、Rust コマンド `check_for_update`/`install_update`(src-tauri/src/updater.rs, ネットワークはここだけ・ユーザー操作時のみ)、Settings に「Software update」セクション(バージョン表示・Check for updates・Install & restart)、capability に `updater:default`。READMEに Software Updates 節を追記しプライバシー記述を「更新チェック時のみGitHubへ接続」に修正。
- CI: release.yml のビルド step に `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` を追加し、fail-fast バリデーションにも秘密鍵を追加(欠落時はビルド前に失敗)。
- 更新署名鍵は生成済み。公開鍵は tauri.conf.json に埋め込み、秘密鍵/空パスワードは `n-asuy/limen` の secrets(`TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`)に登録済み(2026-07-13)。endpoint は `https://github.com/n-asuy/limen/releases/latest/download/latest.json`(latest.json は tauri-action が Release 資産として生成)。
- 依存: 署名導入(R-08)が前提。実機の更新往復検証はR-04で最初の署名付きReleaseを2つ以上作った後に可能。

### R-15 LPの記載検証 `open`(縮小)
ショートカット表記は ⌥+Space に更新済み。残: 「macOS 12+」の動作確認、Downloadリンク(R-01)、セットアップ手順の追記(R-06)。
