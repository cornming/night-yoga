# 晚安瑜伽 night-yoga

睡前 5 到 15 分鐘的溫和伸展與呼吸引導網頁。使用 Rust + [Leptos](https://leptos.dev)（CSR）編譯成 WebAssembly，以 [Trunk](https://trunkrs.dev) 打包。

## 功能

- 三組睡前課程（約 5／10／15 分鐘），十個溫和體式：人形示範圖（貓牛式會跟著呼吸動）、步驟、注意事項、左右側提示
- 呼吸引導：4-7-8、延長吐氣、方塊呼吸，圓圈跟著吸吐縮放
- 動作中的引導（設定裡可關）：開頭念完口訣後不再靜默。動作進行到 15 秒起，約每 25 秒輕聲提醒一句（該動作的注意事項、步驟與簡短的放鬆提示輪流，左右兩側不重複）；左側做完前 6 秒說「準備換邊」；每個動作最後 3 秒倒數「三、二、一」（不到 15 秒的動作不倒數）；呼吸練習從第二輪起報「第二輪」，最後一輪說「最後一輪」。提醒的文字同時顯示在畫面上，關掉語音也看得到。內容全部來自 `data/poses.json`，結尾前 10 秒不說提醒，留給換邊和倒數
- 語音提示、換式鈴聲：優先播放預錄的自然語音（`public/voice/*.mp3`，可用 Google AI Studio 產生），沒有檔案的句子退回瀏覽器內建語音合成（台灣華語）
- 背景音樂：水晶鋼琴、頌缽、溫暖長音、雨聲、海浪，全部用 Web Audio 即時合成（不需音檔、離線可用、無版權問題），可試聽、調音量，練習中也能切換
- 暫停／上一個／下一個，練習中螢幕常亮（Screen Wake Lock）
- 練習紀錄：連續天數、練習天數、累計分鐘、五週打卡格（存在 localStorage）
- 夜間配色、亮度調整、PWA（加到主畫面、離線可用）
- 從 LINE 內建瀏覽器開啟時，提醒改用預設瀏覽器（見下方「從 LINE 點開的連結」）

## 匿名完成統計

整組課程走完時，網頁會匿名送出一筆紀錄：**課程代號（quick／standard／deep）、完成當下的小時（手機的當地時間 0 到 23）、這個瀏覽器第幾次完成的分級（1、2、3、4+）**，伺服器端另外記下時間。沒有帳號、沒有裝置編號、不存 IP，也不使用 cookie。大家的統計公開在 <https://cornming.github.io/stats/>（累計完成次數、一天中的使用時段、課程排行榜、有多少人回來再做）。

- 只有「整組課程走完，而且實際練習時間至少是課程總長的一半」才算一次完成；連按「下一個」跳到最後、中途離開、單獨的呼吸練習都不算。
- 收集端是一份 Google 試算表加 Apps Script，程式碼與部署步驟在 [cornming.github.io 的 `apps-script/`](https://github.com/cornming/cornming.github.io/tree/main/apps-script)。網址寫在 `src/web.rs` 的 `STATS_URL`。
- 「第幾次完成」的次數只存在使用者自己手機的瀏覽器裡（`night-yoga:completions`），送出去的只有分級，不是確切次數。每個瀏覽器的第 1 次只會出現一次，所以統計頁用「第 2 次 ÷ 第 1 次」算回訪率。換手機、清除瀏覽資料、或瀏覽器擋住儲存空間，都會被當成新的人，所以回訪率通常會偏低。
- 收集網址是公開的、沒有登入驗證，所以數字只是約略值，不是嚴謹的使用人數。

## 讓人願意回來的小功能

- **首頁一鍵開始：** 最上面是「今晚繼續：上次做的那組」，第一次來則推最短的課程，不用在睡前再做選擇。上次的課程代號存在這支手機（`night-yoga:last`）。
- **歡迎回來：** 隔了 3 天以上沒練，首頁會說一句「歡迎回來。今晚先做最短的就好。」，不責備。
- **連續天數里程碑：** 剛好連續 3、7、14、30 天的那一天，完成畫面說一句鼓勵，其他日子不說。
- **每天提醒：** 完成畫面可以選一個時間，下載每天重複的行事曆提醒（.ics），或用 Google 日曆加入。提醒由手機的行事曆負責，網站不會記錄你選的時間。純邏輯在 `src/comeback.rs`，有單元測試。

## 從 LINE 點開的連結

LINE 內建的瀏覽器常常播不出語音。網頁用 userAgent 裡的 `Line/<版本>` 辨識是不是從 LINE 開的（`src/inapp.rs`，有單元測試），是的話首頁最上面會出現提醒，提供三個出口：

- **用預設瀏覽器開啟：** 把目前網址加上 `?openExternalBrowser=1` 再前往，這是 LINE 的網址參數，LINE 內建瀏覽器載入帶這個參數的網址時，會改交給手機的預設瀏覽器開啟。
- **複製連結：** 複製後貼到 Safari 或 Chrome。瀏覽器不讓程式寫剪貼簿時，會改顯示網址請使用者長按複製。
- **自己找選單：** 提醒下方也說明可點 LINE 畫面角落的選單，選擇用瀏覽器開啟。

不想等使用者點開才處理，**推廣文裡的連結可以直接寫成** `https://cornming.github.io/night-yoga/?openExternalBrowser=1`，使用者從 LINE 對話點開，就會直接進預設瀏覽器。

提醒只在 LINE 內顯示，一般瀏覽器完全不會看到；按「先這樣用」可以收起來（重新開啟頁面會再出現）。

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

## 預錄語音（自然語音）

瀏覽器內建語音聽起來比較機械。App 會先找 `public/voice/<id>.mp3`，有就播檔案，沒有（或播不出來）才用內建語音念，所以可以一句一句慢慢換。

- `tools/voice/script.csv` 是所有會念的話（目前 59 句，id 對應檔名），由 `src/voice.rs` 的 `catalog()` 決定（動作中引導的句子來自 `src/coach.rs`，節奏常數也在那裡）；`cargo test` 會檢查兩邊一致。59 句都已有預錄語音（開頭口訣、呼吸口令、結語 17 句，加上提醒、倒數、換邊、呼吸輪次 42 句）；之後新增句子時，執行下面的 `generate.py` 只會補錄缺的檔案。改了 `data/` 裡的口訣或課程後，執行 `WRITE_SCRIPT=1 cargo test write_script` 重新產生這個檔案。
- 產生語音檔：到 <https://aistudio.google.com> 的 Get API key 申請金鑰（不要放進程式或 repo），並安裝 ffmpeg，然後在 repo 根目錄執行

  ```bash
  export GEMINI_API_KEY=你的金鑰
  python3 tools/voice/generate.py
  ```

  已經有的檔案會略過，`--force` 重做，`--only phase_inhale end` 只做指定幾句，`--voice Achernar` 換聲音（Zephyr 明亮、Achernar／Sulafat／Vindemiatrix 較柔和）。預設聲音是 Sulafat，和已經錄好的 17 句一樣，補錄的句子聽起來才是同一個人；不要只換一部分的聲音。`--dry-run` 不需要金鑰，會列出接下來要產生哪些句子與用的語氣，先看一眼再決定。若 API 回應格式不符，加 `--api classic` 改用 generateContent 格式。
- 逐句聽過、不滿意的刪掉重跑，再提交 `public/voice/`。重新產生語音檔後，記得把 `public/sw.js` 的 `CACHE` 版本號加一，已安裝的使用者才會拿到新檔。
- 驗收：`python tools/voice/verify.py` 檢查格式、語速、靜音、停頓、音量、爆音、音高起伏；加 `--listen` 再請 Gemini 聽寫並評分（需要 `GEMINI_API_KEY`，評分只供篩選）。結果在 `tools/voice/report/`（不進版控），`review.html` 可直接聽、標「過／重做」並匯出重做指令。

## 本機開發

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
trunk serve --open        # http://127.0.0.1:8080
cargo test                # 資料格式、呼吸階段計算與語音稿一致性的測試
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
