//! 大乱斗板凳席置顶透明悬浮小窗
//!
//! 数据源：`LiveLcuService` 广播的 `ChampSelectSession`（由主窗口转发）。
//! 交互：点击板凳席英雄图标 → 调用 LCU `bench/swap` 一键换人；「摇号」→ 重随当前英雄；
//!       「摇号换回」→ reroll 后瞬时从板凳席换回原英雄；空白区域按下可拖拽移动并记忆坐标。
//! 生命周期：仅在极地大乱斗（及同族板凳席队列）选人期间由主窗口显示，选人结束自动隐藏。

use crate::asset_loader::global_asset_loader;
use crate::components::lcu_image::LcuImage;
use crate::platform;
use crate::services::LiveLcuService;
use crate::theme::Theme;
use crate::BENCH_WINDOW_TITLE;
use gpui::prelude::FluentBuilder;
use gpui::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use yuumi_core::agents::ChampSelectSession;
use yuumi_core::lcu::game_data::GameDataAssets;

/// 悬浮窗坐标持久化文件名（位于应用数据目录）
const OVERLAY_LAYOUT_FILE: &str = "bench-overlay.json";

/// 悬浮窗左上角坐标
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct OverlayLayout {
    x: f32,
    y: f32,
}

fn layout_path() -> PathBuf {
    yuumi_core::runtime::app_data_dir().join(OVERLAY_LAYOUT_FILE)
}

/// 读取上次保存的小窗坐标（首次运行或读失败时返回 None）
pub fn load_saved_origin() -> Option<Point<Pixels>> {
    let raw = std::fs::read_to_string(layout_path()).ok()?;
    let layout: OverlayLayout = serde_json::from_str(&raw).ok()?;
    Some(Point {
        x: px(layout.x),
        y: px(layout.y),
    })
}

#[allow(dead_code)]
fn save_origin(origin: Point<Pixels>) {
    let path = layout_path();
    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            log::warn!("[BenchOverlay] 创建配置目录失败: {}", err);
            return;
        }
    }
    let layout = OverlayLayout {
        x: origin.x.into(),
        y: origin.y.into(),
    };
    match serde_json::to_string(&layout) {
        Ok(json) => {
            if let Err(err) = std::fs::write(&path, json) {
                log::warn!("[BenchOverlay] 保存小窗坐标失败: {}", err);
            }
        }
        Err(err) => log::warn!("[BenchOverlay] 序列化小窗坐标失败: {}", err),
    }
}

/// 英雄图标资源路径
fn champion_icon_path(champion_id: i32) -> String {
    format!(
        "/lol-game-data/assets/v1/champion-icons/{}.png",
        champion_id
    )
}

/// 小窗内单个英雄槽位
#[derive(Debug, Clone, PartialEq)]
pub struct BenchSlot {
    pub champion_id: i32,
    pub name: SharedString,
    pub is_mine: bool,
}

pub struct BenchOverlayView {
    live_service: LiveLcuService,
    /// 板凳席剩余英雄
    bench: Vec<BenchSlot>,
    /// 本方已拥有的英雄（含自己 + 队友）
    team: Vec<BenchSlot>,
    /// 当前登录玩家已选英雄
    my_champion_id: i32,
    queue_id: i32,
    /// 剩余摇号次数
    rerolls_remaining: i32,
    /// 正在提交换人请求的英雄 ID（用于禁用重复点击）
    pending_champion: Option<i32>,
    /// 最近一次操作反馈
    status: Option<SharedString>,
    /// 窗口边界变化监听句柄
    _bounds_subscription: Option<Subscription>,
    /// 坐标保存防抖任务
    _save_task: Option<Task<()>>,
}

impl BenchOverlayView {
    pub fn new(live_service: LiveLcuService) -> Self {
        Self {
            live_service,
            bench: Vec::new(),
            team: Vec::new(),
            my_champion_id: 0,
            queue_id: 0,
            rerolls_remaining: 0,
            pending_champion: None,
            status: None,
            _bounds_subscription: None,
            _save_task: None,
        }
    }

    /// 绑定窗口监听，拖动窗口停止后防抖持久化最终坐标
    #[allow(dead_code)]
    pub fn bind_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self._bounds_subscription = Some(cx.observe_window_bounds(window, |this, window, cx| {
            if let WindowBounds::Windowed(bounds) = window.window_bounds() {
                this._save_task = Some(cx.spawn(async move |_this, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(300))
                        .await;
                    save_origin(bounds.origin);
                }));
            }
        }));
    }

    /// 用最新选人会话刷新小窗数据
    pub fn apply_session(
        &mut self,
        session: &ChampSelectSession,
        assets: Option<&GameDataAssets>,
        cx: &mut Context<Self>,
    ) {
        let name_of = |id: i32| -> SharedString {
            assets
                .and_then(|a| a.champions.get(&id).cloned())
                .unwrap_or_else(|| format!("英雄 {}", id))
                .into()
        };

        self.queue_id = session.queue_id;
        self.my_champion_id = session.local_champion_id();
        self.rerolls_remaining = session.rerolls_remaining;
        self.bench = session
            .bench_champions
            .iter()
            .filter(|c| c.champion_id > 0)
            .map(|c| BenchSlot {
                champion_id: c.champion_id,
                name: name_of(c.champion_id),
                is_mine: c.is_mine,
            })
            .collect();
        self.team = session
            .my_team
            .iter()
            .filter(|p| p.champion_id > 0)
            .map(|p| BenchSlot {
                champion_id: p.champion_id,
                name: name_of(p.champion_id),
                is_mine: p.cell_id == session.local_player_cell_id,
            })
            .collect();

        // 换人请求已落地（板凳席不再包含该英雄）则清除 pending 状态
        if let Some(pending) = self.pending_champion {
            if !self.bench.iter().any(|slot| slot.champion_id == pending) {
                self.pending_champion = None;
            }
        }

        self.preload_icons(cx);
        cx.notify();
    }

    /// 选人结束 / 非大乱斗队列时清空展示
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.bench.clear();
        self.team.clear();
        self.my_champion_id = 0;
        self.queue_id = 0;
        self.rerolls_remaining = 0;
        self.pending_champion = None;
        self.status = None;
        cx.notify();
    }

    /// 异步预热当前展示英雄的图标，解码完成后重绘
    fn preload_icons(&self, cx: &mut Context<Self>) {
        let mut paths: Vec<String> = self
            .bench
            .iter()
            .chain(self.team.iter())
            .map(|slot| champion_icon_path(slot.champion_id))
            .collect();
        paths.sort();
        paths.dedup();
        if paths.is_empty() {
            return;
        }

        cx.spawn(async move |this, cx| {
            let loader = global_asset_loader();
            let mut loaded_any = false;
            for path in paths {
                if loader.get_cached_render_image(&path).is_some() {
                    continue;
                }
                if loader.load_render_image(&path).await.is_ok() {
                    loaded_any = true;
                }
            }
            if loaded_any {
                let _ = this.update(cx, |_view, cx| cx.notify());
            }
        })
        .detach();
    }

    /// 一键换取板凳席英雄
    fn swap_to(&mut self, champion_id: i32, cx: &mut Context<Self>) {
        if self.pending_champion.is_some() || champion_id <= 0 {
            return;
        }
        if champion_id == self.my_champion_id {
            self.status = Some("已是当前英雄".into());
            cx.notify();
            return;
        }

        self.pending_champion = Some(champion_id);
        self.status = Some("换人中…".into());
        cx.notify();

        let service = self.live_service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.swap_bench_champion(champion_id).await;
            let message: SharedString = match &result {
                Ok(()) => "已提交换人".into(),
                Err(e) => format!("换人失败: {}", e).into(),
            };
            if let Err(e) = &result {
                log::warn!("[BenchOverlay] {}", e);
            }
            let _ = this.update(cx, |view, cx| {
                view.pending_champion = None;
                view.status = Some(message);
                cx.notify();
            });
        })
        .detach();
    }

    /// 摇号（重随）当前英雄
    fn reroll(&mut self, cx: &mut Context<Self>) {
        if self.pending_champion.is_some() {
            return;
        }
        self.status = Some("摇号中…".into());
        cx.notify();

        let service = self.live_service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.reroll_aram_champion().await;
            let message: SharedString = match &result {
                Ok(()) => "已提交摇号".into(),
                Err(e) => format!("摇号失败: {}", e).into(),
            };
            if let Err(e) = &result {
                log::warn!("[BenchOverlay] {}", e);
            }
            let _ = this.update(cx, |view, cx| {
                view.status = Some(message);
                cx.notify();
            });
        })
        .detach();
    }

    /// 摇号并瞬时换回原英雄
    fn reroll_and_restore(&mut self, cx: &mut Context<Self>) {
        if self.pending_champion.is_some() {
            return;
        }
        self.status = Some("摇号换回中…".into());
        cx.notify();

        let service = self.live_service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.reroll_and_restore_aram_champion().await;
            let message: SharedString = match &result {
                Ok(champion_id) => format!("已摇号并换回英雄 {}", champion_id).into(),
                Err(e) => format!("摇号换回失败: {}", e).into(),
            };
            if let Err(e) = &result {
                log::warn!("[BenchOverlay] {}", e);
            }
            let _ = this.update(cx, |view, cx| {
                view.status = Some(message);
                cx.notify();
            });
        })
        .detach();
    }

    /// 空白区域按下：交给 Win32 异步启动原生拖拽（坐标由 bind_window 注册的 bounds observer 防抖持久化）
    fn begin_drag(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(hwnd) = platform::find_window_by_title(BENCH_WINDOW_TITLE) else {
            log::warn!("[BenchOverlay] 未找到小窗 HWND，跳过拖拽");
            return;
        };

        platform::begin_window_drag(hwnd);
        cx.notify();
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let queue_label: SharedString = match self.queue_id {
            450 => "极地大乱斗".into(),
            2400 => "海克斯大乱斗".into(),
            2450 => "经典海斗".into(),
            other if other > 0 => format!("队列 {}", other).into(),
            _ => "等待选人数据".into(),
        };

        div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().size(px(8.0)).rounded_full().bg(Theme::AURORA_CYAN))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xffffff))
                            .child("大乱斗板凳席"),
                    )
                    .child(
                        div()
                            .px(px(4.0))
                            .rounded(px(3.0))
                            .bg(rgba(0x00f5d430))
                            .text_size(px(9.0))
                            .text_color(Theme::AURORA_CYAN)
                            .child(queue_label),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(9.0))
                            .text_color(Theme::TEXT_MUTED)
                            .child(format!(
                                "板凳 {} · 本方 {} · 剩余摇号 {}",
                                self.bench.len(),
                                self.team.len(),
                                self.rerolls_remaining
                            )),
                    )
                    .child(
                        div()
                            .id("bench-reroll")
                            .cursor_pointer()
                            .px_2()
                            .rounded_sm()
                            .bg(rgba(0xffffff20))
                            .hover(|s| s.bg(rgba(0x00f5d440)))
                            .text_size(px(10.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _ev, _window, cx| {
                                    cx.stop_propagation();
                                    this.reroll(cx);
                                }),
                            )
                            .child("摇号"),
                    )
                    .child(
                        div()
                            .id("bench-reroll-restore")
                            .cursor_pointer()
                            .px_2()
                            .rounded_sm()
                            .bg(rgba(0x00f5d433))
                            .hover(|s| s.bg(rgba(0x00f5d466)))
                            .text_size(px(10.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::AURORA_CYAN)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _ev, _window, cx| {
                                    cx.stop_propagation();
                                    this.reroll_and_restore(cx);
                                }),
                            )
                            .child("摇号换回"),
                    ),
            )
    }

    fn render_section_label(text: &'static str) -> impl IntoElement {
        div()
            .text_size(px(9.0))
            .text_color(Theme::TEXT_MUTED)
            .child(text)
    }

    fn render_slot(&self, slot: &BenchSlot, cx: &mut Context<Self>) -> impl IntoElement {
        let champion_id = slot.champion_id;
        let is_pending = self.pending_champion == Some(champion_id);
        let highlight = is_pending || champion_id == self.my_champion_id || slot.is_mine;
        let border_color = if highlight {
            Theme::AURORA_CYAN
        } else {
            Theme::CARD_BORDER
        };

        div()
            .id(ElementId::NamedInteger(
                if slot.is_mine {
                    "bench-slot-mine"
                } else {
                    "bench-slot"
                }
                .into(),
                champion_id as u64,
            ))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(1.0))
            .opacity(if is_pending { 0.55 } else { 1.0 })
            .when(!is_pending, |this| {
                this.cursor_pointer().hover(|s| s.opacity(0.85))
            })
            .child(
                LcuImage::new(champion_icon_path(champion_id))
                    .size(px(38.0))
                    .border_color(border_color)
                    .placeholder(slot.name.clone()),
            )
            .child(
                div()
                    .max_w(px(46.0))
                    .overflow_hidden()
                    .text_size(px(8.0))
                    .text_color(if highlight {
                        Theme::AURORA_CYAN
                    } else {
                        Theme::TEXT_SECONDARY
                    })
                    .child(slot.name.clone()),
            )
            .when(!is_pending, |this| {
                this.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _ev, _window, cx| {
                        cx.stop_propagation();
                        view.swap_to(champion_id, cx);
                    }),
                )
            })
    }
}

impl Render for BenchOverlayView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bench = self.bench.clone();
        let team = self.team.clone();
        let status = self.status.clone();
        let header = self.render_header(cx);
        let bench_slots: Vec<_> = bench
            .iter()
            .map(|slot| self.render_slot(slot, cx))
            .collect();

        div()
            .id("bench-overlay-root")
            .size_full()
            .flex()
            .flex_col()
            .gap_1()
            .bg(rgba(0x0f172af2)) // 游戏内深色半透底色
            .border_1()
            .border_color(Theme::AURORA_CYAN.opacity(0.55))
            .rounded_xl()
            .p_3()
            .shadow_lg()
            // 空白区域按下可拖拽移动（英雄槽位与按钮内部阻止冒泡）
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _ev, window, cx| {
                    this.begin_drag(window, cx);
                }),
            )
            .child(header)
            .child(Self::render_section_label("板凳席（点击换取）"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .min_h(px(56.0))
                    .when(bench.is_empty(), |this| {
                        this.child(
                            div()
                                .text_size(px(10.0))
                                .text_color(Theme::TEXT_MUTED)
                                .child("暂无板凳席英雄"),
                        )
                    })
                    .children(bench_slots),
            )
            .child(Self::render_section_label("本方已拥有"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(team.is_empty(), |this| {
                        this.child(
                            div()
                                .text_size(px(10.0))
                                .text_color(Theme::TEXT_MUTED)
                                .child("暂无已选英雄"),
                        )
                    })
                    .children(team.iter().map(|slot| {
                        div().flex().flex_col().items_center().gap(px(1.0)).child(
                            LcuImage::new(champion_icon_path(slot.champion_id))
                                .size(px(26.0))
                                .border_color(if slot.is_mine {
                                    Theme::AURORA_CYAN
                                } else {
                                    Theme::CARD_BORDER
                                })
                                .placeholder(slot.name.clone()),
                        )
                    })),
            )
            .when_some(status, |this, text| {
                this.child(
                    div()
                        .text_size(px(9.0))
                        .text_color(Theme::AURORA_CYAN)
                        .child(text),
                )
            })
    }
}
