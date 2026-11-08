use gpui::*;

const fn const_hsla(h: f32, s: f32, l: f32, a: f32) -> Hsla {
    Hsla { h, s, l, a }
}

/// 纯白水晶极光（Crystal Aurora）主题色板
pub struct Theme;

#[allow(dead_code)]
impl Theme {
    // ── 基础背景与面板 ──
    pub const BG_APP: Hsla = const_hsla(210.0 / 360.0, 0.20, 0.98, 1.0); // 冷白底色 #f8fafc
    pub const CARD_BG: Hsla = const_hsla(0.0, 0.0, 1.0, 0.78); // 半透明纯白卡片
    pub const CARD_BG_HOVER: Hsla = const_hsla(0.0, 0.0, 1.0, 0.92);
    pub const CARD_BORDER: Hsla = const_hsla(214.0 / 360.0, 0.32, 0.91, 0.85); // 柔和冷灰边框 #e2e8f0

    // ── 极光品牌色与高亮 ──
    pub const AURORA_CYAN: Hsla = const_hsla(168.0 / 360.0, 1.0, 0.48, 1.0); // 极光青 #00f5d4
    pub const AURORA_BLUE: Hsla = const_hsla(195.0 / 360.0, 1.0, 0.49, 1.0); // 极光蓝 #00bbf9
    pub const ACCENT_PRIMARY: Hsla = const_hsla(174.0 / 360.0, 0.85, 0.36, 1.0); // 主交互青色 #0d9488
    pub const ACCENT_LIGHT: Hsla = const_hsla(168.0 / 360.0, 0.80, 0.95, 0.9);

    // ── 胜利状态（清爽极光蓝/绿低饱和微透） ──
    pub const WIN_BG: Hsla = const_hsla(168.0 / 360.0, 0.75, 0.96, 0.88);
    pub const WIN_BG_HOVER: Hsla = const_hsla(168.0 / 360.0, 0.75, 0.93, 0.95);
    pub const WIN_BORDER: Hsla = const_hsla(168.0 / 360.0, 0.60, 0.80, 0.85);
    pub const WIN_BAR: Hsla = const_hsla(168.0 / 360.0, 0.90, 0.42, 1.0);
    pub const WIN_TEXT: Hsla = const_hsla(174.0 / 360.0, 0.85, 0.28, 1.0);

    // ── 失败状态（温和微透绯红低饱和） ──
    pub const LOSS_BG: Hsla = const_hsla(350.0 / 360.0, 0.70, 0.97, 0.88);
    pub const LOSS_BG_HOVER: Hsla = const_hsla(350.0 / 360.0, 0.70, 0.94, 0.95);
    pub const LOSS_BORDER: Hsla = const_hsla(350.0 / 360.0, 0.50, 0.86, 0.85);
    pub const LOSS_BAR: Hsla = const_hsla(350.0 / 360.0, 0.80, 0.55, 1.0);
    pub const LOSS_TEXT: Hsla = const_hsla(350.0 / 360.0, 0.75, 0.38, 1.0);

    // ── 重开状态（温和中性灰微透） ──
    pub const REMAKE_BG: Hsla = const_hsla(215.0 / 360.0, 0.15, 0.95, 0.88);
    pub const REMAKE_BORDER: Hsla = const_hsla(215.0 / 360.0, 0.15, 0.85, 0.85);
    pub const REMAKE_BAR: Hsla = const_hsla(215.0 / 360.0, 0.20, 0.60, 1.0);
    pub const REMAKE_TEXT: Hsla = const_hsla(215.0 / 360.0, 0.20, 0.40, 1.0);

    // ── 文字颜色分层 ──
    pub const TEXT_PRIMARY: Hsla = const_hsla(222.0 / 360.0, 0.47, 0.11, 1.0); // 深黑灰 #0f172a
    pub const TEXT_SECONDARY: Hsla = const_hsla(215.0 / 360.0, 0.25, 0.35, 1.0); // 中灰 #475569
    pub const TEXT_MUTED: Hsla = const_hsla(215.0 / 360.0, 0.16, 0.58, 1.0); // 浅灰 #94a3b8
    pub const TEXT_ACCENT: Hsla = const_hsla(174.0 / 360.0, 0.85, 0.32, 1.0);

    // ── 徽章与标签 ──
    pub const BADGE_BG: Hsla = const_hsla(210.0 / 360.0, 0.25, 0.93, 0.90);
    pub const ITEM_SLOT_BG: Hsla = const_hsla(214.0 / 360.0, 0.20, 0.90, 0.70);
    pub const ITEM_SLOT_BORDER: Hsla = const_hsla(214.0 / 360.0, 0.25, 0.82, 0.80);

    // ── 阵营与对局信息（GameInfo）色板 ──
    pub const BLUE_SIDE_ACCENT: Hsla = const_hsla(205.0 / 360.0, 0.90, 0.52, 1.0); // 蓝方极光蓝
    pub const BLUE_SIDE_BG: Hsla = const_hsla(205.0 / 360.0, 0.70, 0.96, 0.90);
    pub const BLUE_SIDE_BORDER: Hsla = const_hsla(205.0 / 360.0, 0.60, 0.85, 0.85);

    pub const RED_SIDE_ACCENT: Hsla = const_hsla(350.0 / 360.0, 0.85, 0.55, 1.0); // 红方极光绯红
    pub const RED_SIDE_BG: Hsla = const_hsla(350.0 / 360.0, 0.70, 0.97, 0.90);
    pub const RED_SIDE_BORDER: Hsla = const_hsla(350.0 / 360.0, 0.50, 0.86, 0.85);

    // ── 组队主题强调色（高辨识度色板） ──
    pub const PREMADE_COLOR_1: Hsla = const_hsla(160.0 / 360.0, 0.90, 0.38, 1.0); // 翡翠青
    pub const PREMADE_COLOR_2: Hsla = const_hsla(38.0 / 360.0, 0.95, 0.45, 1.0);  // 琥珀金
    pub const PREMADE_COLOR_3: Hsla = const_hsla(270.0 / 360.0, 0.80, 0.55, 1.0); // 罗兰紫
    pub const PREMADE_COLOR_4: Hsla = const_hsla(15.0 / 360.0, 0.95, 0.52, 1.0);  // 珊瑚橙

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
