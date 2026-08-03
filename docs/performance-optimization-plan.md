# Yuumi 页面卡顿与网络阻塞优化建议

本文档整理当前静态审查中发现的性能热点，供后续模型或开发者按优先级实施。重点场景包括：页面切换卡顿、按钮点击后反馈慢、网络请求导致 UI 不流畅、Search/GameInfo 重页面加载慢。

## 目标

- 点击按钮后尽快给出 UI 反馈，不让预取、段位、上传等后台工作阻塞当前交互。
- 页面切换时避免一次性触发大量网络请求、图片解析和响应式更新。
- 高频 LCU WebSocket 事件不应因日志或深对象更新拖慢主线程。
- 保持现有功能行为，不改 API 返回格式、Rust 命令签名、配置结构和认证/权限逻辑。

## 当前已有优化

项目不是完全没有优化，已有基础包括：

- `App.vue` 使用 `defineAsyncComponent` 和 `hasVisited*` 延迟挂载部分页面。
- Search 页面已有战绩详情缓存、段位缓存和分页数据缓存。
- `useLcuAsset.ts` 已将同一渲染批次的多个图片请求合并为一次 `get_lcu_assets` IPC。
- Rust 侧部分网络请求已有超时、`reqwest::Client` 复用和资源文件缓存。

后续修改应在这些基础上做小范围增强，避免重写整页或重构无关模块。

## P0：Search 首次搜索被预取阻塞

### 位置

- `src/views/Search.vue:418`
- `src/views/Search.vue:424`
- `src/views/Search.vue:481`

### 现象

`loadMatchHistoryList()` 首次拉取 20 条战绩后，立即 `await loadMoreMatches()` 继续预取 30 条。用户点击搜索后，当前页已经有足够数据可以渲染，但 UI 仍会等待额外网络请求完成。

翻页接近末尾时也同步 `await loadMoreMatches()`，会造成翻页按钮点下去后等待网络。

### 建议

将预取改为后台任务：

- 首次加载只等待当前页必需数据。
- 预取用 `setTimeout(() => void loadMoreMatches(), 0)`、`requestIdleCallback` 或一个轻量 `schedulePrefetchMatches()`。
- 增量预取完成后再更新 `hasMore` 和 `allMatchesSearch`。
- 保留 `loadingMore` 防重入逻辑。

### 成功标准

- 点击搜索后，左侧战绩列表先出现，不等待下一批 30 条预取。
- 网络较慢时，翻页按钮不会因为预取长时间卡住。
- 原有后续翻页仍能继续加载更多战绩。

## P0：Search 默认详情存在重复加载风险

### 位置

- `src/views/Search.vue:383`
- `src/views/Search.vue:437`
- `src/views/Search.vue:539`

### 现象

`doSearch()` 在 `loadMatchHistoryList()` 返回后会 `await selectMatch(matches.value[0].gameId)`。但 `loadMatchHistoryList()` 内部已经调用了一次 `selectMatch(matches.value[0].gameId)`，且没有 await。

这可能导致同一个 gameId 的详情请求并发发起，缓存尚未写入时会重复拉取详情、段位和图片。

### 建议

只保留一个默认详情加载入口：

- 推荐让 `loadMatchHistoryList()` 只负责列表和分页，不主动加载详情。
- 在 `doSearch()` 或跳转逻辑中显式调用一次 `selectMatch()`。
- `selectMatch()` 增加请求序号或 token，防止快速点击不同对局时旧请求回写新状态。

示例思路：

```ts
let selectMatchRequestId = 0;

async function selectMatch(gameId: number) {
  const requestId = ++selectMatchRequestId;
  selectedGameId.value = gameId;
  gameLoading.value = true;

  try {
    // fetch detail
    if (requestId !== selectMatchRequestId) return;
    selectedGame.value = detail;
  } finally {
    if (requestId === selectMatchRequestId) {
      gameLoading.value = false;
    }
  }
}
```

### 成功标准

- 搜索默认只请求一次首局详情。
- 快速连续点击多场对局时，最终只显示最后一次点击的对局详情。
- 不出现旧对局段位覆盖新对局的情况。

## P0：Search 段位请求阻塞详情完成

### 位置

- `src/views/Search.vue:616`
- `src/views/Search.vue:619`

### 现象

对局详情加载后，如果开启显示段位，会 `await Promise.all(rankPromises)` 等待所有玩家段位请求完成。LCU 慢或某个请求耗时时，右侧详情完成状态会被拖住。

另外，循环内逐个写 `participantRanks.value[res.puuid] = res.rankStr` 会触发多次响应式更新。

### 建议

将段位加载改成渐进后台更新：

- 详情数据拿到后先设置 `selectedGame.value`，立即展示详情主体。
- `gameLoading` 只代表详情本身，不等待段位。
- 段位请求后台执行，完成后一次性赋值新对象。
- 可增加 `rankLoading` 状态，只影响段位徽章，不影响整个详情面板。

### 成功标准

- 对局详情主体先展示，段位徽章稍后补齐。
- 某个段位请求慢或失败时，不影响装备、KDA、伤害等详情展示。
- 响应式更新次数减少，快速切换对局不会明显卡顿。

## P1：GameInfo 一次性并发过多

### 位置

- `src/composables/useGamePlayerData.ts:258`
- `src/composables/useGamePlayerData.ts:348`
- `src/composables/useGamePlayerData.ts:389`
- `src/composables/useGamePlayerData.ts:430`

### 现象

GameInfo 会对当前队伍或双方队伍使用 `Promise.all` 一次性加载多个玩家。每个玩家内部又会并行请求：

- 召唤师信息
- 战绩列表
- 段位信息
- 可能的宿命检测

这会在页面切换或选人阶段制造请求风暴，LCU 和前端响应式渲染都会承压。

### 建议

限制并发并分阶段加载：

- 引入本地小工具 `runWithConcurrency(items, limit, worker)`，并发建议 2 或 3。
- 先加载当前可见队伍，再后台加载另一队。
- 玩家卡片可逐个更新，不等待全队完成。
- 宿命检测优先级降低，主体数据展示后后台补齐。

### 成功标准

- 切到 GameInfo 时，页面先显示骨架或部分玩家数据，然后逐步补齐。
- LCU 慢时不会出现整页长时间无响应。
- 选人阶段 WebSocket 更新频繁时，页面仍可点击切换队伍。

## P1：高频 LCU WebSocket 日志拖慢主线程

### 位置

- `src/store/lcuStore.ts:271`

### 现象

`lcu-ws-event` 会将 uri 和完整 data 对象打印到控制台。选人阶段 session 数据体积大、更新频繁，DevTools 打开时尤其容易卡顿。

### 建议

- 生产环境关闭该日志。
- 开发环境只打印 uri，或做节流摘要。
- 如需深度调试，可加显式 debug 开关，不默认输出大对象。

示例方向：

```ts
if (import.meta.env.DEV) {
  console.debug("[lcuStore] lcu-ws-event uri:", uri);
}
```

### 成功标准

- 高频选人事件不再刷大量深对象日志。
- 开发调试仍能看到必要 uri。

## P1：图片 data URL 成本偏高

### 位置

- `src/composables/useLcuAsset.ts:5`
- `src/composables/useLcuAsset.ts:66`
- `src-tauri/src/lcu/client.rs:365`
- `src-tauri/src/lcu/client.rs:542`
- `src-tauri/src/lcu/client.rs:550`

### 现象

当前图片资源通过 base64 data URL 进入前端响应式状态。大量英雄头像、召唤师技能、符文、装备图标会带来：

- base64 字符串内存放大。
- Vue 状态保存大量长字符串。
- 搜索详情和战绩列表渲染时产生较多字符串更新。

### 短期建议

- 降低前端 data URL 缓存上限，或按页面分类缓存。
- 对详情页非首屏图片延迟加载。
- 优先加载英雄头像，装备/符文可在详情主体显示后再加载。

### 中期建议

改为返回本地缓存文件 URL 或 Tauri asset protocol URL，而不是 data URL。Rust 侧仍负责下载和缓存，前端只保存短 URL。

### 成功标准

- Search 详情页图片大量加载时，内存和响应式更新压力下降。
- 页面切换后旧页面保留状态时，不保存过多巨大 data URL 字符串。

## P2：页面保活策略可能累积重页面成本

### 位置

- `src/App.vue:741`
- `src/App.vue:755`
- `src/App.vue:770`
- `src/App.vue:783`
- `src/App.vue:801`
- `src/App.vue:814`
- `src/App.vue:827`

### 现象

多个页面使用 `v-show` 保持挂载。好处是状态保留，坏处是访问过 Search、GameInfo、Career、TFT、Tools、SavedPlayers 后，隐藏页面的 DOM、watch、事件监听和缓存仍留在内存中。

### 建议

不要一刀切改成 `v-if`。建议分类：

- Search 和 GameInfo：确实需要保留状态，可继续保活，但应暂停不可见时的后台预取和图片加载。
- Settings、Tools、SavedPlayers、TFT：评估是否真的需要长期保活。若不需要，可切回 `v-if` 或使用可配置的轻量缓存策略。
- 给需要后台工作的组件传入 `active` prop，不活跃时停止轮询、预取和重请求。

### 成功标准

- 多页面来回切换后，后台网络请求不会持续增长。
- 访问过多个重页面后，切回首页仍保持轻量。

## P2：视觉效果可能增加低端设备渲染成本

### 位置示例

- `src/App.vue:1300`
- `src/App.vue:1398`
- `src/views/Search.vue:1512`
- `src/views/Search.vue:1717`
- `src/views/Search.vue:1942`
- `src/views/Search.vue:2532`

### 现象

项目大量使用 `backdrop-filter: blur(...)`、阴影、`transition: all`、淡入动画。玻璃拟态风格符合项目设计，但在 Tauri WebView 和低端显卡上可能造成合成层压力，尤其是大面积滚动区域和 overlay。

### 建议

- 对大面积滚动容器避免 `backdrop-filter`。
- 将 `transition: all` 改为具体属性，如 `transform`、`opacity`、`background-color`。
- 给设置页增加“性能模式/减少动效”开关，降低 blur 半径和阴影。
- 遵循 `prefers-reduced-motion`。

### 成功标准

- Search 详情滚动和页面切换掉帧减少。
- 性能模式下视觉保持可接受，但交互明显更顺。

## 推荐实施顺序

1. Search：预取后台化。
2. Search：去掉默认详情重复加载，加请求序号防旧请求回写。
3. Search：段位后台渐进加载，一次性更新 `participantRanks`。
4. GameInfo：玩家数据加载增加并发限制和分阶段加载。
5. lcuStore：关闭或节流高频 WebSocket 深对象日志。
6. 图片：短期降低 data URL 压力，中期改本地 URL/asset protocol。
7. 页面保活和视觉效果：按页面逐步压测后再调整。

## 验证清单

每次修改后建议至少执行：

```bash
pnpm build
```

如涉及 Rust 命令或资源缓存：

```bash
cd src-tauri
cargo check
```

手工验证场景：

- LCU 已连接时，Search 输入玩家并搜索，首屏列表应快速出现。
- Search 快速点击多个对局，最终详情应对应最后一次点击。
- 开启“显示段位”后，对局详情主体不应等待所有段位才出现。
- 翻页到下一页，网络慢时按钮不应长时间冻结。
- 进入 GameInfo，玩家数据应逐步补齐，不应整页长时间空白。
- 选人阶段打开 DevTools，控制台不应刷大量完整 session 对象。
- 多次切换 Home/Search/GameInfo/Career/Tools 后，后台请求数量不应持续增加。

## 风险边界

请避免在本轮性能优化中修改：

- Rust Tauri command 名称、入参和返回结构。
- LCU API 代理路径和鉴权逻辑。
- 配置文件结构和自动迁移逻辑。
- 上传接口 payload 格式。
- Search/GameInfo 的用户可见功能语义。

如果必须扩大范围，应先说明原因、影响面和回滚方式。

