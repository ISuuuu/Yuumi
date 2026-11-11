//! 大乱斗板凳席置顶透明悬浮小窗
//!
//! 数据源：`LiveLcuService` 广播的 `ChampSelectSession`（由主窗口转发）。
//! 交互：点击板凳席英雄图标 → 调用 LCU `bench/swap` 一键换人；「摇号」→ 重随当前英雄；
//!       「摇号换回」→ reroll 后瞬时从板凳席换回原英雄；空白区域按下可拖拽移动并记忆坐标。
//! 生命周期：仅在极地大乱斗（及同族板凳席队列）选人期间由主窗口显示，选人结束自动隐藏。

use crate::asset_loader::global_asset_loader;
use crate::components::icon::icon;
use crate::components::lcu_image::LcuImage;
use crate::platform;
use crate::services::LiveLcuService;
use crate::theme::Theme;
use crate::BENCH_WINDOW_TITLE;
use gpui::prelude::FluentBuilder;
use gpui::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use yuumi_core::agents::ChampSelectSession;
use yuumi_core::lcu::game_data::GameDataAssets;

/// 悬浮窗坐标持久化文件名（位于应用数据目录）
const OVERLAY_LAYOUT_FILE: &str = "bench-overlay.json";

/// [D07-a] 本会话用户是否已主动隐藏板凳席悬浮窗。
/// 置位后 WS 选人事件不再让主窗重建窗口；进入下一轮选人时由 live_lcu 分派点重置。
static BENCH_USER_HIDDEN: AtomicBool = AtomicBool::new(false);

/// [D07-a] 标记用户已隐藏（关闭按钮调用）。
pub fn mark_bench_user_hidden() {
    BENCH_USER_HIDDEN.store(true, Ordering::SeqCst);
}

/// [D07-a] 当前会话是否已被用户隐藏。
pub fn bench_user_hidden() -> bool {
    BENCH_USER_HIDDEN.load(Ordering::SeqCst)
}

/// [D07-a] 新一轮选人 / 用户主动打开时清除隐藏标志。
pub fn reset_bench_user_hidden() {
    BENCH_USER_HIDDEN.store(false, Ordering::SeqCst);
}

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
    /// 摇号 / 摇号换回请求进行中，用于连点去重
    pending_reroll: bool,
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
            pending_reroll: false,
            status: None,
            _bounds_subscription: None,
            _save_task: None,
        }
    }

    /// 绑定窗口监听，拖动窗口停止后防抖持久化最终坐标（P0-5 接线）
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
        self.pending_reroll = false;
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
        if self.pending_champion.is_some() || self.pending_reroll {
            return;
        }
        self.pending_reroll = true;
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
                view.pending_reroll = false;
                view.status = Some(message);
                cx.notify();
            });
        })
        .detach();
    }

    /// 摇号并瞬时换回原英雄
    fn reroll_and_restore(&mut self, cx: &mut Context<Self>) {
        if self.pending_champion.is_some() || self.pending_reroll {
            return;
        }
        self.pending_reroll = true;
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
                view.pending_reroll = false;
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

    #[allow(dead_code)]
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
                    .child(div().size(px(8.0)).rounded_full().bg(Theme::AURORA_CYAN()))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::TEXT_PRIMARY())
                            .child("大乱斗板凳席"),
                    )
                    .child(
                        div()
                            .px(px(4.0))
                            .rounded(px(3.0))
                            // 浅色卡面上改用品牌色浅底 + 主色字，避免亮青字看不清
                            .bg(Theme::ACCENT_LIGHT())
                            .text_size(px(9.0))
                            .text_color(Theme::ACCENT_PRIMARY())
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
                            .text_color(Theme::TEXT_MUTED())
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
                            .bg(Theme::HOVER_BG())
                            .hover(|s| s.bg(rgba(0x00f5d440)))
                            .text_size(px(10.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::TEXT_PRIMARY())
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
                            .bg(Theme::ACCENT_LIGHT())
                            .hover(|s| s.bg(Theme::ACCENT_ALPHA_20()))
                            .text_size(px(10.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::ACCENT_PRIMARY())
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

    fn render_slot(&self, slot: &BenchSlot, cx: &mut Context<Self>) -> impl IntoElement {
        let champion_id = slot.champion_id;
        let is_pending = self.pending_champion == Some(champion_id);
        let highlight = is_pending || champion_id == self.my_champion_id || slot.is_mine;
        let border_color = if highlight {
            Theme::AURORA_CYAN()
        } else {
            Theme::CARD_BORDER()
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
            .relative()
            .size(px(44.0))
            .rounded(px(8.0))
            .overflow_hidden()
            .border_2()
            .border_color(border_color)
            .opacity(if is_pending { 0.4 } else { 1.0 })
            .when(!is_pending, |this| {
                this.cursor_pointer()
                    .hover(|s| s.border_color(Theme::AURORA_CYAN()))
            })
            .child(
                LcuImage::new(champion_icon_path(champion_id))
                    .size(px(44.0))
                    .placeholder(slot.name.clone()),
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
        let status = self.status.clone();
        let rerolls = self.rerolls_remaining;
        let is_pending = self.pending_champion.is_some();

        div()
            .id("bench-overlay-root")
            .size_full()
            .relative()
            .flex()
            .flex_row()
            .items_center()
            // 对齐 Vue .bench-overlay-wrapper 的 --card-bg：浅色近白、深色深蓝，随主题切换；
            // 卡面按页面底色预合成成实色（GPUI 无 backdrop-filter，半透明会直接透出游戏画面）
            .bg(Theme::over_page(Theme::CARD_BG()))
            .border_1()
            .border_color(Theme::CARD_BORDER())
            .rounded(px(12.0))
            .px(px(8.0))
            .py(px(5.0))
            .shadow_md()
            .overflow_hidden()
            // 浮窗空白区域按下启动 Win32 原生拖拽
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _ev, window, cx| {
                    this.begin_drag(window, cx);
                }),
            )
            // 1. 左侧拖动手柄 (宽 20px)
            .child(
                div()
                    .id("bench-drag-handle")
                    .w(px(20.0))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_move()
                    .opacity(0.55)
                    .hover(|s| s.opacity(1.0).text_color(Theme::TEXT_PRIMARY()))
                    .child(icon("icons/drag.svg", 16.0, Theme::TEXT_MUTED()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev, window, cx| {
                            cx.stop_propagation();
                            this.begin_drag(window, cx);
                        }),
                    ),
            )
            // 2. 中间板凳英雄列表 (水平单行 flex-1)
            .child(
                div()
                    .id("bench-champions-list")
                    .flex_1()
                    .h_full()
                    .min_w(px(0.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(6.0))
                    .overflow_x_hidden()
                    .when(bench.is_empty(), |this| {
                        this.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::TEXT_MUTED())
                                .child("板凳席空空如也"),
                        )
                    })
                    .children(bench.iter().map(|slot| self.render_slot(slot, cx))),
            )
            // 3. 右侧操作区 (紧凑摇号/换回 + 关闭)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .id("bench-reroll")
                            .flex()
                            .items_center()
                            .h(px(24.0))
                            .px(px(6.0))
                            .rounded(px(4.0))
                            .bg(Theme::HOVER_BG())
                            .hover(|s| s.bg(rgba(0x00f5d433)))
                            .cursor_pointer()
                            .text_size(px(10.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::TEXT_PRIMARY())
                            .when(is_pending, |s| s.opacity(0.5))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _ev, _window, cx| {
                                    cx.stop_propagation();
                                    this.reroll(cx);
                                }),
                            )
                            .child(if rerolls > 0 {
                                format!("摇号 {}", rerolls)
                            } else {
                                "摇号".to_string()
                            }),
                    )
                    .child(
                        div()
                            .id("bench-reroll-restore")
                            .flex()
                            .items_center()
                            .h(px(24.0))
                            .px(px(6.0))
                            .rounded(px(4.0))
                            .bg(rgba(0x00f5d426))
                            .hover(|s| s.bg(rgba(0x00f5d44d)))
                            .cursor_pointer()
                            .text_size(px(10.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::AURORA_CYAN())
                            .when(is_pending, |s| s.opacity(0.5))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _ev, _window, cx| {
                                    cx.stop_propagation();
                                    this.reroll_and_restore(cx);
                                }),
                            )
                            .child("换回"),
                    )
                    .child(
                        div()
                            .id("bench-close-btn")
                            .size(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .opacity(0.55)
                            .hover(|s| s.opacity(1.0).text_color(Theme::LOSS_BAR()))
                            .text_color(Theme::TEXT_MUTED())
                            .child(icon("icons/close.svg", 14.0, Theme::TEXT_MUTED()))
                            .on_mouse_down(MouseButton::Left, |_ev, window, _app| {
                                // [D07-a] 用户主动隐藏：置位后本轮选人不再被重复事件自动打开
                                mark_bench_user_hidden();
                                window.remove_window();
                            }),
                    ),
            )
            // 4. 底部迷你悬浮提示小药丸
            .when_some(status, |this, text| {
                this.child(
                    div()
                        .absolute()
                        .bottom(px(3.0))
                        .left(px(180.0))
                        .px(px(8.0))
                        .py(px(1.0))
                        .rounded(px(8.0))
                        .bg(rgba(0x000000bf))
                        .text_size(px(10.0))
                        .text_color(rgb(0xffffff))
                        .child(text),
                )
            })
    }
}
