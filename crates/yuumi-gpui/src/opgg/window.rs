//! OP.GG 独立窗口视图
//!
//! 对照 Vue `src/components/OpggWindow.vue` + `OpggModal.vue`：
//! 头部页签与版本号、筛选栏、榜单 / 详情两种正文形态，以及加载、失败与空态。
//!
//! 数据来源分三条互不阻塞的路径：
//! 1. 榜单与详情走 `yuumi_core::opgg`（远程 OP.GG 接口，带缓存）；
//! 2. 图标与符文资料走本地 LCU（`GameDataAssets` + `perkstyles/perks`）；
//! 3. 符文页与装备页写入走 `yuumi_core::lcu::write`。
//!
//! 请求以自增序号标记，只有最新一次结果能落到界面，避免快速切换筛选时旧响应覆盖新数据。

use crate::asset_loader;
use crate::components::icon::opgg_logo;
use crate::components::toast::{toast_overlay, ToastKind, ToastStack};
use crate::opgg::build_detail::{build_detail, BuildDetailContext};
use crate::opgg::filter_bar::{filter_bar, FilterKind, FilterState};
use crate::opgg::format::{champion_icon, item_icon, rune_icon, spell_icon};
use crate::opgg::tier_list::tier_list;
use crate::opgg::{load_rune_catalog, Palette, RuneCatalog};
use crate::services::LiveLcuService;
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use yuumi_core::lcu::game_data::GameDataAssets;
use yuumi_core::lcu::write::{
    apply_item_set, apply_rune_page, ItemSetBlockPayload, RunePageParams,
};
use yuumi_core::opgg::{
    build_item_set_blocks, fetch_champion_build, fetch_tier_list, rune_perk_ids, BuildSummary,
    ChampionBuild, OpggMode, ProxyConfig, TierListEntry,
};

/// 窗口标题（主窗据此定位 HWND 应用置顶）
pub const OPGG_WINDOW_TITLE: &str = "OP.GG";

/// 顶部页签
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpggTab {
    TierList,
    Build,
}

/// OP.GG 独立窗口
pub struct OpggWindow {
    filter: FilterState,
    open_dropdown: Option<FilterKind>,
    tab: OpggTab,

    tier_list: Vec<TierListEntry>,
    build: Option<Arc<ChampionBuild>>,
    selected_champion: Option<i32>,
    version: String,
    loading: bool,
    error: String,
    rune_index: usize,
    applying_items: bool,
    applying_rune: bool,
    /// 窗口级 Toast 队列（对照 Vue NMessageProvider）
    toast_stack: Entity<ToastStack>,

    game_assets: Option<Arc<GameDataAssets>>,
    catalog: RuneCatalog,
    champion_names: HashMap<i32, String>,

    service: LiveLcuService,
    proxy: ProxyConfig,
    request_seq: u64,
    scroll_handle: ScrollHandle,
}

impl OpggWindow {
    pub fn new(
        service: LiveLcuService,
        proxy: ProxyConfig,
        initial_champion: Option<i32>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self {
            filter: FilterState::default(),
            open_dropdown: None,
            tab: OpggTab::TierList,
            tier_list: Vec::new(),
            build: None,
            selected_champion: None,
            version: String::new(),
            loading: false,
            error: String::new(),
            rune_index: 0,
            applying_items: false,
            applying_rune: false,
            toast_stack: {
                let stack = cx.new(|_| ToastStack::new());
                cx.observe(&stack, |_, _, cx| cx.notify()).detach();
                stack
            },
            game_assets: None,
            catalog: RuneCatalog::default(),
            champion_names: HashMap::new(),
            service,
            proxy,
            request_seq: 0,
            scroll_handle: ScrollHandle::new(),
        };
        view.load_static_data(cx);
        match initial_champion {
            Some(champion_id) => view.open_build(champion_id, cx),
            None => view.fetch_tier_list(cx),
        }
        view
    }

    // ─── 数据 ───

    /// 载入 LCU 侧静态资料：英雄名映射与符文树/符文文案
    fn load_static_data(&mut self, cx: &mut Context<Self>) {
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let assets = service.get_game_data_assets().await;
            let catalog = match service.get_client().await {
                Some(client) => load_rune_catalog(&client).await,
                None => {
                    log::warn!("[OPGG] 未连接 LCU，符文树资料不可用");
                    RuneCatalog::default()
                }
            };
            let _ = this.update(cx, |view, cx| {
                if let Some(assets) = assets {
                    view.champion_names = assets.champions.clone();
                    view.game_assets = Some(assets);
                }
                view.catalog = catalog;
                cx.notify();
            });
        })
        .detach();
    }

    /// 预热图片资源，解码落地后重绘
    fn prewarm(&self, paths: Vec<String>, cx: &mut Context<Self>) {
        if paths.is_empty() {
            return;
        }
        let handle = yuumi_core::runtime::tokio_handle();
        cx.spawn(async move |this, cx| {
            let Some(handle) = handle else {
                log::error!("[OPGG] Tokio 运行时未注册，跳过资源预热");
                return;
            };
            let _ = handle
                .spawn(async move {
                    let loader = asset_loader::global_asset_loader();
                    let mut tasks = Vec::with_capacity(paths.len());
                    for path in paths {
                        let loader = loader.clone();
                        tasks.push(tokio::spawn(async move {
                            let _ = loader.load_render_image(&path).await;
                        }));
                    }
                    for task in tasks {
                        let _ = task.await;
                    }
                })
                .await;
            let _ = this.update(cx, |_this, cx| cx.notify());
        })
        .detach();
    }

    /// 拉取当前筛选条件下的梯队榜单
    fn fetch_tier_list(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        self.error.clear();
        self.request_seq = self.request_seq.wrapping_add(1);
        let sequence = self.request_seq;
        let filter = self.filter;
        let proxy = self.proxy.clone();

        cx.spawn(async move |this, cx| {
            let result = fetch_tier_list(
                filter.region,
                filter.mode,
                filter.tier,
                filter.position_for_request(),
                &proxy,
            )
            .await;
            let _ = this.update(cx, |view, cx| {
                if view.request_seq != sequence {
                    return;
                }
                view.loading = false;
                match result {
                    Ok(list) => {
                        view.version = list.version;
                        view.tier_list = list.entries;
                        view.error.clear();
                        let paths: Vec<String> = view
                            .tier_list
                            .iter()
                            .flat_map(|entry| {
                                let mut paths = vec![champion_icon(entry.champion_id)];
                                paths.extend(
                                    entry.counters.iter().take(3).map(|id| champion_icon(*id)),
                                );
                                paths
                            })
                            .collect();
                        view.prewarm(paths, cx);
                    }
                    Err(err) => {
                        log::warn!("[OPGG] 榜单查询失败: {err}");
                        view.tier_list.clear();
                        view.error = err;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 打开指定英雄的详情页
    fn open_build(&mut self, champion_id: i32, cx: &mut Context<Self>) {
        self.selected_champion = Some(champion_id);
        self.tab = OpggTab::Build;
        self.fetch_build(champion_id, cx);
    }

    /// 拉取指定英雄的符文出装详情
    fn fetch_build(&mut self, champion_id: i32, cx: &mut Context<Self>) {
        self.loading = true;
        self.error.clear();
        self.request_seq = self.request_seq.wrapping_add(1);
        let sequence = self.request_seq;
        let filter = self.filter;
        let proxy = self.proxy.clone();

        cx.spawn(async move |this, cx| {
            let result = fetch_champion_build(
                filter.region,
                filter.mode,
                filter.tier,
                champion_id,
                filter.position_for_request(),
                &proxy,
            )
            .await;
            let _ = this.update(cx, |view, cx| {
                if view.request_seq != sequence {
                    return;
                }
                view.loading = false;
                match result {
                    Ok(build) => {
                        view.version = build.version.clone();
                        view.error.clear();
                        // 详情更新时复位符文方案选择（对照 Vue 对 runes 的 watch）
                        view.rune_index = 0;
                        let paths =
                            build_icon_paths(&build, view.game_assets.as_deref(), &view.catalog);
                        view.build = Some(Arc::new(build));
                        view.prewarm(paths, cx);
                    }
                    Err(err) => {
                        log::warn!("[OPGG] 详情查询失败: {err}");
                        view.build = None;
                        view.error = err;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn show_toast(&mut self, message: String, success: bool, cx: &mut Context<Self>) {
        let kind = if success {
            ToastKind::Success
        } else {
            ToastKind::Error
        };
        self.toast_stack.update(cx, |stack, cx| {
            stack.push(kind, message, cx);
        });
    }

    // ─── 写入 ───

    /// 应用当前选中的符文方案
    fn apply_active_rune(&mut self, cx: &mut Context<Self>) {
        if self.applying_rune {
            return;
        }
        let Some(build) = self.build.clone() else {
            return;
        };
        let Some(rune) = build.runes.get(self.rune_index).cloned() else {
            self.show_toast("当前英雄没有可应用的符文方案".to_string(), false, cx);
            return;
        };
        let name = format!(
            "Yuumi: {}",
            self.champion_display_name(build.summary.as_ref())
        );
        let perk_ids = rune_perk_ids(&rune);
        let service = self.service.clone();

        self.applying_rune = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(client) = service.get_client().await else {
                let _ = this.update(cx, |view, cx| {
                    view.applying_rune = false;
                    view.show_toast("未连接到英雄联盟客户端".to_string(), false, cx);
                });
                return;
            };
            let params = RunePageParams {
                name: &name,
                primary_style_id: rune.primary_page_id,
                sub_style_id: rune.secondary_page_id,
                selected_perk_ids: &perk_ids,
            };
            let result = apply_rune_page(&client, params).await;
            let _ = this.update(cx, |view, cx| {
                view.applying_rune = false;
                match result {
                    Ok(()) => view.show_toast(format!("符文页应用成功：{name}"), true, cx),
                    Err(err) => view.show_toast(format!("应用符文页失败: {err}"), false, cx),
                }
            });
        })
        .detach();
    }

    /// 一键应用出装到客户端推荐页
    fn apply_items(&mut self, cx: &mut Context<Self>) {
        if self.applying_items {
            return;
        }
        let Some(build) = self.build.clone() else {
            return;
        };
        let blocks = build_item_set_blocks(&build);
        if blocks.is_empty() {
            // 对照 Vue H04：无推荐出装为警告级别
            self.toast_stack.update(cx, |stack, cx| {
                stack.push(ToastKind::Warning, "当前英雄暂无推荐出装数据", cx);
            });
            cx.notify();
            return;
        }

        let champion_id = build
            .summary
            .as_ref()
            .and_then(|summary| summary.champion_id)
            .or(self.selected_champion);
        let Some(champion_id) = champion_id else {
            // 对照 Vue H03：缺当前英雄信息为警告级别
            self.toast_stack.update(cx, |stack, cx| {
                stack.push(ToastKind::Warning, "无法获取当前英雄信息", cx);
            });
            cx.notify();
            return;
        };

        let position_suffix = self
            .filter
            .position_for_request()
            .map(|position| format!(" - {position}"))
            .unwrap_or_default();
        let title = format!(
            "[Yuumi] {}{position_suffix}",
            self.champion_display_name(build.summary.as_ref())
        );
        let payload: Vec<ItemSetBlockPayload> = blocks.iter().map(Into::into).collect();
        let service = self.service.clone();

        self.applying_items = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(client) = service.get_client().await else {
                let _ = this.update(cx, |view, cx| {
                    view.applying_items = false;
                    view.show_toast("未连接到英雄联盟客户端".to_string(), false, cx);
                });
                return;
            };
            let result = apply_item_set(&client, champion_id, &title, &payload).await;
            let _ = this.update(cx, |view, cx| {
                view.applying_items = false;
                match result {
                    Ok(message) => view.show_toast(message, true, cx),
                    Err(err) => view.show_toast(format!("导入装备页失败: {err}"), false, cx),
                }
            });
        })
        .detach();
    }

    /// 英雄显示名：优先本地化名称，其次 OP.GG 名称
    fn champion_display_name(&self, summary: Option<&BuildSummary>) -> String {
        summary
            .and_then(|summary| summary.champion_id)
            .or(self.selected_champion)
            .and_then(|id| self.champion_names.get(&id).cloned())
            .or_else(|| summary.and_then(|summary| summary.name.clone()))
            .unwrap_or_else(|| "英雄".to_string())
    }

    // ─── 渲染 ───

    /// 顶部：OP.GG 标识、页签与游戏版本
    fn render_header(&self, entity: &Entity<Self>) -> impl IntoElement {
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px(px(16.0))
            .py(px(10.0))
            .border_b_1()
            .border_color(Theme::CARD_BORDER)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(opgg_logo(22.0))
                    .child(
                        div()
                            .text_size(px(15.2))
                            .font_weight(FontWeight::EXTRA_BOLD)
                            .text_color(Palette::logo())
                            .child("OP.GG"),
                    )
                    .child(tab_button(
                        "梯队",
                        self.tab == OpggTab::TierList,
                        false,
                        entity,
                        |this, cx| {
                            this.tab = OpggTab::TierList;
                            this.open_dropdown = None;
                            this.fetch_tier_list(cx);
                        },
                    ))
                    .child(tab_button(
                        "出装",
                        self.tab == OpggTab::Build,
                        self.selected_champion.is_none(),
                        entity,
                        |this, cx| {
                            this.tab = OpggTab::Build;
                            this.open_dropdown = None;
                            if let Some(champion_id) = this.selected_champion {
                                this.fetch_build(champion_id, cx);
                            }
                        },
                    )),
            )
            .child(div().flex().items_center().gap(px(12.0)).children(
                (!self.version.is_empty()).then(|| {
                    div()
                        .text_size(px(11.52))
                        .text_color(Theme::TEXT_MUTED)
                        .child(format!("游戏版本: {}", self.version))
                }),
            ))
    }

    /// 正文：加载 / 失败 / 榜单 / 详情 / 空态
    fn render_body(&mut self, entity: &Entity<Self>) -> AnyElement {
        if self.loading {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                // Vue 是持续旋转的 CSS 圆环；GPUI 无 CSS 动画，这里用静态双环指示加载中
                .child(
                    div()
                        .size(px(28.0))
                        .rounded_full()
                        .border_2()
                        .border_color(Theme::ACCENT_ALPHA_20)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .size(px(10.0))
                                .rounded_full()
                                .bg(Theme::ACCENT_PRIMARY),
                        ),
                )
                .into_any_element();
        }

        if !self.error.is_empty() {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .px(px(24.0))
                .text_size(px(13.6))
                .text_color(Palette::loss_text())
                .child(self.error.clone())
                .into_any_element();
        }

        match self.tab {
            OpggTab::TierList => {
                let select_entity = entity.clone();
                div()
                    .id("opgg-tier-scroll")
                    .flex_1()
                    .px(px(10.0))
                    .py(px(8.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(tier_list(
                        &self.tier_list,
                        &self.champion_names,
                        move |champion_id, _window, app| {
                            select_entity.update(app, |this, cx| {
                                this.open_build(champion_id, cx);
                            });
                        },
                    ))
                    .into_any_element()
            }
            OpggTab::Build => {
                let Some(build) = self.build.clone() else {
                    return div().flex_1().into_any_element();
                };
                let rune_entity = entity.clone();
                let apply_rune_entity = entity.clone();
                let apply_items_entity = entity.clone();
                let context = BuildDetailContext {
                    build: build.as_ref(),
                    champion_id: self.selected_champion,
                    position: self.filter.position_for_request(),
                    champion_names: &self.champion_names,
                    assets: self.game_assets.as_deref(),
                    catalog: &self.catalog,
                    rune_index: self.rune_index,
                    applying_items: self.applying_items,
                    on_select_rune: Rc::new(move |index, _window, app| {
                        rune_entity.update(app, |this, cx| {
                            this.rune_index = index;
                            cx.notify();
                        });
                    }),
                    on_apply_rune: Rc::new(move |_window, app| {
                        apply_rune_entity.update(app, |this, cx| this.apply_active_rune(cx));
                    }),
                    on_apply_items: Rc::new(move |_window, app| {
                        apply_items_entity.update(app, |this, cx| this.apply_items(cx));
                    }),
                };

                div()
                    .id("opgg-build-scroll")
                    .flex_1()
                    .bg(Palette::build_bg())
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(build_detail(context))
                    .into_any_element()
            }
        }
    }
}

/// 页签按钮
fn tab_button(
    label: &'static str,
    active: bool,
    disabled: bool,
    entity: &Entity<OpggWindow>,
    on_click: impl Fn(&mut OpggWindow, &mut Context<OpggWindow>) + 'static,
) -> impl IntoElement {
    let entity = entity.clone();
    div()
        .id(SharedString::from(format!("opgg-tab-{label}")))
        .px(px(12.0))
        .py(px(3.0))
        .border_1()
        .border_color(if active {
            Palette::tab_active()
        } else {
            Theme::CARD_BORDER
        })
        .rounded(px(6.0))
        .bg(if active {
            Palette::tab_active()
        } else {
            Theme::CARD_BG
        })
        .text_size(px(12.48))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if active {
            rgb(0xffffff).into()
        } else {
            Theme::TEXT_SECONDARY
        })
        .when(disabled, |style| style.opacity(0.4))
        .when(!disabled, |style| {
            style
                .cursor_pointer()
                .hover(|hover| {
                    if active {
                        hover
                            .bg(Palette::tab_active_hover())
                            .border_color(Palette::tab_active_hover())
                    } else {
                        hover
                            .bg(Theme::CARD_BG_HOVER)
                            .text_color(Theme::TEXT_PRIMARY)
                    }
                })
                .on_mouse_down(MouseButton::Left, move |_ev, _window, app| {
                    entity.update(app, |this, cx| on_click(this, cx));
                })
        })
        .child(label)
}

/// 详情页需要预热的图标路径（与渲染时的解析函数保持一致，否则预热不命中）
fn build_icon_paths(
    build: &ChampionBuild,
    assets: Option<&GameDataAssets>,
    catalog: &RuneCatalog,
) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    if let Some(champion_id) = build.summary.as_ref().and_then(|s| s.champion_id) {
        paths.push(champion_icon(champion_id));
    }
    for counter in &build.counters {
        paths.push(champion_icon(counter.champion_id));
    }
    for entry in &build.summoner_spells {
        paths.extend(entry.ids.iter().map(|id| spell_icon(assets, *id)));
    }
    for group in build
        .starter_items
        .iter()
        .chain(build.core_items.iter())
        .chain(build.boots.iter())
        .chain(build.last_items.iter())
    {
        paths.extend(group.ids.iter().map(|id| item_icon(assets, *id)));
    }
    for rune in &build.runes {
        paths.push(rune_icon(assets, catalog, rune.primary_page_id));
        paths.push(rune_icon(assets, catalog, rune.secondary_page_id));
        paths.extend(
            rune.primary_rune_ids
                .iter()
                .chain(rune.secondary_rune_ids.iter())
                .chain(rune.stat_mod_ids.iter())
                .map(|id| rune_icon(assets, catalog, *id)),
        );
    }
    paths
}

impl Render for OpggWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().clone();
        let body = self.render_body(&entity);
        let header = self.render_header(&entity);

        let toggle_entity = entity.clone();
        let select_entity = entity.clone();
        let filters = filter_bar(
            self.filter,
            self.open_dropdown,
            move |kind, _window, app| {
                toggle_entity.update(app, |this, cx| {
                    this.open_dropdown = if this.open_dropdown == Some(kind) {
                        None
                    } else {
                        Some(kind)
                    };
                    cx.notify();
                });
            },
            move |kind, value, _window, app| {
                select_entity.update(app, |this, cx| {
                    this.open_dropdown = None;
                    match kind {
                        FilterKind::Region => this.filter.region = value,
                        FilterKind::Mode => this.filter.mode = OpggMode::from_filter_value(value),
                        FilterKind::Tier => this.filter.tier = value,
                        FilterKind::Position => this.filter.position = value,
                    }
                    // 切换筛选后按当前页签重新取数（对照 Vue 的 watch）
                    match this.tab {
                        OpggTab::TierList => this.fetch_tier_list(cx),
                        OpggTab::Build => {
                            if let Some(champion_id) = this.selected_champion {
                                this.fetch_build(champion_id, cx);
                            }
                        }
                    }
                });
            },
        );

        div()
            .id("opgg-window-root")
            .size_full()
            .flex()
            .flex_col()
            .bg(Theme::BG_APP)
            .text_color(Theme::TEXT_PRIMARY)
            .font_family(Theme::font_family())
            // 空白区域按下收起筛选下拉（对照 Vue 的 document click 全局关闭）
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev, _window, cx| {
                    if this.open_dropdown.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .child(header)
            .child(filters)
            .child(body)
            // 窗口级 Toast 浮层（最后绘制，z 序最高；不拦截鼠标）
            .child(toast_overlay(self.toast_stack.read(cx)))
    }
}
