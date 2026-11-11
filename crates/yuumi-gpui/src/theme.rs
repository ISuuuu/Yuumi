// 大写常量名函数刻意保持与 Vue CSS 变量一一对应的命名风格
#![allow(non_snake_case)]

use gpui::*;

// ─── 运行时主题层 ───
// GPUI 无 CSS 变量机制，以「默认 const + 运行时覆盖表」实现对齐 Vue 的
// 主题模式（浅/深/跟随系统）、主题色与胜负/死亡自定义色的运行时消费。

use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

/// 运行时颜色覆盖表（键 = 常量名）。空表 = 按各常量内置的浅/深默认值渲染。
static RUNTIME_COLORS: LazyLock<RwLock<HashMap<&'static str, Hsla>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 当前是否处于深色主题（由 Theme::apply 写入；Auto 模式跟随系统）。
static DARK_MODE_ACTIVE: LazyLock<RwLock<bool>> = LazyLock::new(|| RwLock::new(false));

/// 运行时覆盖查询：命中覆盖表取覆盖值，否则按当前主题回落到内置默认。
fn runtime_or(key: &'static str, default: Hsla) -> Hsla {
    RUNTIME_COLORS
        .read()
        .ok()
        .and_then(|map| map.get(key).copied())
        .unwrap_or(default)
}

/// 供常量回落逻辑判断当前主题模式
fn is_dark_mode_active() -> bool {
    DARK_MODE_ACTIVE.read().map(|v| *v).unwrap_or(false)
}

const fn const_hsla(h: f32, s: f32, l: f32, a: f32) -> Hsla {
    Hsla { h, s, l, a }
}

/// 纯白水晶极光（Crystal Aurora）主题色板
///
/// 颜色语义与旧 Vue 版 `src/App.vue` 的 `:root` 变量一一对应，
/// 注释中的 `变量名 → 色值` 即 Vue 侧的来源，便于逐项核对。
pub struct Theme;

#[allow(dead_code)]
impl Theme {
    // ── 外壳尺寸（对照 CustomTitleBar.vue / NavigationSidebar.vue）──
    /// 侧栏折叠宽度
    pub const SIDEBAR_WIDTH: Pixels = px(64.0);
    /// 侧栏展开宽度
    pub const SIDEBAR_WIDTH_EXPANDED: Pixels = px(210.0);
    /// 侧栏头部（折叠按钮行）高度
    pub const SIDEBAR_HEADER_HEIGHT: Pixels = px(52.0);
    /// 侧栏菜单行高度
    pub const NAV_ITEM_HEIGHT: Pixels = px(42.0);
    /// 自定义标题栏高度
    pub const TITLEBAR_HEIGHT: Pixels = px(38.0);

    // ── 圆角三档（--radius-sm/md/lg）──
    pub const RADIUS_SM: Pixels = px(6.0);
    pub const RADIUS_MD: Pixels = px(10.0);
    pub const RADIUS_LG: Pixels = px(16.0);

    // ── 基础背景与面板 ──
    pub const BG_APP_DEFAULT: Hsla = const_hsla(0.5833, 0.4000, 0.9804, 1.0); // --bg-color → #f8fafc
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn BG_APP() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.5833, 0.4293, 0.1137, 1.0) } else { Self::BG_APP_DEFAULT };
        runtime_or("BG_APP", fallback)
    }
    pub const SIDEBAR_BG_DEFAULT: Hsla = const_hsla(0.0000, 0.0000, 1.0000, 0.75); // --sidebar-bg → rgba(255,255,255,0.75)
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn SIDEBAR_BG() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.5833, 0.2500, 0.2043, 0.75) } else { Self::SIDEBAR_BG_DEFAULT };
        runtime_or("SIDEBAR_BG", fallback)
    }
    pub const SIDEBAR_HOVER_DEFAULT: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.03); // --hover-bg
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn SIDEBAR_HOVER() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.0000, 0.0000, 1.0000, 0.06) } else { Self::SIDEBAR_HOVER_DEFAULT };
        runtime_or("SIDEBAR_HOVER", fallback)
    }
    pub const SIDEBAR_ACTIVE_DEFAULT: Hsla = const_hsla(0.5108, 1.0000, 0.3333, 0.15); // --primary-color-alpha-15
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn SIDEBAR_ACTIVE() -> Hsla {
        let fallback = Self::SIDEBAR_ACTIVE_DEFAULT;
        runtime_or("SIDEBAR_ACTIVE", fallback)
    }
    pub const SIDEBAR_ACTIVE_BORDER_DEFAULT: Hsla = const_hsla(0.5108, 1.0000, 0.3333, 0.30); // --primary-color-alpha-30
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn SIDEBAR_ACTIVE_BORDER() -> Hsla {
        let fallback = Self::SIDEBAR_ACTIVE_BORDER_DEFAULT;
        runtime_or("SIDEBAR_ACTIVE_BORDER", fallback)
    }
    pub const TITLEBAR_BG_DEFAULT: Hsla = const_hsla(0.0000, 0.0000, 1.0000, 0.85); // --titlebar-bg
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn TITLEBAR_BG() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.5833, 0.2500, 0.2043, 0.85) } else { Self::TITLEBAR_BG_DEFAULT };
        runtime_or("TITLEBAR_BG", fallback)
    }
    pub const CARD_BG_DEFAULT: Hsla = const_hsla(0.0000, 0.0000, 1.0000, 0.75); // --card-bg
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn CARD_BG() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.5833, 0.2500, 0.2043, 0.75) } else { Self::CARD_BG_DEFAULT };
        runtime_or("CARD_BG", fallback)
    }
    pub const CARD_BG_HOVER_DEFAULT: Hsla = const_hsla(0.0000, 0.0000, 1.0000, 0.95); // --card-bg-hover
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn CARD_BG_HOVER() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.5833, 0.2262, 0.3176, 0.95) } else { Self::CARD_BG_HOVER_DEFAULT };
        runtime_or("CARD_BG_HOVER", fallback)
    }
    pub const CARD_BORDER_DEFAULT: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.06); // --border-color
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn CARD_BORDER() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.0000, 0.0000, 1.0000, 0.10) } else { Self::CARD_BORDER_DEFAULT };
        runtime_or("CARD_BORDER", fallback)
    }
    pub const BORDER_HOVER_DEFAULT: Hsla = const_hsla(0.5108, 1.0000, 0.3333, 0.35); // --border-color-hover
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn BORDER_HOVER() -> Hsla {
        let fallback = Self::BORDER_HOVER_DEFAULT;
        runtime_or("BORDER_HOVER", fallback)
    }

    // ── 品牌色与高亮 ──
    pub const ACCENT_PRIMARY_DEFAULT: Hsla = const_hsla(0.5108, 1.0000, 0.3333, 1.0); // --primary-color → #009faa
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn ACCENT_PRIMARY() -> Hsla {
        let fallback = Self::ACCENT_PRIMARY_DEFAULT;
        runtime_or("ACCENT_PRIMARY", fallback)
    }
    pub const ACCENT_HOVER_DEFAULT: Hsla = const_hsla(0.4888, 1.0000, 0.3510, 1.0); // --primary-color-hover → #00b3a7
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn ACCENT_HOVER() -> Hsla {
        let fallback = Self::ACCENT_HOVER_DEFAULT;
        runtime_or("ACCENT_HOVER", fallback)
    }
    pub const ACCENT_LIGHT_DEFAULT: Hsla = const_hsla(0.5108, 1.0000, 0.3333, 0.15); // --primary-color-alpha-15
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn ACCENT_LIGHT() -> Hsla {
        let fallback = Self::ACCENT_LIGHT_DEFAULT;
        runtime_or("ACCENT_LIGHT", fallback)
    }
    /// 极浅品牌底（对应 --primary-color-alpha-10，用于页签激活态）
    pub const ACCENT_ALPHA_10_DEFAULT: Hsla = const_hsla(0.5108, 1.0000, 0.3333, 0.10);
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn ACCENT_ALPHA_10() -> Hsla {
        let fallback = Self::ACCENT_ALPHA_10_DEFAULT;
        runtime_or("ACCENT_ALPHA_10", fallback)
    }
    /// 品牌描边（对应 --primary-color-alpha-20）
    pub const ACCENT_ALPHA_20_DEFAULT: Hsla = const_hsla(0.5108, 1.0000, 0.3333, 0.20);
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn ACCENT_ALPHA_20() -> Hsla {
        let fallback = Self::ACCENT_ALPHA_20_DEFAULT;
        runtime_or("ACCENT_ALPHA_20", fallback)
    }
    /// 兼容旧实现的极光强调色，已收敛到品牌青的亮阶，避免霓虹偏色
    pub const AURORA_CYAN_DEFAULT: Hsla = const_hsla(0.5102, 1.0000, 0.3843, 1.0); // #00b8c4
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn AURORA_CYAN() -> Hsla {
        let fallback = Self::AURORA_CYAN_DEFAULT;
        runtime_or("AURORA_CYAN", fallback)
    }
    pub const AURORA_BLUE_DEFAULT: Hsla = const_hsla(0.5512, 0.9320, 0.5961, 1.0); // #38bdf8
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn AURORA_BLUE() -> Hsla {
        let fallback = Self::AURORA_BLUE_DEFAULT;
        runtime_or("AURORA_BLUE", fallback)
    }

    // ── 胜利状态（--win-*）──
    pub const WIN_BG_DEFAULT: Hsla = const_hsla(0.2998, 0.7340, 0.3980, 0.20); // --win-bg → rgba(57,176,27,0.20)
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn WIN_BG() -> Hsla {
        let fallback = Self::WIN_BG_DEFAULT;
        runtime_or("WIN_BG", fallback)
    }
    pub const WIN_BG_HOVER_DEFAULT: Hsla = const_hsla(0.2998, 0.7340, 0.3980, 0.32); // --win-bg-hover
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn WIN_BG_HOVER() -> Hsla {
        let fallback = Self::WIN_BG_HOVER_DEFAULT;
        runtime_or("WIN_BG_HOVER", fallback)
    }
    pub const WIN_BORDER_DEFAULT: Hsla = const_hsla(0.2998, 0.7340, 0.3980, 0.40); // --win-border
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn WIN_BORDER() -> Hsla {
        let fallback = Self::WIN_BORDER_DEFAULT;
        runtime_or("WIN_BORDER", fallback)
    }
    pub const WIN_BAR_DEFAULT: Hsla = const_hsla(0.2998, 0.7340, 0.3980, 1.0);
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn WIN_BAR() -> Hsla {
        let fallback = Self::WIN_BAR_DEFAULT;
        runtime_or("WIN_BAR", fallback)
    }
    pub const WIN_TEXT_DEFAULT: Hsla = const_hsla(0.4532, 0.8812, 0.1980, 1.0); // --win-color → #065f46
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn WIN_TEXT() -> Hsla {
        let fallback = Self::WIN_TEXT_DEFAULT;
        runtime_or("WIN_TEXT", fallback)
    }

    // ── 失败状态（--loss-*）──
    pub const LOSS_BG_DEFAULT: Hsla = const_hsla(0.0109, 0.8924, 0.4373, 0.20); // --loss-bg → rgba(211,25,12,0.20)
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn LOSS_BG() -> Hsla {
        let fallback = Self::LOSS_BG_DEFAULT;
        runtime_or("LOSS_BG", fallback)
    }
    pub const LOSS_BG_HOVER_DEFAULT: Hsla = const_hsla(0.0109, 0.8924, 0.4373, 0.32); // --loss-bg-hover
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn LOSS_BG_HOVER() -> Hsla {
        let fallback = Self::LOSS_BG_HOVER_DEFAULT;
        runtime_or("LOSS_BG_HOVER", fallback)
    }
    pub const LOSS_BORDER_DEFAULT: Hsla = const_hsla(0.0109, 0.8924, 0.4373, 0.40); // --loss-border
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn LOSS_BORDER() -> Hsla {
        let fallback = Self::LOSS_BORDER_DEFAULT;
        runtime_or("LOSS_BORDER", fallback)
    }
    pub const LOSS_BAR_DEFAULT: Hsla = const_hsla(0.0109, 0.8924, 0.4373, 1.0);
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn LOSS_BAR() -> Hsla {
        let fallback = Self::LOSS_BAR_DEFAULT;
        runtime_or("LOSS_BAR", fallback)
    }
    pub const LOSS_TEXT_DEFAULT: Hsla = const_hsla(0.9539, 0.7966, 0.3471, 1.0); // --loss-color → #9f1239
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn LOSS_TEXT() -> Hsla {
        let fallback = Self::LOSS_TEXT_DEFAULT;
        runtime_or("LOSS_TEXT", fallback)
    }

    // ── 重开状态（--remake-*）──
    pub const REMAKE_BG_DEFAULT: Hsla = const_hsla(0.5972, 0.2022, 0.6510, 0.14); // --remake-bg
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn REMAKE_BG() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.5972, 0.2022, 0.3600, 0.20) } else { Self::REMAKE_BG_DEFAULT };
        runtime_or("REMAKE_BG", fallback)
    }
    pub const REMAKE_BORDER_DEFAULT: Hsla = const_hsla(0.5972, 0.2022, 0.6510, 0.30); // --remake-border
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn REMAKE_BORDER() -> Hsla {
        let fallback = Self::REMAKE_BORDER_DEFAULT;
        runtime_or("REMAKE_BORDER", fallback)
    }
    pub const REMAKE_BAR_DEFAULT: Hsla = const_hsla(0.5972, 0.2022, 0.6510, 1.0);
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn REMAKE_BAR() -> Hsla {
        let fallback = Self::REMAKE_BAR_DEFAULT;
        runtime_or("REMAKE_BAR", fallback)
    }
    pub const REMAKE_TEXT_DEFAULT: Hsla = const_hsla(0.5983, 0.1632, 0.4686, 1.0); // --remake-color → #64748b
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn REMAKE_TEXT() -> Hsla {
        let fallback = Self::REMAKE_TEXT_DEFAULT;
        runtime_or("REMAKE_TEXT", fallback)
    }

    // ── 文字颜色分层（--text-color / --text-muted / --text-dimmed）──
    pub const TEXT_PRIMARY_DEFAULT: Hsla = const_hsla(0.6173, 0.4737, 0.1118, 1.0); // #0f172a
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn TEXT_PRIMARY() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.5833, 0.4000, 0.9412, 1.0) } else { Self::TEXT_PRIMARY_DEFAULT };
        runtime_or("TEXT_PRIMARY", fallback)
    }
    pub const TEXT_SECONDARY_DEFAULT: Hsla = const_hsla(0.5980, 0.1932, 0.3451, 1.0); // #475569
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn TEXT_SECONDARY() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.5833, 0.2444, 0.8392, 1.0) } else { Self::TEXT_SECONDARY_DEFAULT };
        runtime_or("TEXT_SECONDARY", fallback)
    }
    pub const TEXT_MUTED_DEFAULT: Hsla = const_hsla(0.5983, 0.1632, 0.4686, 1.0); // #64748b
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn TEXT_MUTED() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.5833, 0.1544, 0.6549, 1.0) } else { Self::TEXT_MUTED_DEFAULT };
        runtime_or("TEXT_MUTED", fallback)
    }
    pub const TEXT_ACCENT_DEFAULT: Hsla = const_hsla(0.5108, 1.0000, 0.3333, 1.0);
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn TEXT_ACCENT() -> Hsla {
        let fallback = Self::TEXT_ACCENT_DEFAULT;
        runtime_or("TEXT_ACCENT", fallback)
    }
    pub const TEXT_ON_PRIMARY_DEFAULT: Hsla = const_hsla(0.0000, 0.0000, 1.0000, 1.0); // 纯白 #ffffff
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn TEXT_ON_PRIMARY() -> Hsla {
        let fallback = Self::TEXT_ON_PRIMARY_DEFAULT;
        runtime_or("TEXT_ON_PRIMARY", fallback)
    }
    pub const DEATH_RED_DEFAULT: Hsla = const_hsla(350.0 / 360.0, 0.85, 0.55, 1.0); // --death-color → #f43f5e
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn DEATH_RED() -> Hsla {
        let fallback = Self::DEATH_RED_DEFAULT;
        runtime_or("DEATH_RED", fallback)
    }

    // ── 骨架屏与搜索历史标签 ──
    pub const SKELETON_BG_DEFAULT: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.04);
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn SKELETON_BG() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.0000, 0.0000, 1.0000, 0.06) } else { Self::SKELETON_BG_DEFAULT };
        runtime_or("SKELETON_BG", fallback)
    }
    pub const SKELETON_BORDER_DEFAULT: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.06);
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn SKELETON_BORDER() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.0000, 0.0000, 1.0000, 0.10) } else { Self::SKELETON_BORDER_DEFAULT };
        runtime_or("SKELETON_BORDER", fallback)
    }

    // ── 徽章与标签 ──
    pub const BADGE_BG_DEFAULT: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.05); // --hover-bg-strong 派生的浅标签底
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn BADGE_BG() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.0000, 0.0000, 1.0000, 0.08) } else { Self::BADGE_BG_DEFAULT };
        runtime_or("BADGE_BG", fallback)
    }
    pub const ITEM_SLOT_BG_DEFAULT: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.05);
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn ITEM_SLOT_BG() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.0000, 0.0000, 1.0000, 0.08) } else { Self::ITEM_SLOT_BG_DEFAULT };
        runtime_or("ITEM_SLOT_BG", fallback)
    }
    pub const ITEM_SLOT_BORDER_DEFAULT: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.08);
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn ITEM_SLOT_BORDER() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.0000, 0.0000, 1.0000, 0.12) } else { Self::ITEM_SLOT_BORDER_DEFAULT };
        runtime_or("ITEM_SLOT_BORDER", fallback)
    }

    // ── 阵营与对局信息（GameInfo）色板 ──
    pub const BLUE_SIDE_ACCENT_DEFAULT: Hsla = const_hsla(205.0 / 360.0, 0.90, 0.52, 1.0); // 蓝方极光蓝
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn BLUE_SIDE_ACCENT() -> Hsla {
        let fallback = Self::BLUE_SIDE_ACCENT_DEFAULT;
        runtime_or("BLUE_SIDE_ACCENT", fallback)
    }
    pub const BLUE_SIDE_BG_DEFAULT: Hsla = const_hsla(205.0 / 360.0, 0.70, 0.96, 0.90);
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn BLUE_SIDE_BG() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.5694, 0.6420, 0.3098, 0.90) } else { Self::BLUE_SIDE_BG_DEFAULT };
        runtime_or("BLUE_SIDE_BG", fallback)
    }
    pub const BLUE_SIDE_BORDER_DEFAULT: Hsla = const_hsla(205.0 / 360.0, 0.60, 0.85, 0.85);
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn BLUE_SIDE_BORDER() -> Hsla {
        let fallback = Self::BLUE_SIDE_BORDER_DEFAULT;
        runtime_or("BLUE_SIDE_BORDER", fallback)
    }

    pub const RED_SIDE_ACCENT_DEFAULT: Hsla = const_hsla(350.0 / 360.0, 0.85, 0.55, 1.0); // 红方极光绯红
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn RED_SIDE_ACCENT() -> Hsla {
        let fallback = Self::RED_SIDE_ACCENT_DEFAULT;
        runtime_or("RED_SIDE_ACCENT", fallback)
    }
    pub const RED_SIDE_BG_DEFAULT: Hsla = const_hsla(350.0 / 360.0, 0.70, 0.97, 0.90);
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn RED_SIDE_BG() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.9722, 0.5800, 0.3100, 0.90) } else { Self::RED_SIDE_BG_DEFAULT };
        runtime_or("RED_SIDE_BG", fallback)
    }
    pub const RED_SIDE_BORDER_DEFAULT: Hsla = const_hsla(350.0 / 360.0, 0.50, 0.86, 0.85);
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn RED_SIDE_BORDER() -> Hsla {
        let fallback = Self::RED_SIDE_BORDER_DEFAULT;
        runtime_or("RED_SIDE_BORDER", fallback)
    }

    // ── 组队主题强调色（高辨识度色板） ──
    pub const PREMADE_COLOR_1_DEFAULT: Hsla = const_hsla(160.0 / 360.0, 0.90, 0.38, 1.0); // 翡翠青
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn PREMADE_COLOR_1() -> Hsla {
        let fallback = Self::PREMADE_COLOR_1_DEFAULT;
        runtime_or("PREMADE_COLOR_1", fallback)
    }
    pub const PREMADE_COLOR_2_DEFAULT: Hsla = const_hsla(38.0 / 360.0, 0.95, 0.45, 1.0); // 琥珀金
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn PREMADE_COLOR_2() -> Hsla {
        let fallback = Self::PREMADE_COLOR_2_DEFAULT;
        runtime_or("PREMADE_COLOR_2", fallback)
    }
    pub const PREMADE_COLOR_3_DEFAULT: Hsla = const_hsla(270.0 / 360.0, 0.80, 0.55, 1.0); // 罗兰紫
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn PREMADE_COLOR_3() -> Hsla {
        let fallback = Self::PREMADE_COLOR_3_DEFAULT;
        runtime_or("PREMADE_COLOR_3", fallback)
    }
    pub const PREMADE_COLOR_4_DEFAULT: Hsla = const_hsla(15.0 / 360.0, 0.95, 0.52, 1.0); // 珊瑚橙
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn PREMADE_COLOR_4() -> Hsla {
        let fallback = Self::PREMADE_COLOR_4_DEFAULT;
        runtime_or("PREMADE_COLOR_4", fallback)
    }
    pub const GOLD_DEFAULT: Hsla = const_hsla(0.1047, 0.9213, 0.5020, 1.0); // --accent-color → #f59e0b
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn GOLD() -> Hsla {
        let fallback = Self::GOLD_DEFAULT;
        runtime_or("GOLD", fallback)
    }
    pub const HOVER_BG_DEFAULT: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.03); // --hover-bg
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn HOVER_BG() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.0000, 0.0000, 1.0000, 0.06) } else { Self::HOVER_BG_DEFAULT };
        runtime_or("HOVER_BG", fallback)
    }
    pub const HOVER_BG_STRONG_DEFAULT: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.06); // --hover-bg-strong
    /// 运行时色板查询（深色模式/主题色/胜负卡色覆盖），未命中回落默认值
    #[allow(dead_code)]
    pub fn HOVER_BG_STRONG() -> Hsla {
        let fallback = if is_dark_mode_active() { const_hsla(0.0000, 0.0000, 1.0000, 0.10) } else { Self::HOVER_BG_STRONG_DEFAULT };
        runtime_or("HOVER_BG_STRONG", fallback)
    }

    pub fn premade_color(idx: usize) -> Hsla {
        match idx % 4 {
            0 => Self::PREMADE_COLOR_1(),
            1 => Self::PREMADE_COLOR_2(),
            2 => Self::PREMADE_COLOR_3(),
            _ => Self::PREMADE_COLOR_4(),
        }
    }

    // ── 字体族设定 ──
    pub fn font_family() -> SharedString {
        "Microsoft YaHei UI".into()
    }

    /// 解析 6 位 (#RRGGBB) 或 8 位 (#AARRGGBB) 十六进制颜色字符为 gpui::Rgba
    pub fn parse_hex(hex: &str) -> Rgba {
        parse_hex_color(hex)
    }

    /// 根据用户个性化配置获取品牌强调色（未配置则返回默认极光青）
    pub fn accent_color(config: &yuumi_core::config::AppConfig) -> Rgba {
        parse_hex_color(&config.personalization.theme_color)
    }

    /// 根据用户个性化配置获取胜利对局卡片底色
    pub fn win_card_color(config: &yuumi_core::config::AppConfig) -> Rgba {
        parse_hex_color(&config.personalization.win_card_color)
    }

    /// 根据用户个性化配置获取失败对局卡片底色
    pub fn lose_card_color(config: &yuumi_core::config::AppConfig) -> Rgba {
        parse_hex_color(&config.personalization.lose_card_color)
    }

    /// 根据用户个性化配置获取重开对局卡片底色
    pub fn remake_card_color(config: &yuumi_core::config::AppConfig) -> Rgba {
        parse_hex_color(&config.personalization.remake_card_color)
    }

    /// 根据用户个性化配置获取死亡数字高亮色
    pub fn death_color(config: &yuumi_core::config::AppConfig, is_dark: bool) -> Rgba {
        if is_dark {
            parse_hex_color(&config.personalization.dark_deaths_number_color)
        } else {
            parse_hex_color(&config.personalization.light_deaths_number_color)
        }
    }

    /// 按个性化配置重建运行时色板。应用启动与设置页相关项保存后调用。
    ///
    /// `system_dark` 为当前 Windows 系统深色偏好（Auto 模式使用）。
    pub fn apply(personalization: &yuumi_core::config::PersonalizationConfig, system_dark: bool) {
        let is_dark = match personalization.theme_mode.as_str() {
            "Dark" => true,
            "Light" => false,
            _ => system_dark,
        };
        *DARK_MODE_ACTIVE.write().unwrap_or_else(|e| e.into_inner()) = is_dark;

        let mut map: HashMap<&'static str, Hsla> = HashMap::new();

        // 主题色派生：强调色系共享用户配置的品牌色（默认极光青 #009faa）
        let accent = Hsla::from(parse_hex_color(&personalization.theme_color));
        map.insert("ACCENT_PRIMARY", accent);
        map.insert("ACCENT_HOVER", Hsla { l: (accent.l + 0.02).min(1.0), ..accent });
        map.insert("ACCENT_LIGHT", Hsla { a: 0.15, ..accent });
        map.insert("ACCENT_ALPHA_10", Hsla { a: 0.10, ..accent });
        map.insert("ACCENT_ALPHA_20", Hsla { a: 0.20, ..accent });
        map.insert("SIDEBAR_ACTIVE", Hsla { a: 0.15, ..accent });
        map.insert("SIDEBAR_ACTIVE_BORDER", Hsla { a: 0.30, ..accent });
        map.insert("BORDER_HOVER", Hsla { a: 0.35, ..accent });
        map.insert("TEXT_ACCENT", accent);

        // 胜负 / 重开卡片底色（#AARRGGBB 自带透明度，未配置时 parse 回落默认极光色）
        let win = Hsla::from(parse_hex_color(&personalization.win_card_color));
        map.insert("WIN_BG", win);
        map.insert("WIN_BG_HOVER", Hsla { a: (win.a * 1.6).min(1.0), ..win });
        map.insert("WIN_BORDER", Hsla { a: (win.a * 2.0).min(1.0), ..win });

        let lose = Hsla::from(parse_hex_color(&personalization.lose_card_color));
        map.insert("LOSS_BG", lose);
        map.insert("LOSS_BG_HOVER", Hsla { a: (lose.a * 1.6).min(1.0), ..lose });
        map.insert("LOSS_BORDER", Hsla { a: (lose.a * 2.0).min(1.0), ..lose });

        let remake = Hsla::from(parse_hex_color(&personalization.remake_card_color));
        map.insert("REMAKE_BG", remake);
        map.insert("REMAKE_BORDER", Hsla { a: (remake.a * 2.0).min(1.0), ..remake });

        // 死亡数字颜色按当前主题取对应配置
        let death = if is_dark {
            parse_hex_color(&personalization.dark_deaths_number_color)
        } else {
            parse_hex_color(&personalization.light_deaths_number_color)
        };
        map.insert("DEATH_RED", Hsla::from(death));

        *RUNTIME_COLORS.write().unwrap_or_else(|e| e.into_inner()) = map;
    }
}

/// 解析 6 位或 8 位十六进制颜色字符为 gpui::Rgba
pub fn parse_hex_color(hex: &str) -> Rgba {
    let s = hex.trim().trim_start_matches('#');
    if s.len() == 6 {
        if let Ok(val) = u32::from_str_radix(s, 16) {
            return rgba((val << 8) | 0xff);
        }
    } else if s.len() == 8 {
        // Vue: #AARRGGBB
        if let Ok(val) = u32::from_str_radix(s, 16) {
            let a = (val >> 24) & 0xff;
            let rgb = val & 0x00ffffff;
            return rgba((rgb << 8) | a);
        }
    }
    rgba(0x009faaff)
}

// 兼容别名与全局快捷常量（历史遗留死代码，随主题层改造成函数形态保留）
#[allow(dead_code)]
pub fn BG_MAIN() -> Hsla { Theme::BG_APP() }
#[allow(dead_code)]
pub fn BG_CARD() -> Hsla { Theme::CARD_BG() }
#[allow(dead_code)]
pub fn BG_CARD_HOVER() -> Hsla { Theme::CARD_BG_HOVER() }
#[allow(dead_code)]
pub fn BG_ELEVATED() -> Hsla { Theme::CARD_BG_HOVER() }
#[allow(dead_code)]
pub fn PRIMARY() -> Hsla { Theme::ACCENT_PRIMARY() }
#[allow(dead_code)]
pub fn TEXT_PRIMARY() -> Hsla { Theme::TEXT_PRIMARY() }
#[allow(dead_code)]
pub fn TEXT_SECONDARY() -> Hsla { Theme::TEXT_SECONDARY() }
#[allow(dead_code)]
pub fn TEXT_MUTED() -> Hsla { Theme::TEXT_MUTED() }
#[allow(dead_code)]
pub fn BORDER_SECONDARY() -> Hsla { Theme::CARD_BORDER() }
#[allow(dead_code)]
pub fn WIN_PRIMARY() -> Hsla { Theme::WIN_BAR() }
#[allow(dead_code)]
pub fn WIN_MUTED() -> Hsla { Theme::WIN_BG() }
#[allow(dead_code)]
pub fn WIN_BORDER() -> Hsla { Theme::WIN_BORDER() }
#[allow(dead_code)]
pub fn LOSS_PRIMARY() -> Hsla { Theme::LOSS_BAR() }
#[allow(dead_code)]
pub fn LOSS_MUTED() -> Hsla { Theme::LOSS_BG() }
#[allow(dead_code)]
pub fn PRIMARY_MUTED() -> Hsla { Theme::ACCENT_LIGHT() }

#[cfg(test)]
mod tests {
    // 精确导入：theme.rs 顶层 use gpui::* 会经 super::* 把 gpui::test 属性宏
    // 带进作用域遮蔽标准 #[test]，导致宏展开递归溢出
    use super::{is_dark_mode_active, Theme};
    use std::sync::Mutex;
    use yuumi_core::config::PersonalizationConfig;

    /// Theme::apply 写全局色板，两个测试并行时必须串行
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// 测试结束恢复默认色板，避免污染同进程其他测试
    fn restore_default_palette() {
        let p = PersonalizationConfig::default();
        Theme::apply(&p, false);
    }

    #[test]
    fn theme_apply_switches_palettes_between_modes() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        restore_default_palette();
        // Light：中性色保持浅色默认
        let mut p = PersonalizationConfig {
            theme_mode: "Light".into(),
            ..PersonalizationConfig::default()
        };
        Theme::apply(&p, false);
        assert_eq!(Theme::BG_APP(), Theme::BG_APP_DEFAULT);
        assert!(!is_dark_mode_active());

        // Dark：中性色切换到深色覆盖表
        p.theme_mode = "Dark".into();
        Theme::apply(&p, false);
        assert_ne!(Theme::BG_APP(), Theme::BG_APP_DEFAULT);
        assert!(is_dark_mode_active());

        // Auto：跟随系统深色偏好
        p.theme_mode = "Auto".into();
        Theme::apply(&p, true);
        assert_ne!(Theme::BG_APP(), Theme::BG_APP_DEFAULT);
        Theme::apply(&p, false);
        assert_eq!(Theme::BG_APP(), Theme::BG_APP_DEFAULT);

        restore_default_palette();
    }

    #[test]
    fn theme_apply_derives_accent_from_theme_color() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        restore_default_palette();
        let mut p = PersonalizationConfig {
            theme_color: "#ff0000".into(),
            ..PersonalizationConfig::default()
        };
        Theme::apply(&p, false);

        let accent = Theme::ACCENT_PRIMARY();
        // 红 hue ≈ 0（Hsla::from(Rgba) 的 hue 归一在 0..1）
        assert!(accent.h < 0.05 || accent.h > 0.95, "红主题色 hue 应接近 0");
        // 派生系共享同 hue，透明度按语义递减
        let light = Theme::ACCENT_LIGHT();
        assert!((light.a - 0.15).abs() < 0.01);
        assert!((light.h - accent.h).abs() < 1e-6);

        // 胜利卡片底色来自配置（alpha 0x66 > 0.35 应被解析）
        p.win_card_color = "#6639B01B".into();
        Theme::apply(&p, false);
        assert!(Theme::WIN_BG().a > 0.35);

        restore_default_palette();
    }
}
