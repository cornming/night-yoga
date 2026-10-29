# 晚安瑜伽 night-yoga

睡前 5 到 15 分鐘的溫和伸展與呼吸引導網頁。使用 Rust + [Leptos](https://leptos.dev)（CSR）編譯成 WebAssembly，以 [Trunk](https://trunkrs.dev) 打包。

## 功能

- 三組睡前課程（約 5／10／15 分鐘），十個溫和體式：人形示範圖（貓牛式會跟著呼吸動）、步驟、注意事項、左右側提示
- 呼吸引導：4-7-8、延長吐氣、方塊呼吸，圓圈跟著吸吐縮放
- 語音提示（瀏覽器內建語音合成，台灣華語）、換式鈴聲
- 背景音樂：水晶鋼琴、頌缽、溫暖長音、雨聲、海浪，全部用 Web Audio 即時合成（不需音檔、離線可用、無版權問題），可試聽、調音量，練習中也能切換
- 暫停／上一個／下一個，練習中螢幕常亮（Screen Wake Lock）
- 練習紀錄：連續天數、練習天數、累計分鐘、五週打卡格（存在 localStorage）
- 夜間配色、亮度調整、PWA（加到主畫面、離線可用）

## 設計參考

| 參考專案 | 借鏡的部分 |
|---|---|
| [meditohq/medito-app](https://github.com/meditohq/medito-app) | 免費、免註冊、無廣告 |
| [heylinda/heylinda-app](https://github.com/heylinda/heylinda-app) | 連續天數、行事曆、累計時間、深色模式、離線 |
| [mmazzarolo/breathly-app](https://github.com/mmazzarolo/breathly-app) | 呼吸動畫＋語音引導 |
| [sepandhaghighi/nafas](https://github.com/sepandhaghighi/nafas) | 以 JSON 定義呼吸程式（pre／cycles／ratio） |
| [cris-maillo/yogAI](https://github.com/cris-maillo/yogAI) | 瀏覽器內姿勢辨識（規劃中的第二階段） |

只參考設計概念，沒有使用上述專案的程式碼或素材。

## 修改內容

課程、體式、呼吸程式都在 `data/` 的 JSON 檔，改完重新編譯即可：

- `data/poses.json`：體式名稱、語音提示、步驟、注意事項、示範圖骨架（`figure`）
  - 示範圖用關節角度描述（0° 朝右、90° 朝下），程式依人體比例畫出人形；`view` 可選 `side`／`front`／`top`
  - 手腳可以給角度 `[上臂, 前臂]`，或給要碰到的點 `{ "to": [x, y], "bend": 1 }` 自動算關節
  - 給兩個影格會來回播放成動畫（`period` 秒）
  - `PREVIEW=1 cargo test preview` 會把所有示範圖輸出到 `target/figures.html`
- `data/breathing.json`：`pre` 準備秒數、`cycles` 輪數、`ratio` 吸／停／吐／停秒數
- `data/routines.json`：課程組合，`{ "pose": "twist", "seconds": 45, "side": "left" }` 或 `{ "breath": "478" }`

## 本機開發

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
trunk serve --open        # http://127.0.0.1:8080
cargo test                # 資料格式與呼吸階段計算的測試
```

## 部署到 GitHub Pages

1. 建立 GitHub repo，把專案推到 `main` 分支
2. **Settings → Pages → Build and deployment → Source** 選 **GitHub Actions**
3. **Settings → Actions → General → Workflow permissions** 勾選 **Allow GitHub Actions to create and approve pull requests**（自動發版需要）

之後每次推到 `main`，`deploy.yml` 會測試、編譯並部署到 `https://<帳號>.github.io/<repo>/`。

## 版號與更新紀錄

`release.yml` 使用 release-please：commit 訊息用 `feat:`、`fix:` 開頭，推到 `main` 後會自動開一個發版 PR，合併後自動更新 `Cargo.toml` 版號、寫入 `CHANGELOG.md`、建立 GitHub Release。頁面底部會顯示目前版號。

## 安全提醒

內容僅為一般放鬆伸展，不能取代醫療建議。動作感到疼痛請立即停止；有傷病或懷孕請先諮詢醫師。
