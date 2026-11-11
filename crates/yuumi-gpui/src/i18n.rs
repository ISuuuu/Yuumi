//! 壳层多语言运行时（对照 Vue `src/i18n.ts` 与 `src/locales/{zh-CN,zh-TW,en-US}.json`）。
//!
//! - 键名沿用 Vue 键表（如 `nav.home`），键值从三个 locales JSON 的壳层子集移植；
//!   GPUI 新增且 Vue 无对应键的文案（设置页 GPUI 专属描述、托盘主页/退出等）单独补键，三语言同键集。
//! - 语言来源：`Personalization.Language`（"zh_CN"/"zh_TW"/"en_US"/"Auto"），
//!   "Auto" 在启动时经 `Lang::from_config` 解析为系统 UI 语言（Windows `GetUserDefaultUILanguage`），
//!   判定失败回落 zh-CN；解析结果存入全局 `LANGUAGE`。
//! - `t(key)` 在渲染时查当前语言表：缺键回落 zh-CN，再缺返回键名本身（便于发现漏键）。
//!   GPUI 每帧重调 render，切换语言后 `cx.notify()` 全量重渲染即立即生效。

use std::sync::RwLock;

/// 界面语言（"Auto" 仅存在于配置，启动时已解析为具体语言）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    ZhCn,
    ZhTw,
    EnUs,
}

impl Lang {
    /// 解析配置字符串（对照 Vue `Personalization.Language` 取值）。
    pub fn from_config(language: &str) -> Self {
        match language {
            "zh_TW" => Lang::ZhTw,
            "en_US" => Lang::EnUs,
            "zh_CN" => Lang::ZhCn,
            // "Auto" 或未知值：跟随系统默认 UI 语言
            _ => Self::from_system(),
        }
    }

    /// 跟随系统 UI 区域：Windows 用 `GetUserDefaultUILanguage` 的 LANGID 判定
    /// （低 10 位主语言 / 高 6 位子语言；简中 0x0804、繁中 0x0404、英文 0x0009）。
    /// 非 Windows 平台与判定失败一律回落 zh-CN。
    fn from_system() -> Self {
        #[cfg(windows)]
        {
            let lang_id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
            let primary = lang_id & 0x03FF;
            let sublang = (lang_id >> 10) & 0x3F;
            match primary {
                // 中文：TW(0x01)/HK(0x09)/MO(0x15) 等繁中子语言 → zh_TW，其余（CN/SG）→ zh_CN
                0x0004 => {
                    if matches!(sublang, 0x01 | 0x09 | 0x15) {
                        Lang::ZhTw
                    } else {
                        Lang::ZhCn
                    }
                }
                0x0009 => Lang::EnUs,
                _ => Lang::ZhCn,
            }
        }
        #[cfg(not(windows))]
        {
            Lang::ZhCn
        }
    }
}

// ─── 三张语言表：键集合一致（见 tests），值来自 Vue locales JSON 与 GPUI 新增键 ───

static LANG_ZH_CN: &[(&str, &str)] = &[
    // ─── 导航侧栏 / 托盘 / 路由标题（Vue nav.*） ───
    ("nav.home", "启动页"),
    ("nav.career", "生涯"),
    ("nav.search", "战绩查询"),
    ("nav.gameinfo", "对局信息"),
    ("nav.tft", "云顶之弈"),
    ("nav.tools", "其他功能"),
    ("nav.reconnect", "修复无限加载"),
    ("nav.notice", "公告"),
    ("nav.settings", "设置"),
    ("nav.opgg", "OP.GG"),
    ("nav.savedPlayers", "路人集"),
    // ─── 标题栏阵营徽章 ───
    ("titlebar.blueSide", "蓝色方"),
    ("titlebar.redSide", "红色方"),
    // ─── 游戏阶段徽章（对照 Vue phase.*） ───
    ("phase.Lobby", "房间组队中"),
    ("phase.Matchmaking", "匹配中"),
    ("phase.CheckedIntoTournament", "锦标赛签到"),
    ("phase.ReadyCheck", "确认对局"),
    ("phase.ChampSelect", "选择英雄"),
    ("phase.GameStart", "游戏加载"),
    ("phase.FailedToLaunch", "启动失败"),
    ("phase.InProgress", "游戏中"),
    ("phase.Reconnect", "重新连接"),
    ("phase.WaitingForStats", "等待数据"),
    ("phase.PreEndOfGame", "结算中"),
    ("phase.EndOfGame", "对局结束"),
    ("phase.TerminatedInError", "对局异常终止"),
    ("phase.TerminatedInSeries", "系列赛终止"),
    ("phase.WatchInProgress", "观战中"),
    // ─── 公告弹窗 ───
    ("noticePage.title", "公告"),
    ("noticePage.loadingText", "正在获取最新公告..."),
    ("noticePage.errorText", "无法获取公告，请检查您的网络连接"),
    ("noticePage.retryBtn", "重新加载"),
    ("noticePage.emptyContent", "当前版本已完整复刻 Vue 纯白水晶极光界面，Direct3D 11 原生极速渲染，内存与启动性能优化完毕。"),
    // ─── 公共文案 ───
    ("common.lcuNotConnected", "LCU 未连接，请先启动英雄联盟客户端"),
    ("common.reconnectTriggered", "已触发游戏重连"),
    ("common.reconnectFailed", "重连请求失败"),
    ("common.save", "保存"),
    ("common.refresh", "刷新"),
    // ─── 设置页 ───
    ("settings.title", "设置"),
    ("settings.groupFeatures", "功能"),
    ("settings.lcuConcurrencyTitle", "LCU API 并发数"),
    ("settings.lcuConcurrencyDesc", "该值越大数据加载速度越快，但越可能引起客户端闪退"),
    ("settings.defaultGamesTitle", "默认对局数量"),
    ("settings.defaultGamesDesc", "调整在个人生涯界面中显示的最大对局数量"),
    ("settings.gameInfoFilterTitle", "对局信息过滤"),
    ("settings.gameInfoFilterDesc", "基于你当前游戏模式（地图/队列）筛选战绩，只显示相同模式的玩家历史数据"),
    ("settings.reserveGameInfoTitle", "保留对局信息界面内容"),
    ("settings.reserveGameInfoDesc", "保留上一局的对局信息内容直到下一次对局开始"),
    ("settings.showTierInGameInfoTitle", "对局详情中显示段位"),
    ("settings.showTierInGameInfoDesc", "在搜索界面对局详情界面中显示段位，启动该选项将影响加载界面的速度"),
    ("settings.autoShowOpggTitle", "自动显示 OP.GG 窗口"),
    ("settings.autoShowOpggDesc", "在英雄选择开始时自动显示 OP.GG 窗口"),
    ("settings.pinOpggTitle", "置顶 OP.GG 窗口"),
    ("settings.pinOpggDesc", "在英雄选择时将 OP.GG 窗口置顶"),
    ("settings.generalGroup", "通用设置"),
    ("settings.lolPathGroup", "英雄联盟客户端路径"),
    ("settings.lolPathDesc", "设置英雄联盟客户端所在目录的路径"),
    ("settings.pathNotSet", "未设置"),
    ("settings.pathSetCount", "已设置 {count} 个路径"),
    ("settings.pathEmpty", "暂无客户端路径"),
    ("settings.wegamePathPlaceholder", "WeGame 安装路径（可手动修改）"),
    ("settings.detectBtn", "自动检测"),
    ("settings.browseBtn", "浏览文件夹"),
    ("settings.wegameAddBtn", "添加 WeGame 启动项"),
    ("settings.autoStartLolTitle", "启动时自动运行 LOL"),
    ("settings.autoStartLolDesc", "Yuumi 启动后自动检测并运行英雄联盟客户端"),
    ("settings.clearCacheTitle", "清除缓存"),
    ("settings.clearCacheDesc", "删除所有游戏资源的缓存（建议在游戏资源有更新时使用）"),
    ("settings.deleteBtn", "删除"),
    ("settings.closeToTrayTitle", "关闭到托盘"),
    ("settings.closeToTrayDesc", "点击关闭按钮时将窗口隐藏到系统托盘区域"),
    ("settings.startMinimizedTitle", "启动时最小化"),
    ("settings.startMinimizedDesc", "软件启动后自动隐藏主窗口并最小化到系统托盘"),
    ("settings.cloudServiceTitle", "云端服务"),
    ("settings.cloudServiceDesc", "LCU 实时数据上传及 SignalR 实时查询服务"),
    ("settings.uploadConfigured", "已配置上传"),
    ("settings.uploadNotConfigured", "未配置"),
    ("settings.apiServerAddrLabel", "服务器 API 地址:"),
    ("settings.realtimeLcuLabel", "LCU 实时查询:"),
    ("settings.useridPlaceholder", "留空默认使用 lcu_user_001"),
    ("settings.hideMenuTitle", "隐藏菜单"),
    ("settings.hideMenuDesc", "在侧边栏中隐藏选中的菜单入口"),
    ("settings.enabled", "已启用"),
    ("settings.disabled", "未启用"),
    ("settings.hideMenuOptionTft", "云顶之弈"),
    ("settings.hideMenuOptionSavedPlayers", "路人集"),
    ("settings.screenshotGroup", "自动截图"),
    ("settings.enableScreenshotOnMultikillTitle", "多杀与超神自动截图"),
    ("settings.enableScreenshotOnMultikillDesc", "多杀或超神时自动截图。游戏视频设置须为「无边框」或「窗口化」以防黑屏"),
    ("settings.screenshotLevelsTitle", "截图触发事件"),
    ("settings.tripleKill", "三杀"),
    ("settings.quadraKill", "四杀"),
    ("settings.pentaKill", "五杀"),
    ("settings.legendary", "超神"),
    ("settings.screenshotSavePathTitle", "截图保存位置"),
    ("settings.screenshotSavePathPlaceholder", "默认保存在系统「图片」目录下的 Yuumi_Screenshots 文件夹"),
    ("settings.openScreenshotFolderBtn", "打开截图文件夹"),
    ("settings.logGroup", "日志"),
    ("settings.logLevelTitle", "日志等级"),
    ("settings.logLevelDesc", "日志写入文件记录的等级（Debug, Info, Error）"),
    ("settings.logFileTitle", "日志文件"),
    ("settings.logFileDesc", "[exe 目录]/log/"),
    ("settings.openFolderBtn", "打开文件夹"),
    ("settings.personalizationGroup", "个性化"),
    ("settings.micaTitle", "云母效果 (Windows 11)"),
    ("settings.micaDesc", "启用 Windows 11 云母材质效果，使窗口背景模糊及跟随系统主题"),
    ("settings.themeModeTitle", "主题模式"),
    ("settings.themeModeDesc", "选择浅色/深色/跟随系统"),
    ("settings.themeModeLight", "浅色"),
    ("settings.themeModeDark", "深色"),
    ("settings.themeModeAuto", "跟随系统"),
    ("settings.themeColorTitle", "主题颜色"),
    ("settings.themeColorDesc", "自定义软件界面主色调 (点击色块选择)"),
    ("settings.themeColorPickerLabel", "选择调色盘颜色:"),
    ("settings.resetColors", "重置默认颜色"),
    ("settings.cardColorTitle", "战绩卡片颜色"),
    ("settings.cardColorDesc", "自定义胜利、失败、重开对局的战绩卡片颜色"),
    ("settings.cardColorStatusSet", "已设置自定义颜色"),
    ("settings.winCard", "胜利"),
    ("settings.loseCard", "失败"),
    ("settings.remakeCard", "重开"),
    ("settings.deathColorTitle", "死亡数字颜色"),
    ("settings.deathColorDesc", "自定义对局详情中死亡数字的颜色"),
    ("settings.deathColorStatusSet", "已配置死亡高亮"),
    ("settings.lightDeathLabel", "浅色模式高亮色:"),
    ("settings.darkDeathLabel", "深色模式高亮色:"),
    ("settings.dpiScaleTitle", "界面缩放"),
    ("settings.dpiScaleDesc", "支持调整客户端视图的 DPI 缩放大小比例"),
    ("settings.dpiScaleAuto", "跟随系统"),
    ("settings.langTitle", "语言"),
    ("settings.langDesc", "选择软件界面语言选项"),
    ("settings.langAuto", "跟随系统"),
    ("settings.langSaved", "语言已保存并即时生效"),
    ("settings.softwareUpdateGroup", "软件更新"),
    ("settings.checkUpdateTitle", "检查更新"),
    ("settings.checkUpdateBtn", "检查更新"),
    ("settings.goToReleases", "前往 GitHub 下载"),
    ("settings.httpProxyGroup", "软件 HTTP 代理"),
    ("settings.httpProxyDesc", "软件更新、TFT 数据、OP.GG 及 GitHub 服务访问走此 HTTP 代理通道"),
    ("settings.aboutGroup", "关于"),
    ("settings.aboutHistoryTitle", "更新日志"),
    ("settings.aboutHistoryBtn", "更新日志"),
    ("settings.aboutRepository", "开源仓库"),
    ("settings.aboutHistoryLoading", "正在加载..."),
    ("settings.cloudConnected", "云端已连接"),
    ("settings.cloudConnecting", "云端连接中..."),
    ("settings.cloudConnectFailed", "云端连接失败"),
    ("settings.cloudDisconnected", "云端未连接"),
    ("settings.realtimeLcuHint", "使用上方的服务器 API 地址"),
    ("settings.paletteHint", "选择推荐色阶:"),
    ("settings.checkUpdateDescPortable", "检查是否有签名的 GPUI 便携版更新"),
    ("settings.checkUpdateDescGpui", "启动时检查 GPUI 更新；开启后将自动下载可用更新"),
    ("settings.aboutIntroGpui", "Yuumi 是一款基于 Tauri 与 GPUI 原生构建的英雄联盟辅助工具，提供战绩查询、对局信息、云顶之弈数据、战利品管理以及自动对局功能，让游戏体验更轻松。"),
    ("settings.micaError", "当前系统无法切换云母效果"),
    ("settings.dpiScaleApplied", "DPI 缩放已保存"),
    ("settings.pathExists", "该路径已存在"),
    ("settings.pathAdded", "已添加: {path}"),
    ("settings.lolClientNotFound", "未检测到运行中的英雄联盟客户端"),
    ("settings.wegameExists", "WeGame 启动项已存在"),
    ("settings.wegameAdded", "已添加 WeGame 启动项"),
    ("settings.uploadUrlInvalid", "请输入有效的 HTTP 或 HTTPS 上传地址"),
    ("settings.proxyAddrInvalid", "请输入有效的代理地址（例如 127.0.0.1:7897）"),
    ("settings.clearingCache", "正在清理缓存..."),
    ("settings.clearCacheFailed", "清理失败: {error}"),
    ("settings.screenshotDirUpdated", "已成功更新截图保存目录"),
    ("settings.screenshotFolderOpenFailed", "无法打开截图文件夹: {error}"),
    ("settings.logFolderOpenFailed", "打开日志文件夹失败: {error}"),
    ("settings.saveFailedDetail", "设置保存失败: {error}"),
    ("settings.configLoadError", "配置文件异常"),
    ("settings.masteryLoadFailed", "获取英雄熟练度失败，请确认客户端已连接"),
    ("settings.cloudInvalidAddr", "云端地址无效"),
    // ─── tray ───
    ("tray.home", "主页"),
    ("tray.quit", "退出"),
];

static LANG_ZH_TW: &[(&str, &str)] = &[
    // ─── 导航侧栏 / 托盘 / 路由标题（Vue nav.*） ───
    ("nav.home", "啟動頁"),
    ("nav.career", "生涯"),
    ("nav.search", "戰績查詢"),
    ("nav.gameinfo", "對局信息"),
    ("nav.tft", "聯盟戰棋"),
    ("nav.tools", "其他功能"),
    ("nav.reconnect", "修復無限加載"),
    ("nav.notice", "公告"),
    ("nav.settings", "設置"),
    ("nav.opgg", "OP.GG"),
    ("nav.savedPlayers", "路人集"),
    // ─── 标题栏阵营徽章 ───
    ("titlebar.blueSide", "藍色方"),
    ("titlebar.redSide", "紅色方"),
    // ─── 游戏阶段徽章（对照 Vue phase.*） ───
    ("phase.Lobby", "房間組隊中"),
    ("phase.Matchmaking", "匹配中"),
    ("phase.CheckedIntoTournament", "錦標賽簽到"),
    ("phase.ReadyCheck", "確認對局"),
    ("phase.ChampSelect", "選擇英雄"),
    ("phase.GameStart", "遊戲加載"),
    ("phase.FailedToLaunch", "啟動失敗"),
    ("phase.InProgress", "遊戲中"),
    ("phase.Reconnect", "重新連接"),
    ("phase.WaitingForStats", "等待數據"),
    ("phase.PreEndOfGame", "結算中"),
    ("phase.EndOfGame", "對局結束"),
    ("phase.TerminatedInError", "對局異常終止"),
    ("phase.TerminatedInSeries", "系列賽終止"),
    ("phase.WatchInProgress", "觀戰中"),
    // ─── 公告弹窗 ───
    ("noticePage.title", "公告"),
    ("noticePage.loadingText", "正在獲取最新公告..."),
    ("noticePage.errorText", "無法獲取公告，請檢查您的網絡連接"),
    ("noticePage.retryBtn", "重新加載"),
    ("noticePage.emptyContent", "當前版本已完整復刻 Vue 純白水晶極光界面，Direct3D 11 原生極速渲染，內存與啟動性能優化完畢。"),
    // ─── 公共文案 ───
    ("common.lcuNotConnected", "LCU 未連接，請先啟動英雄聯盟客戶端"),
    ("common.reconnectTriggered", "已觸發遊戲重連"),
    ("common.reconnectFailed", "重連請求失敗"),
    ("common.save", "保存"),
    ("common.refresh", "刷新"),
    // ─── 设置页 ───
    ("settings.title", "設置"),
    ("settings.groupFeatures", "功能"),
    ("settings.lcuConcurrencyTitle", "LCU API 並發數"),
    ("settings.lcuConcurrencyDesc", "該值越大數據加載速度越快，但越可能引起客戶端閃退"),
    ("settings.defaultGamesTitle", "默認對局數量"),
    ("settings.defaultGamesDesc", "調整在個人生涯界面中顯示的最大對局數量"),
    ("settings.gameInfoFilterTitle", "對局信息過濾"),
    ("settings.gameInfoFilterDesc", "基於你當前遊戲模式（地圖/隊列）篩選戰績，只顯示相同模式的玩家歷史數據"),
    ("settings.reserveGameInfoTitle", "保留對局信息界面內容"),
    ("settings.reserveGameInfoDesc", "保留上一局對局信息內容直到下一次對局開始"),
    ("settings.showTierInGameInfoTitle", "對局詳情中顯示段位"),
    ("settings.showTierInGameInfoDesc", "在搜索界面對局詳情界面中顯示段位，啟動該選項將影響加載界面的速度"),
    ("settings.autoShowOpggTitle", "自動顯示 OP.GG 窗口"),
    ("settings.autoShowOpggDesc", "在英雄選擇開始時自動顯示 OP.GG 窗口"),
    ("settings.pinOpggTitle", "置頂 OP.GG 窗口"),
    ("settings.pinOpggDesc", "在英雄選擇時將 OP.GG 窗口置頂"),
    ("settings.generalGroup", "通用設置"),
    ("settings.lolPathGroup", "英雄聯盟客戶端路徑"),
    ("settings.lolPathDesc", "設置英雄聯盟客戶端所在目錄的路徑"),
    ("settings.pathNotSet", "未設置"),
    ("settings.pathSetCount", "已設置 {count} 個路徑"),
    ("settings.pathEmpty", "暫無客戶端路徑"),
    ("settings.wegamePathPlaceholder", "WeGame 安裝路徑（可手動修改）"),
    ("settings.detectBtn", "自動檢測"),
    ("settings.browseBtn", "瀏覽文件夾"),
    ("settings.wegameAddBtn", "添加 WeGame 啟動項"),
    ("settings.autoStartLolTitle", "啟動時自動運行 LOL"),
    ("settings.autoStartLolDesc", "Yuumi 啟動後自動檢測並運行英雄聯盟客戶端"),
    ("settings.clearCacheTitle", "清除緩存"),
    ("settings.clearCacheDesc", "刪除所有遊戲資源的緩存（建議在遊戲資源有更新時使用）"),
    ("settings.deleteBtn", "刪除"),
    ("settings.closeToTrayTitle", "關閉到托盤"),
    ("settings.closeToTrayDesc", "點擊關閉按鈕時將窗口隱藏到系統托盤區域"),
    ("settings.startMinimizedTitle", "啟動時最小化"),
    ("settings.startMinimizedDesc", "軟件啟動後自動隱藏主窗口並最小化到系統托盤"),
    ("settings.cloudServiceTitle", "雲端服務"),
    ("settings.cloudServiceDesc", "LCU 實時數據上傳及 SignalR 實時查詢服務"),
    ("settings.uploadConfigured", "已配置上傳"),
    ("settings.uploadNotConfigured", "未配置"),
    ("settings.apiServerAddrLabel", "服務器 API 地址:"),
    ("settings.realtimeLcuLabel", "LCU 實時查詢:"),
    ("settings.useridPlaceholder", "留空默認使用 lcu_user_001"),
    ("settings.hideMenuTitle", "隱藏菜單"),
    ("settings.hideMenuDesc", "在側邊欄中隱藏選中的菜單入口"),
    ("settings.enabled", "已啟用"),
    ("settings.disabled", "未啟用"),
    ("settings.hideMenuOptionTft", "雲頂之弈"),
    ("settings.hideMenuOptionSavedPlayers", "路人集"),
    ("settings.screenshotGroup", "自動截圖"),
    ("settings.enableScreenshotOnMultikillTitle", "多殺與超神自動截圖"),
    ("settings.enableScreenshotOnMultikillDesc", "多殺或超神時自動截圖。遊戲視頻設置須為「無邊框」或「窗口化」以防黑屏"),
    ("settings.screenshotLevelsTitle", "截圖觸發事件"),
    ("settings.tripleKill", "三殺"),
    ("settings.quadraKill", "四殺"),
    ("settings.pentaKill", "五殺"),
    ("settings.legendary", "超神"),
    ("settings.screenshotSavePathTitle", "截圖保存位置"),
    ("settings.screenshotSavePathPlaceholder", "默認保存在系統「圖片」目錄下的 Yuumi_Screenshots 文件夾"),
    ("settings.openScreenshotFolderBtn", "打開截圖文件夾"),
    ("settings.logGroup", "日誌"),
    ("settings.logLevelTitle", "日誌等級"),
    ("settings.logLevelDesc", "日誌寫入文件記錄的等級（Debug, Info, Error）"),
    ("settings.logFileTitle", "日誌文件"),
    ("settings.logFileDesc", "[exe 目錄]/log/"),
    ("settings.openFolderBtn", "打開文件夾"),
    ("settings.personalizationGroup", "個性化"),
    ("settings.micaTitle", "雲母效果 (Windows 11)"),
    ("settings.micaDesc", "啟用 Windows 11 雲母材質效果，使窗口背景模糊及跟隨系統主題"),
    ("settings.themeModeTitle", "主題模式"),
    ("settings.themeModeDesc", "選擇淺色/深色/跟隨系統"),
    ("settings.themeModeLight", "淺色"),
    ("settings.themeModeDark", "深色"),
    ("settings.themeModeAuto", "跟隨系統"),
    ("settings.themeColorTitle", "主題顏色"),
    ("settings.themeColorDesc", "自定義軟件界面主色調 (點擊色塊選擇)"),
    ("settings.themeColorPickerLabel", "選擇調色盤顏色:"),
    ("settings.resetColors", "重置默認顏色"),
    ("settings.cardColorTitle", "戰績卡片顏色"),
    ("settings.cardColorDesc", "自定義勝利、失敗、重開對局的戰績卡片顏色"),
    ("settings.cardColorStatusSet", "已設置自定義顏色"),
    ("settings.winCard", "勝利"),
    ("settings.loseCard", "失敗"),
    ("settings.remakeCard", "重開"),
    ("settings.deathColorTitle", "死亡數字顏色"),
    ("settings.deathColorDesc", "自定義對局詳情中死亡數字的顏色"),
    ("settings.deathColorStatusSet", "已配置死亡高亮"),
    ("settings.lightDeathLabel", "淺色模式高亮色:"),
    ("settings.darkDeathLabel", "深色模式高亮色:"),
    ("settings.dpiScaleTitle", "界面縮放"),
    ("settings.dpiScaleDesc", "支持調整客戶端視圖的 DPI 縮放大小比例"),
    ("settings.dpiScaleAuto", "跟隨系統"),
    ("settings.langTitle", "語言"),
    ("settings.langDesc", "選擇軟件界面語言選項"),
    ("settings.langAuto", "跟隨系統"),
    ("settings.langSaved", "語言已保存並即時生效"),
    ("settings.softwareUpdateGroup", "軟件更新"),
    ("settings.checkUpdateTitle", "檢查更新"),
    ("settings.checkUpdateBtn", "檢查更新"),
    ("settings.goToReleases", "前往 GitHub 下載"),
    ("settings.httpProxyGroup", "軟體 HTTP 代理"),
    ("settings.httpProxyDesc", "軟體更新、TFT 數據、OP.GG 及 GitHub 服務存取走此 HTTP 代理通道"),
    ("settings.aboutGroup", "關於"),
    ("settings.aboutHistoryTitle", "更新日誌"),
    ("settings.aboutHistoryBtn", "更新日誌"),
    ("settings.aboutRepository", "開源倉庫"),
    ("settings.aboutHistoryLoading", "正在加載..."),
    ("settings.cloudConnected", "雲端已連接"),
    ("settings.cloudConnecting", "雲端連接中..."),
    ("settings.cloudConnectFailed", "雲端連接失敗"),
    ("settings.cloudDisconnected", "雲端未連接"),
    ("settings.realtimeLcuHint", "使用上方的伺服器 API 位址"),
    ("settings.paletteHint", "選擇推薦色階:"),
    ("settings.checkUpdateDescPortable", "檢查是否有簽名的 GPUI 便攜版更新"),
    ("settings.checkUpdateDescGpui", "啟動時檢查 GPUI 更新；開啟後將自動下載可用更新"),
    ("settings.aboutIntroGpui", "Yuumi 是一款基於 Tauri 與 GPUI 原生構建的英雄聯盟輔助工具，提供戰績查詢、對局資訊、雲頂之弈數據、戰利品管理以及自動對局功能，讓遊戲體驗更輕鬆。"),
    ("settings.micaError", "當前系統無法切換雲母效果"),
    ("settings.dpiScaleApplied", "DPI 縮放已保存"),
    ("settings.pathExists", "該路徑已存在"),
    ("settings.pathAdded", "已添加: {path}"),
    ("settings.lolClientNotFound", "未檢測到運行中的英雄聯盟客戶端"),
    ("settings.wegameExists", "WeGame 啟動項已存在"),
    ("settings.wegameAdded", "已添加 WeGame 啟動項"),
    ("settings.uploadUrlInvalid", "請輸入有效的 HTTP 或 HTTPS 上傳地址"),
    ("settings.proxyAddrInvalid", "請輸入有效的代理位址（例如 127.0.0.1:7897）"),
    ("settings.clearingCache", "正在清理緩存..."),
    ("settings.clearCacheFailed", "清理失敗: {error}"),
    ("settings.screenshotDirUpdated", "已成功更新截圖保存目錄"),
    ("settings.screenshotFolderOpenFailed", "無法打開截圖文件夾: {error}"),
    ("settings.logFolderOpenFailed", "打開日誌文件夾失敗: {error}"),
    ("settings.saveFailedDetail", "設置保存失敗: {error}"),
    ("settings.configLoadError", "配置文件異常"),
    ("settings.masteryLoadFailed", "獲取英雄熟練度失敗，請確認客戶端已連接"),
    ("settings.cloudInvalidAddr", "雲端地址無效"),
    // ─── tray ───
    ("tray.home", "主頁"),
    ("tray.quit", "退出"),
];

static LANG_EN_US: &[(&str, &str)] = &[
    // ─── 导航侧栏 / 托盘 / 路由标题（Vue nav.*） ───
    ("nav.home", "Home"),
    ("nav.career", "Career"),
    ("nav.search", "Search"),
    ("nav.gameinfo", "In-Game Info"),
    ("nav.tft", "Teamfight Tactics"),
    ("nav.tools", "Tools"),
    ("nav.reconnect", "Fix Load Loop"),
    ("nav.notice", "Notice"),
    ("nav.settings", "Settings"),
    ("nav.opgg", "OP.GG"),
    ("nav.savedPlayers", "Players Met"),
    // ─── 标题栏阵营徽章 ───
    ("titlebar.blueSide", "Blue Side"),
    ("titlebar.redSide", "Red Side"),
    // ─── 游戏阶段徽章（对照 Vue phase.*） ───
    ("phase.Lobby", "Lobby"),
    ("phase.Matchmaking", "Matchmaking"),
    ("phase.CheckedIntoTournament", "Tournament Check-In"),
    ("phase.ReadyCheck", "Ready Check"),
    ("phase.ChampSelect", "Champ Select"),
    ("phase.GameStart", "Game Start"),
    ("phase.FailedToLaunch", "Failed to Launch"),
    ("phase.InProgress", "In Progress"),
    ("phase.Reconnect", "Reconnecting"),
    ("phase.WaitingForStats", "Waiting for Stats"),
    ("phase.PreEndOfGame", "Ending"),
    ("phase.EndOfGame", "End of Game"),
    ("phase.TerminatedInError", "Terminated with Error"),
    ("phase.TerminatedInSeries", "Terminated in Series"),
    ("phase.WatchInProgress", "Spectating"),
    // ─── 公告弹窗 ───
    ("noticePage.title", "Notice"),
    ("noticePage.loadingText", "Fetching latest announcements..."),
    ("noticePage.errorText", "Failed to fetch announcements, please check your network connection"),
    ("noticePage.retryBtn", "Retry"),
    ("noticePage.emptyContent", "This build fully recreates the aurora-crystal UI with native Direct3D 11 rendering and optimized memory and startup performance."),
    // ─── 公共文案 ───
    ("common.lcuNotConnected", "LCU not connected, please start League Client first"),
    ("common.reconnectTriggered", "Reconnect triggered"),
    ("common.reconnectFailed", "Reconnect failed"),
    ("common.save", "Save"),
    ("common.refresh", "Refresh"),
    // ─── 设置页 ───
    ("settings.title", "Settings"),
    ("settings.groupFeatures", "Features"),
    ("settings.lcuConcurrencyTitle", "LCU API Concurrency"),
    ("settings.lcuConcurrencyDesc", "Higher values load data faster but may cause client crashes"),
    ("settings.defaultGamesTitle", "Default Games"),
    ("settings.defaultGamesDesc", "Max games shown in career view"),
    ("settings.gameInfoFilterTitle", "Filter Game Info"),
    ("settings.gameInfoFilterDesc", "Filter match history based on current game mode (map/queue)"),
    ("settings.reserveGameInfoTitle", "Reserve Game Info"),
    ("settings.reserveGameInfoDesc", "Keep last game info until next game starts"),
    ("settings.showTierInGameInfoTitle", "Show Tier in Game Info"),
    ("settings.showTierInGameInfoDesc", "Show rank tier in game info details. Enabling this may affect page load speed"),
    ("settings.autoShowOpggTitle", "Auto Show OP.GG"),
    ("settings.autoShowOpggDesc", "Automatically open OP.GG window when champ select starts"),
    ("settings.pinOpggTitle", "Pin OP.GG Window"),
    ("settings.pinOpggDesc", "Keep OP.GG window on top during champ select"),
    ("settings.generalGroup", "General"),
    ("settings.lolPathGroup", "LOL Client Path"),
    ("settings.lolPathDesc", "Set the League Client installation directory"),
    ("settings.pathNotSet", "Not set"),
    ("settings.pathSetCount", "{count} paths set"),
    ("settings.pathEmpty", "No client paths configured"),
    ("settings.wegamePathPlaceholder", "WeGame install path (editable)"),
    ("settings.detectBtn", "Auto Detect"),
    ("settings.browseBtn", "Browse"),
    ("settings.wegameAddBtn", "Add WeGame Launcher"),
    ("settings.autoStartLolTitle", "Auto Launch LOL"),
    ("settings.autoStartLolDesc", "Auto-detect and launch League Client when Yuumi starts"),
    ("settings.clearCacheTitle", "Clear Cache"),
    ("settings.clearCacheDesc", "Delete all game resource caches (use when game resources update)"),
    ("settings.deleteBtn", "Delete"),
    ("settings.closeToTrayTitle", "Close to Tray"),
    ("settings.closeToTrayDesc", "Hide window to system tray when closing"),
    ("settings.startMinimizedTitle", "Start Minimized"),
    ("settings.startMinimizedDesc", "Auto-minimize Yuumi to system tray on startup"),
    ("settings.cloudServiceTitle", "Cloud Service"),
    ("settings.cloudServiceDesc", "LCU real-time data upload & SignalR query service"),
    ("settings.uploadConfigured", "Upload configured"),
    ("settings.uploadNotConfigured", "Not configured"),
    ("settings.apiServerAddrLabel", "API Server URL:"),
    ("settings.realtimeLcuLabel", "LCU Real-time Query:"),
    ("settings.useridPlaceholder", "Leave empty for default lcu_user_001"),
    ("settings.hideMenuTitle", "Hide Menu Items"),
    ("settings.hideMenuDesc", "Hide selected menu entries in the sidebar"),
    ("settings.enabled", "Enabled"),
    ("settings.disabled", "Disabled"),
    ("settings.hideMenuOptionTft", "TFT"),
    ("settings.hideMenuOptionSavedPlayers", "Players Met"),
    ("settings.screenshotGroup", "Auto Screenshot"),
    ("settings.enableScreenshotOnMultikillTitle", "Auto Screenshot on Multikill & Legendary"),
    ("settings.enableScreenshotOnMultikillDesc", "Auto screenshot on multikills. Borderless/Windowed mode in LOL is required"),
    ("settings.screenshotLevelsTitle", "Trigger Events"),
    ("settings.tripleKill", "Triple Kill"),
    ("settings.quadraKill", "Quadra Kill"),
    ("settings.pentaKill", "Penta Kill"),
    ("settings.legendary", "Legendary"),
    ("settings.screenshotSavePathTitle", "Save Location"),
    ("settings.screenshotSavePathPlaceholder", "Default: Pictures/Yuumi_Screenshots in home directory"),
    ("settings.openScreenshotFolderBtn", "Open Folder"),
    ("settings.logGroup", "Log"),
    ("settings.logLevelTitle", "Log Level"),
    ("settings.logLevelDesc", "Log file recording level (Debug, Info, Error)"),
    ("settings.logFileTitle", "Log File"),
    ("settings.logFileDesc", "[exe dir]/log/"),
    ("settings.openFolderBtn", "Open Folder"),
    ("settings.personalizationGroup", "Personalization"),
    ("settings.micaTitle", "Mica Effect (Windows 11)"),
    ("settings.micaDesc", "Enable Windows 11 Mica material effect for blurred window background"),
    ("settings.themeModeTitle", "Theme Mode"),
    ("settings.themeModeDesc", "Select light/dark/system theme"),
    ("settings.themeModeLight", "Light"),
    ("settings.themeModeDark", "Dark"),
    ("settings.themeModeAuto", "Follow System"),
    ("settings.themeColorTitle", "Theme Color"),
    ("settings.themeColorDesc", "Customize the main UI color (click to select)"),
    ("settings.themeColorPickerLabel", "Select palette color:"),
    ("settings.resetColors", "Reset Default Colors"),
    ("settings.cardColorTitle", "Card Colors"),
    ("settings.cardColorDesc", "Customize colors for win/loss/remake match cards"),
    ("settings.cardColorStatusSet", "Custom colors set"),
    ("settings.winCard", "Win"),
    ("settings.loseCard", "Loss"),
    ("settings.remakeCard", "Remake"),
    ("settings.deathColorTitle", "Death Number Color"),
    ("settings.deathColorDesc", "Customize the death number color in game details"),
    ("settings.deathColorStatusSet", "Death highlight configured"),
    ("settings.lightDeathLabel", "Light mode highlight:"),
    ("settings.darkDeathLabel", "Dark mode highlight:"),
    ("settings.dpiScaleTitle", "DPI Scaling"),
    ("settings.dpiScaleDesc", "Adjust client view DPI scaling ratio"),
    ("settings.dpiScaleAuto", "Follow System"),
    ("settings.langTitle", "Language"),
    ("settings.langDesc", "Select software interface language"),
    ("settings.langAuto", "Follow System"),
    ("settings.langSaved", "Language saved and applied instantly"),
    ("settings.softwareUpdateGroup", "Software Update"),
    ("settings.checkUpdateTitle", "Check Updates"),
    ("settings.checkUpdateBtn", "Check Update"),
    ("settings.goToReleases", "Download from GitHub"),
    ("settings.httpProxyGroup", "HTTP Proxy"),
    ("settings.httpProxyDesc", "Use HTTP proxy for updates, TFT, OP.GG, and GitHub access"),
    ("settings.aboutGroup", "About"),
    ("settings.aboutHistoryTitle", "Changelog"),
    ("settings.aboutHistoryBtn", "Changelog"),
    ("settings.aboutRepository", "GitHub Repo"),
    ("settings.aboutHistoryLoading", "Loading..."),
    ("settings.cloudConnected", "Cloud connected"),
    ("settings.cloudConnecting", "Connecting to cloud..."),
    ("settings.cloudConnectFailed", "Cloud connection failed"),
    ("settings.cloudDisconnected", "Cloud disconnected"),
    ("settings.realtimeLcuHint", "Uses the API server URL above"),
    ("settings.paletteHint", "Select a palette color:"),
    ("settings.checkUpdateDescPortable", "Check for signed GPUI portable updates"),
    ("settings.checkUpdateDescGpui", "Check for GPUI updates on startup; available updates download automatically"),
    ("settings.aboutIntroGpui", "Yuumi is a League of Legends companion tool built natively with Tauri and GPUI, offering match history, live game info, TFT data, loot management, and automated match features for a smoother gaming experience."),
    ("settings.micaError", "Mica effect is not available on this system"),
    ("settings.dpiScaleApplied", "DPI scaling saved"),
    ("settings.pathExists", "This path already exists"),
    ("settings.pathAdded", "Added: {path}"),
    ("settings.lolClientNotFound", "No running League of Legends client detected"),
    ("settings.wegameExists", "WeGame launcher already exists"),
    ("settings.wegameAdded", "WeGame launcher added"),
    ("settings.uploadUrlInvalid", "Please enter a valid HTTP or HTTPS upload URL"),
    ("settings.proxyAddrInvalid", "Please enter a valid proxy address (e.g. 127.0.0.1:7897)"),
    ("settings.clearingCache", "Clearing cache..."),
    ("settings.clearCacheFailed", "Clear failed: {error}"),
    ("settings.screenshotDirUpdated", "Screenshot save directory updated"),
    ("settings.screenshotFolderOpenFailed", "Failed to open screenshot folder: {error}"),
    ("settings.logFolderOpenFailed", "Failed to open log folder: {error}"),
    ("settings.saveFailedDetail", "Failed to save settings: {error}"),
    ("settings.configLoadError", "Config file error"),
    ("settings.masteryLoadFailed", "Failed to load champion mastery, please check the client connection"),
    ("settings.cloudInvalidAddr", "Invalid cloud address"),
    // ─── tray ───
    ("tray.home", "Home"),
    ("tray.quit", "Exit"),
];
/// 当前语言。默认 zh-CN；启动时由 `MainWindow::apply_language` 按配置初始化。
static LANGUAGE: RwLock<Lang> = RwLock::new(Lang::ZhCn);

/// 当前语言（锁中毒时按默认 zh-CN 处理，避免渲染线程 panic）。
pub fn current_language() -> Lang {
    LANGUAGE.read().map(|g| *g).unwrap_or(Lang::ZhCn)
}

/// 切换全局界面语言（启动初始化与设置页保存时调用）。
pub fn set_language(lang: Lang) {
    if let Ok(mut guard) = LANGUAGE.write() {
        *guard = lang;
    }
}

/// 在指定语言表中线性查键。
fn lookup(table: &'static [(&'static str, &'static str)], key: &str) -> Option<&'static str> {
    table
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}

/// 查询当前语言的壳层文案：缺键回落 zh-CN，再缺返回键名本身（便于发现漏键）。
/// 入参要求 `&'static str`（所有调用点均为键名字面量），才能在缺键时零拷贝返回键名。
pub fn t(key: &'static str) -> &'static str {
    let table = match current_language() {
        Lang::ZhCn => LANG_ZH_CN,
        Lang::ZhTw => LANG_ZH_TW,
        Lang::EnUs => LANG_EN_US,
    };
    lookup(table, key)
        .or_else(|| lookup(LANG_ZH_CN, key))
        .unwrap_or(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Mutex;

    /// 语言是全局状态，涉及 set_language 的用例串行执行，避免线程间互相干扰。
    static LANG_LOCK: Mutex<()> = Mutex::new(());

    fn keys_of(table: &'static [(&'static str, &'static str)]) -> HashSet<&'static str> {
        table.iter().map(|(k, _)| *k).collect()
    }

    #[test]
    fn three_tables_share_identical_key_sets() {
        let zh = keys_of(LANG_ZH_CN);
        assert_eq!(zh.len(), LANG_ZH_CN.len(), "zh-CN 表存在重复键");
        assert_eq!(zh, keys_of(LANG_ZH_TW), "zh-TW 键集与 zh-CN 不一致");
        assert_eq!(zh, keys_of(LANG_EN_US), "en-US 键集与 zh-CN 不一致");
    }

    #[test]
    fn table_values_are_non_empty() {
        for table in [LANG_ZH_CN, LANG_ZH_TW, LANG_EN_US] {
            for (k, v) in table {
                assert!(!v.is_empty(), "键 {k} 的值为空");
            }
        }
    }

    #[test]
    fn lookup_follows_current_language() {
        let _guard = LANG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_language(Lang::ZhCn);
        assert_eq!(t("nav.settings"), "设置");
        set_language(Lang::EnUs);
        assert_eq!(t("nav.settings"), "Settings");
        set_language(Lang::ZhTw);
        assert_eq!(t("nav.settings"), "設置");
        set_language(Lang::ZhCn);
    }

    #[test]
    fn missing_key_falls_back_to_zh_cn_then_key_itself() {
        let _guard = LANG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_language(Lang::EnUs);
        // 未登记的键：回落 zh-CN 也查不到 → 返回键名本身
        assert_eq!(t("no.such.key"), "no.such.key");
        set_language(Lang::ZhCn);
    }

    #[test]
    fn from_config_parses_known_values() {
        assert_eq!(Lang::from_config("zh_CN"), Lang::ZhCn);
        assert_eq!(Lang::from_config("zh_TW"), Lang::ZhTw);
        assert_eq!(Lang::from_config("en_US"), Lang::EnUs);
        // "Auto" 与未知值必须解析为合法语言（Windows 跟随系统区域；非 Windows 固定 zh-CN）
        assert!(matches!(
            Lang::from_config("Auto"),
            Lang::ZhCn | Lang::ZhTw | Lang::EnUs
        ));
        assert!(matches!(
            Lang::from_config("fr_FR"),
            Lang::ZhCn | Lang::ZhTw | Lang::EnUs
        ));
    }
}
