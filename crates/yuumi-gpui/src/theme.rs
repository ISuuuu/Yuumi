use gpui::*;

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
    pub const BG_APP: Hsla = const_hsla(0.5833, 0.4000, 0.9804, 1.0); // --bg-color → #f8fafc
    pub const SIDEBAR_BG: Hsla = const_hsla(0.0000, 0.0000, 1.0000, 0.75); // --sidebar-bg → rgba(255,255,255,0.75)
    pub const SIDEBAR_HOVER: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.03); // --hover-bg
    pub const SIDEBAR_ACTIVE: Hsla = const_hsla(0.5108, 1.0000, 0.3333, 0.15); // --primary-color-alpha-15
    pub const SIDEBAR_ACTIVE_BORDER: Hsla = const_hsla(0.5108, 1.0000, 0.3333, 0.30); // --primary-color-alpha-30
    pub const TITLEBAR_BG: Hsla = const_hsla(0.0000, 0.0000, 1.0000, 0.85); // --titlebar-bg
    pub const CARD_BG: Hsla = const_hsla(0.0000, 0.0000, 1.0000, 0.75); // --card-bg
    pub const CARD_BG_HOVER: Hsla = const_hsla(0.0000, 0.0000, 1.0000, 0.95); // --card-bg-hover
    pub const CARD_BORDER: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.06); // --border-color
    pub const BORDER_HOVER: Hsla = const_hsla(0.5108, 1.0000, 0.3333, 0.35); // --border-color-hover

    // ── 品牌色与高亮 ──
    pub const ACCENT_PRIMARY: Hsla = const_hsla(0.5108, 1.0000, 0.3333, 1.0); // --primary-color → #009faa
    pub const ACCENT_HOVER: Hsla = const_hsla(0.4888, 1.0000, 0.3510, 1.0); // --primary-color-hover → #00b3a7
    pub const ACCENT_LIGHT: Hsla = const_hsla(0.5108, 1.0000, 0.3333, 0.15); // --primary-color-alpha-15
    /// 兼容旧实现的极光强调色，已收敛到品牌青的亮阶，避免霓虹偏色
    pub const AURORA_CYAN: Hsla = const_hsla(0.5102, 1.0000, 0.3843, 1.0); // #00b8c4
    pub const AURORA_BLUE: Hsla = const_hsla(0.5512, 0.9320, 0.5961, 1.0); // #38bdf8

    // ── 胜利状态（--win-*）──
    pub const WIN_BG: Hsla = const_hsla(0.2998, 0.7340, 0.3980, 0.20); // --win-bg → rgba(57,176,27,0.20)
    pub const WIN_BG_HOVER: Hsla = const_hsla(0.2998, 0.7340, 0.3980, 0.32); // --win-bg-hover
    pub const WIN_BORDER: Hsla = const_hsla(0.2998, 0.7340, 0.3980, 0.40); // --win-border
    pub const WIN_BAR: Hsla = const_hsla(0.2998, 0.7340, 0.3980, 1.0);
    pub const WIN_TEXT: Hsla = const_hsla(0.4532, 0.8812, 0.1980, 1.0); // --win-color → #065f46

    // ── 失败状态（--loss-*）──
    pub const LOSS_BG: Hsla = const_hsla(0.0109, 0.8924, 0.4373, 0.20); // --loss-bg → rgba(211,25,12,0.20)
    pub const LOSS_BG_HOVER: Hsla = const_hsla(0.0109, 0.8924, 0.4373, 0.32); // --loss-bg-hover
    pub const LOSS_BORDER: Hsla = const_hsla(0.0109, 0.8924, 0.4373, 0.40); // --loss-border
    pub const LOSS_BAR: Hsla = const_hsla(0.0109, 0.8924, 0.4373, 1.0);
    pub const LOSS_TEXT: Hsla = const_hsla(0.9539, 0.7966, 0.3471, 1.0); // --loss-color → #9f1239

    // ── 重开状态（--remake-*）──
    pub const REMAKE_BG: Hsla = const_hsla(0.5972, 0.2022, 0.6510, 0.14); // --remake-bg
    pub const REMAKE_BORDER: Hsla = const_hsla(0.5972, 0.2022, 0.6510, 0.30); // --remake-border
    pub const REMAKE_BAR: Hsla = const_hsla(0.5972, 0.2022, 0.6510, 1.0);
    pub const REMAKE_TEXT: Hsla = const_hsla(0.5983, 0.1632, 0.4686, 1.0); // --remake-color → #64748b

    // ── 文字颜色分层（--text-color / --text-muted / --text-dimmed）──
    pub const TEXT_PRIMARY: Hsla = const_hsla(0.6173, 0.4737, 0.1118, 1.0); // #0f172a
    pub const TEXT_SECONDARY: Hsla = const_hsla(0.5980, 0.1932, 0.3451, 1.0); // #475569
    pub const TEXT_MUTED: Hsla = const_hsla(0.5983, 0.1632, 0.4686, 1.0); // #64748b
    pub const TEXT_ACCENT: Hsla = const_hsla(0.5108, 1.0000, 0.3333, 1.0);

    // ── 徽章与标签 ──
    pub const BADGE_BG: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.05); // --hover-bg-strong 派生的浅标签底
    pub const ITEM_SLOT_BG: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.05);
    pub const ITEM_SLOT_BORDER: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.08);

    // ── 阵营与对局信息（GameInfo）色板 ──
    pub const BLUE_SIDE_ACCENT: Hsla = const_hsla(205.0 / 360.0, 0.90, 0.52, 1.0); // 蓝方极光蓝
    pub const BLUE_SIDE_BG: Hsla = const_hsla(205.0 / 360.0, 0.70, 0.96, 0.90);
    pub const BLUE_SIDE_BORDER: Hsla = const_hsla(205.0 / 360.0, 0.60, 0.85, 0.85);

    pub const RED_SIDE_ACCENT: Hsla = const_hsla(350.0 / 360.0, 0.85, 0.55, 1.0); // 红方极光绯红
    pub const RED_SIDE_BG: Hsla = const_hsla(350.0 / 360.0, 0.70, 0.97, 0.90);
    pub const RED_SIDE_BORDER: Hsla = const_hsla(350.0 / 360.0, 0.50, 0.86, 0.85);

    // ── 组队主题强调色（高辨识度色板） ──
    pub const PREMADE_COLOR_1: Hsla = const_hsla(160.0 / 360.0, 0.90, 0.38, 1.0); // 翡翠青
    pub const PREMADE_COLOR_2: Hsla = const_hsla(38.0 / 360.0, 0.95, 0.45, 1.0); // 琥珀金
    pub const PREMADE_COLOR_3: Hsla = const_hsla(270.0 / 360.0, 0.80, 0.55, 1.0); // 罗兰紫
    pub const PREMADE_COLOR_4: Hsla = const_hsla(15.0 / 360.0, 0.95, 0.52, 1.0); // 珊瑚橙
    pub const GOLD: Hsla = const_hsla(0.1047, 0.9213, 0.5020, 1.0); // --accent-color → #f59e0b
    pub const HOVER_BG: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.03); // --hover-bg
    pub const HOVER_BG_STRONG: Hsla = const_hsla(0.0000, 0.0000, 0.0000, 0.06); // --hover-bg-strong

    pub fn premade_color(idx: usize) -> Hsla {
        match idx % 4 {
            0 => Self::PREMADE_COLOR_1,
            1 => Self::PREMADE_COLOR_2,
            2 => Self::PREMADE_COLOR_3,
            _ => Self::PREMADE_COLOR_4,
        }
    }

    // ── 字体族设定 ──
    pub fn font_family() -> SharedString {
        "Microsoft YaHei UI, Segoe UI, PingFang SC, sans-serif".into()
    }
}

// 兼容别名与全局快捷常量
pub const BG_MAIN: Hsla = Theme::BG_APP;
pub const BG_CARD: Hsla = Theme::CARD_BG;
pub const BG_CARD_HOVER: Hsla = Theme::CARD_BG_HOVER;
pub const BG_ELEVATED: Hsla = Theme::CARD_BG_HOVER;
pub const BG_INPUT: Hsla = const_hsla(0.0000, 0.0000, 1.0000, 0.95); // --popover-bg 近似
pub const PRIMARY: Hsla = Theme::ACCENT_PRIMARY;
pub const TEXT_PRIMARY: Hsla = Theme::TEXT_PRIMARY;
pub const TEXT_SECONDARY: Hsla = Theme::TEXT_SECONDARY;
pub const TEXT_MUTED: Hsla = Theme::TEXT_MUTED;
pub const TEXT_ON_PRIMARY: Hsla = const_hsla(0.0, 0.0, 1.0, 1.0);
pub const BORDER_SECONDARY: Hsla = Theme::CARD_BORDER;
pub const WIN_PRIMARY: Hsla = Theme::WIN_BAR;
pub const WIN_MUTED: Hsla = Theme::WIN_BG;
pub const WIN_BORDER: Hsla = Theme::WIN_BORDER;
pub const LOSS_PRIMARY: Hsla = Theme::LOSS_BAR;
pub const LOSS_MUTED: Hsla = Theme::LOSS_BG;
pub const PRIMARY_MUTED: Hsla = Theme::ACCENT_LIGHT;
pub const GOLD: Hsla = Theme::GOLD;
