// 課程內容都在這裡。改課程文字只動這個檔案，並同步改 lessons/ 裡對應的 Markdown。
// 執行 `node tools/check-lessons.js` 會逐字比對兩邊，不一致就會報錯。
window.NIGHT_YOGA = {
  // 呼吸引導圈。來源只寫「吸氣加深、吐氣拉長」，沒有給秒數；這裡的秒數是專案自訂的規劃值。
  breath: { inhaleMs: 2500, cycleMs: 6000, minScale: 0.7 },

  // 每堂課共用的「停下來的時機」，與 lessons/*.md 最後一節相同。
  stop: '任何地方出現刺痛、發麻，或越做越不舒服，就停下來，回到攤屍式。',

  // 說明頁底部的小字。{inhale}、{exhale} 會換成上面 breath 的秒數。
  notes: [
    '沒有聲音。可以先把螢幕亮度調低，開始後螢幕會保持亮著。',
    '這是睡前的放鬆習慣，不是失眠治療。瑜珈對睡眠的研究測的是持續數週、每次 45～120 分鐘的完整課程，這樣短的睡前課程不能直接套用那些結果。',
    '引導圈的節奏（吸 {inhale} 秒、吐 {exhale} 秒）是這個專案自訂的，來源只寫「吸氣加深、吐氣拉長」。懷孕、年長，或有健康狀況的人，請先和醫療人員討論。'
  ],

  lessons: [
    {
      id: '01',
      title: '躺好就好',
      file: 'lessons/01-lie-down.md',
      tagline: '全程躺著，做完直接睡',
      prep: ['頭下 1 顆枕頭。', '想墊在膝蓋下方的話，再準備 2 顆。'],
      steps: [
        {
          name: '躺好', pose: '攤屍式', min: 2,
          src: [
            ['GhostBed', 'https://www.ghostbed.com/pages/yoga-in-bed'],
            ['YogaUOnline', 'https://yogauonline.com/yoga-health-benefits/insomnia/this-bedtime-yoga-sequence-to-find-deep-restful-sleep/']
          ],
          cues: [
            '仰躺下來，雙手放在身體兩側，雙腿自然伸直。不用躺得很正，舒服就好。',
            '讓身體慢慢沉進床墊。把注意力放在呼吸上：吸氣的時候稍微加深一點，吐氣的時候拉長一點。',
            '如果思緒跑掉了，沒關係，再回到呼吸。'
          ]
        },
        {
          name: '束角式', pose: '仰臥束角式', min: 3,
          src: [
            ['Yoga Journal', 'https://www.yogajournal.com/yoga-101/pillow-poses-relaxing-restorative-yoga-you-can-do-in-bed'],
            ['PureWow', 'https://www.purewow.com/wellness/yoga-in-bed'],
            ['GhostBed', 'https://www.ghostbed.com/pages/yoga-in-bed']
          ],
          cues: [
            '把兩隻腳的腳掌靠在一起，讓膝蓋慢慢往兩邊打開，像是翻開一本書。',
            '肩膀離耳朵遠一點，雙手放在身上或身體兩側。',
            '如果下背有點不舒服，把腳跟往遠離身體的方向滑一些，膝蓋下方各墊一顆枕頭。',
            '如果大腿內側有刺痛感，就在大腿下方墊結實的枕頭。這個姿勢應該是溫和的，不該疼痛。'
          ]
        },
        {
          name: '回到攤屍式', pose: '攤屍式', min: 3,
          src: [
            ['GhostBed', 'https://www.ghostbed.com/pages/yoga-in-bed'],
            ['YogaUOnline', 'https://yogauonline.com/yoga-health-benefits/insomnia/this-bedtime-yoga-sequence-to-find-deep-restful-sleep/']
          ],
          cues: [
            '慢慢把雙腿伸直，回到仰躺。',
            '繼續吸氣加深、吐氣拉長。感覺身體的重量交給床。',
            '準備好了，就可以直接睡。'
          ]
        }
      ]
    }
  ]
};
