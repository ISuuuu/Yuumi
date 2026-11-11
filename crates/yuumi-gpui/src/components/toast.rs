// 窗口级 Toast 通知（对照 Vue NMessageProvider 默认行为：顶部居中、3 秒停留）。
//
// 每个窗口（主窗 / OP.GG 独立窗）各持一份 `Entity<ToastStack>`，在根渲染最后叠加浮层：
// 同时最多 3 条可见，其余排队，计时从可见时开始；每条独立 ID 到期移除，
// 避免「旧消息到期误删新消息」。浮层不注册鼠标监听、不请求焦点，
// 空白处与条目本身均可点击穿透。
use std::collections::VecDeque;
use std::time::Duration;

use gpui::{
    div, px, Context, FontWeight, IntoElement, ParentElement, Pixels, SharedString, Styled,
};

use crate::theme::Theme;

/// 同时最多可见的条数，其余排队
const MAX_VISIBLE: usize = 3;
/// 单条停留时间（对照 Vue NMessageProvider 默认 3 秒）
const TOAST_DURATION: Duration = Duration::from_secs(3);
/// 浮层顶部偏移（标题栏 38px 下方）
const TOP_OFFSET: Pixels = px(46.0);
/// 单条最大宽度，超出自动换行
const MAX_PILL_WIDTH: Pixels = px(720.0);

/// 提示级别（对照 Vue showToast 的 success/error/warning/info 四级）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
    Warning,
    Info,
}

impl ToastKind {
    fn color(self) -> gpui::Hsla {
        match self {
            ToastKind::Success => Theme::WIN_BAR(),
            ToastKind::Error => Theme::LOSS_BAR(),
            ToastKind::Warning => Theme::GOLD(),
            ToastKind::Info => Theme::ACCENT_PRIMARY(),
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            ToastKind::Success => "✓",
            ToastKind::Error => "✕",
            ToastKind::Warning => "⚠",
            ToastKind::Info => "ℹ",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Toast {
    id: u64,
    kind: ToastKind,
    message: SharedString,
}

/// 纯队列簿记（不触 GPUI），便于脱离窗口单测排队/补位/过期规则。
struct ToastQueue {
    visible: VecDeque<Toast>,
    pending: VecDeque<Toast>,
    next_id: u64,
}

/// `enqueue` 的结果：`visible_at` 为 Some 表示立即上屏，需按该 id 起计时器。
struct Enqueued {
    id: u64,
    visible_at: Option<u64>,
}

impl ToastQueue {
    fn new() -> Self {
        Self {
            visible: VecDeque::new(),
            pending: VecDeque::new(),
            next_id: 0,
        }
    }

    fn enqueue(&mut self, kind: ToastKind, message: impl Into<SharedString>) -> Enqueued {
        let id = self.next_id;
        self.next_id += 1;
        let toast = Toast {
            id,
            kind,
            message: message.into(),
        };
        if self.visible.len() < MAX_VISIBLE {
            self.visible.push_back(toast);
            Enqueued {
                id,
                visible_at: Some(id),
            }
        } else {
            self.pending.push_back(toast);
            Enqueued {
                id,
                visible_at: None,
            }
        }
    }

    /// 到期移除指定条目；空出的槽位从队列补位，补位条目的 id 随结果返回（需起新计时器）。
    /// 返回 None 表示 id 不存在（旧计时器迟到/已清理），调用方无需处理。
    fn expire(&mut self, id: u64) -> Option<u64> {
        let removed = self.visible.len();
        self.visible.retain(|t| t.id != id);
        if self.visible.len() == removed {
            return None;
        }
        let promoted = self.pending.pop_front().map(|t| {
            let promoted_id = t.id;
            self.visible.push_back(t);
            promoted_id
        });
        Some(promoted.unwrap_or(id))
    }
}

/// 每窗口一份的 Toast 队列状态（独立 Entity，便于服务层经 WeakEntity 异步回传）
pub struct ToastStack {
    queue: ToastQueue,
}

impl ToastStack {
    pub fn new() -> Self {
        Self {
            queue: ToastQueue::new(),
        }
    }

    /// 入队一条提示。有空位立即可见并开始计时，否则排队等待补位。
    pub fn push(
        &mut self,
        kind: ToastKind,
        message: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> u64 {
        let enqueued = self.queue.enqueue(kind, message);
        if let Some(id) = enqueued.visible_at {
            self.schedule_expire(id, cx);
        }
        cx.notify();
        enqueued.id
    }

    /// 到期移除指定条目；空出的槽位从队列补位，新条目从补位时刻开始计时。
    fn expire(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.queue.expire(id).is_some() {
            cx.notify();
        }
    }

    fn schedule_expire(&mut self, id: u64, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_DURATION).await;
            let _ = this.update(cx, |stack, cx| stack.expire(id, cx));
        })
        .detach();
    }
}

/// 根层浮层：顶部居中纵向堆叠，容器与条目均不拦截鼠标事件。
pub fn toast_overlay(stack: &ToastStack) -> impl IntoElement {
    let toasts: Vec<Toast> = stack.queue.visible.iter().cloned().collect();
    div()
        .absolute()
        .top(TOP_OFFSET)
        .left(px(0.0))
        .right(px(0.0))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.0))
        .children(toasts.into_iter().map(|toast| {
            let color = toast.kind.color();
            let glyph = toast.kind.glyph();
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .px(px(14.0))
                .py(px(9.0))
                .rounded(Theme::RADIUS_MD)
                .max_w(MAX_PILL_WIDTH)
                .bg(Theme::CARD_BG_HOVER())
                .border_1()
                .border_color(Theme::CARD_BORDER())
                .shadow_md()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(color)
                        .child(glyph),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(Theme::TEXT_PRIMARY())
                        .child(toast.message),
                )
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_caps_visible_at_three() {
        let mut queue = ToastQueue::new();
        let mut ids = Vec::new();
        for msg in ["a", "b", "c", "d", "e"] {
            let enqueued = queue.enqueue(ToastKind::Info, msg);
            ids.push(enqueued.id);
            assert_eq!(
                enqueued.visible_at.is_some(),
                enqueued.id < 3,
                "第 {} 条的可见性不符",
                enqueued.id
            );
        }
        assert_eq!(queue.visible.len(), 3);
        assert_eq!(queue.pending.len(), 2);
        // FIFO：可见区保持入队顺序
        let order: Vec<&str> = queue.visible.iter().map(|t| t.message.as_ref()).collect();
        assert_eq!(order, vec!["a", "b", "c"]);
        let _ = ids;
    }

    #[test]
    fn expire_promotes_pending() {
        let mut queue = ToastQueue::new();
        let mut ids = Vec::new();
        for msg in ["a", "b", "c", "d"] {
            ids.push(queue.enqueue(ToastKind::Info, msg).id);
        }
        // 最早一条到期 → d 从队列补位，其余不动
        assert_eq!(queue.expire(ids[0]), Some(ids[3]));
        let order: Vec<&str> = queue.visible.iter().map(|t| t.message.as_ref()).collect();
        assert_eq!(order, vec!["b", "c", "d"]);
        assert!(queue.pending.is_empty());
    }

    #[test]
    fn expire_unknown_id_is_noop() {
        let mut queue = ToastQueue::new();
        queue.enqueue(ToastKind::Success, "a");
        queue.enqueue(ToastKind::Error, "b");
        assert_eq!(queue.expire(9999), None);
        assert_eq!(queue.visible.len(), 2);
    }

    #[test]
    fn stale_expire_does_not_remove_newer_entry() {
        let mut queue = ToastQueue::new();
        queue.enqueue(ToastKind::Info, "old");
        queue.enqueue(ToastKind::Info, "new");
        let old_id = queue.visible[0].id;
        // 旧计时器迟到：先移除旧条目，再触发同一次到期回调不应影响新条目
        assert!(queue.expire(old_id).is_some());
        assert_eq!(queue.expire(old_id), None);
        let msgs: Vec<&str> = queue.visible.iter().map(|t| t.message.as_ref()).collect();
        assert_eq!(msgs, vec!["new"]);
    }

    #[test]
    fn successive_expiry_drains_queue_one_by_one() {
        let mut queue = ToastQueue::new();
        let mut ids = Vec::new();
        for msg in ["a", "b", "c", "d", "e", "f"] {
            ids.push(queue.enqueue(ToastKind::Info, msg).id);
        }
        // 逐条到期：每次只补位一条，d → e → f 依次上屏
        for (round, front, expect) in [(0, "b", "d"), (1, "c", "e"), (2, "d", "f")] {
            assert_eq!(queue.expire(ids[round]), Some(ids[3 + round]));
            assert_eq!(
                queue.visible.front().map(|t| t.message.as_ref()),
                Some(front)
            );
            assert_eq!(
                queue.visible.back().map(|t| t.message.as_ref()),
                Some(expect)
            );
        }
        assert!(queue.pending.is_empty());
        assert_eq!(queue.visible.len(), 3);
    }
}
