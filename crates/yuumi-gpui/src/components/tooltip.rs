//! 悬浮提示气泡（自绘）
//!
//! 为什么不用 GPUI 内置 tooltip：vendored gpui 的 Windows 后端不消费 `Window::set_tooltip`；
//! 而 `group_hover` 在本版本里也不可靠（命中框在 paint 阶段登记，样式在 prepaint 就计算完毕）。
//! 因此这里用「on_hover 写全局激活态 + cx.notify 重绘 + deferred 延迟绘制」实现：
//! 悬停时气泡由 deferred 在整棵树之后绘制，既不会被滚动容器裁剪，也不会被后面的兄弟节点盖住。

use std::cell::RefCell;

use gpui::prelude::FluentBuilder;
use gpui::*;

/// 气泡内的单行文本
#[derive(Clone)]
pub struct TooltipLine {
    pub text: String,
    pub color: Hsla,
    pub bold: bool,
}

impl TooltipLine {
    /// 标题行（白色加粗）
    pub fn title(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: rgb(0xf8fafc).into(),
            bold: true,
        }
    }

    /// 正文行（浅灰）
    pub fn body(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: rgb(0xe2e8f0).into(),
            bold: false,
        }
    }

    /// 次要行（灰蓝，用于时间/说明）
    pub fn muted(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: rgb(0x94a3b8).into(),
            bold: false,
        }
    }

    /// 自定义颜色正文行
    pub fn accent(text: impl Into<String>, color: Hsla) -> Self {
        Self {
            text: text.into(),
            color,
            bold: false,
        }
    }
}

thread_local! {
    /// 当前鼠标悬停的提示标识（UI 线程独占）
    static ACTIVE_TOOLTIP: RefCell<Option<SharedString>> = const { RefCell::new(None) };
    /// 承载这些提示的视图实体：鼠标事件里无法调用 `Window::current_view`，
    /// 只能在渲染期记录，事件里拿来触发重绘
    static HOST_VIEW: std::cell::Cell<Option<EntityId>> = const { std::cell::Cell::new(None) };
}

/// 记录承载悬浮提示的视图实体（主窗每次渲染时更新）
pub fn set_host_view(id: EntityId) {
    HOST_VIEW.with(|slot| slot.set(Some(id)));
}

fn is_active(id: &SharedString) -> bool {
    ACTIVE_TOOLTIP.with(|active| active.borrow().as_ref() == Some(id))
}

/// 深色气泡容器（亮色文字，与 OP.GG 提示风格一致）
fn bubble(lines: Vec<TooltipLine>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        // 不换行：宽度由最长的一行决定，既不折行也不留大片空白
        .whitespace_nowrap()
        .max_w(px(560.0))
        .px(px(8.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(Hsla::from(rgb(0x334155)))
        .bg(rgb(0x0f172a))
        .text_size(px(10.0))
        .children(lines.into_iter().map(|line| {
            div()
                .text_color(line.color)
                .when(line.bold, |el| el.font_weight(FontWeight::BOLD))
                .child(line.text)
        }))
}

/// 气泡底边距离触发器底边的像素距离（略大于最小触发器高度即可）
const BUBBLE_GAP: f32 = 20.0;

/// 给触发元素挂上悬浮气泡（显示在触发器上方）
pub fn with_tooltip(
    group: impl Into<SharedString>,
    trigger: impl IntoElement + 'static,
    lines: Vec<TooltipLine>,
) -> AnyElement {
    build_tooltip(group, trigger, lines, true, BUBBLE_GAP)
}

/// 给触发元素挂上悬浮气泡（显示在触发器下方，用于贴顶的列头）
pub fn with_tooltip_below(
    group: impl Into<SharedString>,
    trigger: impl IntoElement + 'static,
    lines: Vec<TooltipLine>,
) -> AnyElement {
    build_tooltip(group, trigger, lines, false, 0.0)
}

fn build_tooltip(
    group: impl Into<SharedString>,
    trigger: impl IntoElement + 'static,
    lines: Vec<TooltipLine>,
    above: bool,
    offset_y: f32,
) -> AnyElement {
    let id: SharedString = group.into();
    let visible = is_active(&id);

    let enter = id.clone();
    let leave = id.clone();

    div()
        .relative()
        .child(
            div()
                .id(id)
                .on_hover(move |is_hovered, _window, cx| {
                    if *is_hovered {
                        ACTIVE_TOOLTIP.with(|active| *active.borrow_mut() = Some(enter.clone()));
                    } else {
                        ACTIVE_TOOLTIP.with(|active| {
                            let mut active = active.borrow_mut();
                            if active.as_ref() == Some(&leave) {
                                *active = None;
                            }
                        });
                    }
                    // 不能在这里调用 window.current_view()（只在布局/绘制阶段可用），
                    // 用渲染期记录的宿主视图触发重绘
                    if let Some(view) = HOST_VIEW.with(|slot| slot.get()) {
                        cx.notify(view);
                    }
                })
                .child(trigger),
        )
        .when(visible, |el| {
            // deferred：整棵树绘制完之后再画，避免被列容器的 overflow_hidden 裁掉
            let bubble_el = if above {
                div().absolute().left_0().bottom(px(offset_y))
            } else {
                // top_full：紧贴触发器下沿（下拉菜单同款写法）
                div().absolute().left_0().top_full().mt(px(2.0))
            };
            el.child(deferred(bubble_el.child(bubble(lines))))
        })
        .into_any_element()
}
