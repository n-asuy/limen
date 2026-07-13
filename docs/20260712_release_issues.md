# Limen v0.1 リリースイシュー管理

最終更新: 2026-07-13 (第6版: v0.1.1 リリース成功。R-01/R-04/R-08/R-13 を削除)
対象: リリース(初回配布)に向けた課題の一覧と状態管理。
ステータス: `open`(未着手)

v0.1.1 を署名+公証+updater署名付きで公開済み: https://github.com/n-asuy/limen/releases/tag/v0.1.1

注記: 対応済みの項目は本一覧から削除した。削除済み: R-02/R-03/R-05/R-07/R-09/R-10/R-11/R-12/R-14/R-16 と「修正済みの細目」に加え、以下:
- R-01 配布経路: `n-asuy/limen`(public)から GitHub Releases で配布。同一リポジトリへの publish なので PAT 不要、ビルトイン `GITHUB_TOKEN` で完結。
- R-04 リリースCI再実行: バージョン 0.1.1 統一の上、注釈付き v0.1.1 タグを push し CI が署名付き Release を publish 成功(8m48s)。
- R-08 署名+公証: Curino LLC(Team BG439TZ56H)名義で6 secret 登録、CI の署名+公証を実証。鍵の控えは `~/limen-release-keys/`(リポジトリ外)。
- R-13 アップデータ: tauri-plugin-updater 実装+Settings「Software update」+CI で updater アーティファクト署名。`latest.json` が version 0.1.1 を署名付きで返すことを確認。実機の更新往復検証は 0.1.2 リリース後に可能。

## 残タスク(LP関連)

### R-06 Mission Controlショートカットの前提 `open`(LP記載)
文書・アプリ内案内・初回導線・自動検出は実装済み。残: LPへのセットアップ記載。

### R-15 LPの記載検証 `open`
ショートカット表記は ⌥+Space に更新済み。残:
- 「macOS 12+」の動作確認
- Download リンクを v0.1.1 Release 資産(.dmg)に接続
- セットアップ手順の追記(R-06)
- LPデプロイのCI修復(現在 "Deploy landing page" ワークフローが失敗中)
