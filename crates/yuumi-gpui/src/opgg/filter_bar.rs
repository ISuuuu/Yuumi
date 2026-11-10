//! OP.GG 筛选栏
//!
//! 对照 Vue `src/components/opgg/OpggFilterBar.vue`：顺序为区服、模式、段位、位置，
//! 位置仅排位模式显示，选中项高亮，点击外部可关闭下拉。
//!
//! Vue 的「点击外部关闭」由 `document` 上的全局 click 监听实现；GPUI 无全局监听，
//! 改由窗口根节点在空白区域按下时收起，触发器与菜单项各自阻止冒泡。

use crate::components::icon::icon;
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use yuumi_core::opgg::OpggMode;

/// 下拉种类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    Region,
    Mode,
    Tier,
    Position,
}

/// 当前筛选状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterState {
    pub region: &'static str,
    pub mode: OpggMode,
    pub tier: &'static str,
    pub position: &'static str,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            region: "kr",
            mode: OpggMode::Ranked,
            tier: "emerald_plus",
            position: "MID",
        }
    }
}

impl FilterState {
    /// 当前模式的接口取值
    pub fn mode_value(&self) -> &'static str {
        self.mode.as_str()
    }

    /// 详情请求使用的分路（仅排位模式携带）
    pub fn position_for_request(&self) -> Option<&'static str> {
        if self.mode.uses_position() {
            Some(self.position)
        } else {
            None
        }
    }
}

const REGIONS: &[(&str, &str)] = &[("kr", "韩服"), ("global", "全球")];

const MODES: &[(&str, &str)] = &[
    ("ranked", "排位模式"),
    ("normal", "匹配模式"),
    ("aram", "极地大乱斗"),
    ("arena", "斗魂竞技场"),
];

const TIERS: &[(&str, &str)] = &[
    ("all", "全部"),
    ("gold_plus", "黄金+"),
    ("platinum_plus", "铂金+"),
    ("emerald_plus", "翡翠+"),
    ("diamond_plus", "钻石+"),
    ("master", "大师"),
    ("master_plus", "大师+"),
    ("grandmaster", "宗师"),
    ("challenger", "王者"),
];

const POSITIONS: &[(&str, &str)] = &[
    ("TOP", "上单"),
    ("JUNGLE", "打野"),
    ("MID", "中单"),
    ("ADC", "下路 (ADC)"),
    ("SUPPORT", "辅助"),
];

/// 按值查显示名，未命中退回原值
fn label_of(options: &[(&'static str, &'static str)], value: &str) -> &'static str {
    options
        .iter()
        .find(|(key, _)| *key == value)
        .map(|(_, label)| *label)
        .unwrap_or("")
}

/// 单个筛选触发器（含下拉菜单）
fn trigger(
    id: &'static str,
    kind: FilterKind,
    options: &'static [(&'static str, &'static str)],
    current: &'static str,
    open: Option<FilterKind>,
    on_toggle: impl Fn(FilterKind, &mut Window, &mut App) + 'static,
    on_select: impl Fn(FilterKind, &'static str, &mut Window, &mut App) + 'static + Clone,
) -> impl IntoElement {
    let is_open = open == Some(kind);
    let toggle = on_toggle;
    let select = on_select;

    div()
        .id(id)
        .relative()
        .flex_none()
        .child(
            div()
                .id(SharedString::from(format!("{id}-trigger")))
                .flex()
                .items_center()
                .gap(px(4.0))
                .px(px(8.0))
                .py(px(4.0))
                .border_1()
                .border_color(if is_open {
                    Theme::ACCENT_PRIMARY
                } else {
                    Theme::CARD_BORDER
                })
                .rounded(px(6.0))
                .bg(Theme::CARD_BG)
                .text_size(px(12.48))
                .text_color(Theme::TEXT_PRIMARY)
                // 触发器文案不折行（对照 Vue `.filter-trigger` 的 inline-flex 单行排布）
                .whitespace_nowrap()
                .cursor_pointer()
                .hover(|style| style.border_color(Theme::ACCENT_PRIMARY))
                .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                    app.stop_propagation();
                    toggle(kind, window, app);
                })
                .child(label_of(options, current))
                .child(icon(
                    if is_open {
                        "icons/chevron-up.svg"
                    } else {
                        "icons/chevron-down.svg"
                    },
                    11.0,
                    Theme::TEXT_MUTED,
                )),
        )
        .when(is_open, |el| {
            el.child(
                // 浮层菜单：`deferred` 让菜单盖住后续兄弟节点（沿用项目 dropdown 的做法）
                deferred(
                    div()
                        .id(SharedString::from(format!("{id}-menu")))
                        .absolute()
                        .top_full()
                        .left(px(0.0))
                        .mt(px(4.0))
                        .flex()
                        .flex_col()
                        .py(px(4.0))
                        // Vue 用 `min-width: max-content` 让菜单按最长项撑开；
                        // GPUI 无 max-content，这里给一个足够容纳最长选项的宽度并禁止折行
                        .min_w(px(150.0))
                        .bg(Theme::CARD_BG_HOVER)
                        .border_1()
                        .border_color(Theme::CARD_BORDER)
                        .rounded(px(8.0))
                        .shadow_lg()
                        .on_mouse_down(MouseButton::Left, |_ev, _window, app| {
                            app.stop_propagation();
                        })
                        .children(options.iter().map(|(value, label)| {
                            let value: &'static str = *value;
                            let label: &'static str = *label;
                            let is_active = *value == *current;
                            let select = select.clone();
                            div()
                                .id(SharedString::from(format!("{id}-item-{value}")))
                                .px(px(14.0))
                                .py(px(6.0))
                                .text_size(px(12.48))
                                .whitespace_nowrap()
                                .when(is_active, |style| {
                                    style
                                        .text_color(Theme::ACCENT_PRIMARY)
                                        .font_weight(FontWeight::BOLD)
                                        .bg(Theme::ACCENT_LIGHT)
                                })
                                .when(!is_active, |style| {
                                    style.text_color(Theme::TEXT_SECONDARY)
                                })
                                .cursor_pointer()
                                .hover(|style| {
                                    style
                                        .bg(Theme::HOVER_BG)
                                        .text_color(Theme::TEXT_PRIMARY)
                                })
                                .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                                    app.stop_propagation();
                                    select(kind, value, window, app);
                                })
                                .child(label)
                        })),
                )
                .with_priority(1),
            )
        })
}

/// 筛选栏本体
pub fn filter_bar(
    state: FilterState,
    open: Option<FilterKind>,
    on_toggle: impl Fn(FilterKind, &mut Window, &mut App) + 'static + Clone,
    on_select: impl Fn(FilterKind, &'static str, &mut Window, &mut App) + 'static + Clone,
) -> impl IntoElement {
    let toggle_region = on_toggle.clone();
    let toggle_mode = on_toggle.clone();
    let toggle_tier = on_toggle.clone();
    let toggle_position = on_toggle;
    let select_region = on_select.clone();
    let select_mode = on_select.clone();
    let select_tier = on_select.clone();
    let select_position = on_select;

    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(16.0))
        .py(px(8.0))
        .border_b_1()
        .border_color(Theme::CARD_BORDER)
        .bg(Theme::BG_APP)
        .child(trigger(
            "opgg-filter-region",
            FilterKind::Region,
            REGIONS,
            state.region,
            open,
            toggle_region,
            select_region,
        ))
        .child(trigger(
            "opgg-filter-mode",
            FilterKind::Mode,
            MODES,
            state.mode_value(),
            open,
            toggle_mode,
            select_mode,
        ))
        .child(trigger(
            "opgg-filter-tier",
            FilterKind::Tier,
            TIERS,
            state.tier,
            open,
            toggle_tier,
            select_tier,
        ))
        // 位置筛选仅在排位模式出现
        .when(state.mode.uses_position(), |el| {
            el.child(trigger(
                "opgg-filter-position",
                FilterKind::Position,
                POSITIONS,
                state.position,
                open,
                toggle_position,
                select_position,
            ))
        })
}
