# Limen v0.1 リリースイシュー管理

最終更新: 2026-07-12 (第2版: R-03/R-05/R-08決定を反映)
対象: リリース(初回配布)に向けた課題の一覧と状態管理。全コードベース調査(2026-07-12)に基づく。

ステータス: `open`(未着手) / `decision`(製品判断待ち) / `fixed`(本ブランチで修正済み) / `deferred`(リリース後対応)

## P0: リリースブロッカー

### R-01 配布経路 `decision`(絞り込み済み)
無料配布のため外部ホスティング(R2/S3等)は不要、GitHub Releasesで配る方針。ただしリポジトリがPRIVATEのままではRelease資産も非公開なので、どこかを公開する必要がある。
- 推奨: リリース専用の公開リポジトリ(例: `limen`)を作り、tauri-actionの `owner`/`repo` 入力+fine-grained PATでそこへ公開する。ソースと履歴(旧名Adbrand/SpaceLane、営業資料 docs/20260314_sales_letter_analysis.md)は非公開のまま。履歴の掃除が不要で最小コスト。
- 代替: 本リポジトリをそのまま公開(履歴の露出を許容するなら最も簡単)。実シークレットの混入は無し(調査済み)。
- 残作業: 公開先リポジトリの作成、workflowへの `owner`/`repo` とPAT追加、LPのDownloadリンク実装(lp/index.html:413, 561)、LPデプロイのCI化。

### R-02 ユーザーデータ消失の構造 `fixed`
space.json / config.json が非アトミック書き込みで、読み込み失敗時にデフォルトで即上書きしていた。「Space名がリセットされる」既知バグの根本原因。
- 修正: tmp書き込み+rename によるアトミック保存(src-tauri/src/persistence.rs)。破損ファイルは `.corrupt-<unixtime>` に退避してから初期化(削除しない)。フロントは「ファイル無し」と「読めない」を区別し、読めない場合は退避成功時のみデフォルトを保存(src/lib/persisted-state.ts, src/hooks/use-space-store.ts)。テスト: Rust 11件+bun 9件追加。

### R-03 デフォルトショートカット `fixed`
`Option+Space`(⌥Space)に統一し、コード(src-tauri/src/config.rs)・README×3・LPをすべて一致させた。選定理由: (1) 片手で最速のコード、(2) 「SpaceキーでSpaceを呼ぶ」という記憶しやすさ、(3) リングUIは⌥押下中に数字ヒントを出す設計のため「⌥を押しっぱなしでSpace→数字」が一連の動作として繋がる、(4) Cmd+K(アプリ内検索の定番)やCtrl+Space(日本語入力ソース切替)のような広範な衝突がない。既知の衝突はChatGPTデスクトップ等の一部ランチャーで、config.jsonで変更可能。デフォルトの妥当性はテストで固定(default_primary_is_a_valid_shortcut)。

### R-04 リリースCIの再実行 `open`
v0.1.0 のリリースは tauri-action@v1 が当時未公開で失敗(現在はv1.0.0公開済みで修正不要)。タグ v0.1.0 は1コミット古い d4a804a を指す。バージョンを 0.1.1 に上げ(package.json / tauri.conf.json / Cargo.toml の3ファイル一致が必須。CIがタグとの一致を検証)、v0.1.1 タグをpushする。R-01〜R-03解決後に実施。

## P1: 初回体験を壊すもの

### R-05 設定画面 `fixed`(実装)
方針転換(config.json手編集はUXとして不可)により、設定画面を実装した。従来はUI本体が最初からスタブ(`<div />`)で一度も機能していなかった。構成:
- グローバルショートカットのレコーダー(録画中はグローバルショートカットをsuspend、Escでキャンセル、「Reset to default」はバックエンドの reset_shortcut_config に委譲しデフォルト値の情報源をRustに一本化)
- Accessibility権限の状態表示(未許可の間は2秒ポーリング)+ Grantボタン
- Mission Controlショートカット有効化の案内(R-06のアプリ内表示)
- Launch at login トグル(既にバイナリに同梱されながら未使用だった autostart プラグインを活用。@tauri-apps/plugin-autostart 追加)
バックエンドは設定コマンド群を復元しつつ、update/resetで重複していたトランザクション処理を change_shortcut_config() に統合。main.tsx はウィンドウラベルでルーティング。テスト: ショートカットパーサ9件+config 3件追加。

実機E2E検証済み(2026-07-12, devビルド): ⌥SpaceのOS登録ログ確認、⌥Space→リング表示、既存space.jsonのSpace名が新永続化コードで無傷読み込み、トレイ→Settings起動、⌥Space表示/Grantedバッジ/autostart状態のIPC実動作、レコーダー往復(⌘⇧K登録→config.json書き換え→再登録→Reset→⌥Space復帰)、Settings表示中はリング呼び出しを抑止。未検証: (1)Accessibility未許可分岐(この環境は許可済みで、TCC削除は本番権限も壊すため実行せず。署名ビルド初回起動が必ず未許可から始まるのでそこで確認) (2)Launch at loginの書き込み(既存LaunchAgentをdevビルドのパスで上書きしてしまうため読み取りのみ確認)。

### R-06 Mission Controlショートカットの前提 `fixed`(文書+アプリ内案内+初回導線+自動検出) / `open`(LP記載)
README×3に「Setup Requirements」節を追加し、設定画面のPermissionsセクションにも有効化手順を常設表示。さらに初回起動時にSettingsを自動オープンするオンボーディングを実装([20260712_onboarding_plan.md](20260712_onboarding_plan.md))したため、初見ユーザーが手順を必ず目にする。

自動検出も実装(2026-07-13)。`com.apple.symbolichotkeys` の symbolic hotkey 118-126("Switch to Desktop 1..9")を CFPreferences 経由で読み、`enabled` かつ注入する Ctrl+数字のまま割り当てられている Desktop だけを「切り替え可能」と判定する(別の組み合わせに再割り当てされた Desktop は、注入した Ctrl+N が届かないため利用不可扱い)。Settings は Accessibility 行と同じ形で状態を出し、9つ全部有効なら `Enabled` バッジ、そうでなければ効かない Space を名指しして Keyboard ペインを開く「Enable...」ボタンを出す。1つも有効でない場合は Space 切り替えが全く機能しないため、起動時に Settings を開く(一部だけ有効な運用は正常系として扱い、毎回開かない)。System Settings の変更は cfprefsd 経由で書かれるので、読む前に `CFPreferencesAppSynchronize` でキャッシュを捨て、常駐中でも 2 秒ポーリングで状態が反映される。

実機検証(2026-07-13, devビルド): 全9有効の実機で `Enabled` バッジ表示、Desktop 5 を一時的に無効化して検出が [1,2,3,4,6,7,8,9] に変わること、常駐プロセスが変更を拾うこと(9→8→9)、`x-apple.systempreferences:com.apple.preference.keyboard?Shortcuts` が Keyboard ペインを開くことを確認。

検討して見送り: Limen 自身が symbolic hotkey 118-126 を書き込み `activateSettings` で再読込させる自動有効化(案内文もリンクも不要になる)。理由は、ユーザーが意図的に別のキーへ再割り当てしている場合に上書きしてしまうこと、および再ログインなしで実際に効くかが未検証であること。案内方式で十分機能しているため現状維持。

残: LPへのセットアップ記載。

### R-07 Accessibility権限の導線 `fixed`
- 修正: 起動時に未許可ならシステムプロンプトを表示(従来は切替失敗後のみ)。システムダイアログはアプリごとに一度しか出ないため、2回目以降の失敗時はシステム設定のAccessibilityペインを直接開く(プロセスあたり1回。src-tauri/src/macos.rs)。失敗時に壊れた設定ウィンドウを開いていた導線は削除(src/app.tsx)。
- 文書面もREADME×3のSetup Requirements節で対応済み。残: 権限付与後にアプリ再起動が必要なケースの案内(必要ならFAQで)。

### R-08 署名+公証 `fixed`(CI側) / `open`(手動作業)
署名する方針で確定。release.yml に署名+公証のenvを追加し、secrets欠落時はビルド前に失敗するfail-fastバリデーションを追加(未署名成果物が黙って出荷されない)。残りの手動作業:
1. Apple Developer Program 加入($99/年)
2. Developer ID Application 証明書を作成し、.p12 をbase64化
3. App用パスワード(appleid.apple.com)を発行
4. リポジトリsecretsに6つ登録: `APPLE_CERTIFICATE`(.p12のbase64) / `APPLE_CERTIFICATE_PASSWORD` / `APPLE_SIGNING_IDENTITY`("Developer ID Application: <名前> (<TeamID>)") / `APPLE_ID` / `APPLE_PASSWORD`(App用パスワード) / `APPLE_TEAM_ID`

## P2: 品質・信頼性(リリース後の早期対応)

### R-09 切替レイテンシ約230ms `fixed`
switch_to_space 内の指紋学習(180ms sleep+ウィンドウ走査)がIPC応答を同期ブロックしていた。ワーカースレッドに移動(src-tauri/src/macos.rs)。キー注入間の50ms sleepはOS要件のため残置。

### R-10 space-changed通知でメインスレッドがブロック `fixed`
通知コールバック(メインスレッド)がウィンドウ走査+アイコンのTIFF→PNG変換+ディスク書き込みを同期実行していた。長時間稼働後の「ホバー可能になるまで遅い」症状の温床。処理を refresh_space_snapshot() に一本化しワーカースレッドで実行(src-tauri/src/lib.rs)。起動時スナップショットの重複コード約40行も削減。

### R-11 Space推定(指紋方式)の制約 `open`
空のSpace同士は指紋が衝突する(トレイの数字が誤る)。マルチディスプレイでは全画面のウィンドウが混ざり推定が成立しない。ウィンドウ移動で学習済み指紋が無効化し、fpマップは削除されないため古いエントリが誤マッチし続ける。方式の限界のため、READMEに制約(シングルディスプレイ推奨・9 Spaceまで・推定は近似)を明記する。

### R-12 トレイメニューが起動時から更新されない `open`
メニュー(Space名一覧と✓マーク)は init_tray() で一度だけ構築され、名前変更・Space切替後も再構築されない(src-tauri/src/tray.rs)。アイコンの数字のみ更新される。名前変更時とspace-changed時にメニューを再構築する。

### R-13 アップデータ未導入 `deferred`
tauri-plugin-updater なし。バグ修正を届ける手段がGitHubの手動再ダウンロードのみ。署名導入(R-08)とセットで検討。

### R-14 CIにRustテストが無い `fixed`
release.yml に `cargo test --manifest-path src-tauri/Cargo.toml` を追加(現在16件)。

### R-15 LPの記載検証 `open`(縮小)
ショートカット表記は ⌥+Space に更新済み。残: 「macOS 12+」の動作確認、Downloadリンク(R-01)、セットアップ手順の追記(R-06)。

### R-16 ビルド生成物がgit管理されている `open`
ルートの `target/rust-analyzer/` (flycheck出力、数千行)がコミットに毎回混入している。ルート .gitignore に `/target/` を追加し、追跡から外す(`git rm -r --cached target`)。

## 修正済みの細目(本ブランチ)

- README×3の設定ファイル保存先の誤記を修正(`~/.config/Limen/` → `~/Library/Application Support/Limen/`)。
- README×3に Setup Requirements 節(Accessibility / Mission Controlショートカット / ネットワーク送信なし)を追加し、リリースビルドの記述を署名+公証に更新。
- オーバーレイ内の旧Cmd+Kハンドラ(死んだ分岐)を削除(src/app.tsx)。
- Cargo.toml の `description = "A Tauri App"` / `authors = ["you"]` を実態に合わせて修正。
- useSpaceStore が返す spaces の in-place sort(state配列の破壊的変更)を修正。
- 永続化スキーマ判定・デフォルト生成・マイグレーション・アイコンパス検証を純関数化(src/lib/persisted-state.ts)しテストを追加。

## 権限まわりの整理(参考)

必要なTCC権限は Accessibility のみ。キーイベント注入(CGEventPost)に必須。CGWindowList はウィンドウ所有者名・PID・レイヤーのみ読むため Screen Recording 権限は不要(ウィンドウタイトル kCGWindowName を読まない実装であること)。グローバルショートカット・トレイ・LaunchAgent自動起動は権限不要。ネットワーク送信は一切なし(CSPのconnect-srcも'self'のみ)。信頼材料としてREADME/LPに明記する価値あり。
