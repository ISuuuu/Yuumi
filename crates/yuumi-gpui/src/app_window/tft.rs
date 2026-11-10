// 云顶之弈路由：三页签的数据加载、动作分发与渲染。
//
// 数据层全部在 yuumi_core::parsers::tft：
// - 段位 / 战绩走 LCU；
// - 英雄、羁绊、装备与海克斯强化走 CDragon 镜像（一次下载，落盘缓存）；
// - 热门阵容走 OP.GG MCP。
use super::*;
use crate::views::tft::{TftAction, TftTab};
use yuumi_core::parsers::summoner::fetch_current_summoner;
use yuumi_core::parsers::tft::{
    fetch_tft_match_history, fetch_tft_meta_decks, fetch_tft_ranked_stats, load_tft_catalog,
    TftMatchSummary,
};

/// 生涯页一次展示的对局数（对照 Vue 的 `begIndex=0, endIndex=20`）
const TFT_MATCH_WINDOW: u32 = 20;

/// 读取客户端版本（`15.21`），失败返回空串
async fn fetch_client_version(client: &yuumi_core::lcu::client::LcuClient) -> String {
    let mut raw = String::new();
    for path in [
        "/lol-patch/v1/game-version",
        "/lol-game-data/assets/v1/game-version.json",
    ] {
        match client.get(path).await {
            Ok(serde_json::Value::String(text)) => {
                raw = text;
            }
            Ok(other) => {
                if let Some(text) = other.as_str() {
                    raw = text.to_string();
                }
            }
            Err(err) => log::debug!("[TFT] 读取客户端版本 {path} 失败: {err}"),
        }
        if !raw.trim().is_empty() {
            break;
        }
    }

    let trimmed = raw.trim().trim_start_matches('v').to_string();
    let parts: Vec<&str> = trimmed.split('.').collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[0], parts[1])
    } else {
        trimmed
    }
}

/// 收集战绩中需要预热的图片路径（棋子 / 装备 / 羁绊）
fn tft_icon_paths(summary: &TftMatchSummary) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for entry in &summary.matches {
        for unit in &entry.units {
            if !unit.icon_url.is_empty() {
                paths.push(unit.icon_url.clone());
            }
            paths.extend(
                unit.item_icon_urls
                    .iter()
                    .filter(|url| !url.is_empty())
                    .cloned(),
            );
        }
        for trait_value in &entry.traits {
            if !trait_value.icon_url.is_empty() {
                paths.push(trait_value.icon_url.clone());
            }
        }
    }
    for entry in &summary.matches {
        for participant in &entry.participants {
            if participant.is_self {
                continue;
            }
            for unit in &participant.units {
                if !unit.icon_url.is_empty() {
                    paths.push(unit.icon_url.clone());
                }
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

impl MainWindow {
    /// HTTP 代理配置（仅开启且地址非空时返回）
    fn tft_proxy(&self) -> Option<String> {
        if !self.app_config.general.enable_http_proxy {
            return None;
        }
        let addr = self.app_config.general.http_proxy_addr.trim();
        if addr.is_empty() {
            None
        } else {
            Some(addr.to_string())
        }
    }

    /// 进入云顶路由或切换页签时按需补数据（已有数据不重复请求）
    pub(super) fn ensure_tft_data(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.connection_status, ConnectionStatus::Connected { .. }) {
            return;
        }
        match self.tft_view.active_tab {
            TftTab::Career => {
                if self.tft_view.summary.is_none() && !self.tft_view.loading {
                    self.load_tft_career(cx);
                }
            }
            TftTab::Comps => {
                if self.tft_view.decks.is_empty() && !self.tft_view.comps_loading {
                    self.load_tft_comps(cx);
                }
            }
            TftTab::Augments => {
                if self.tft_view.augments.is_empty() && !self.tft_view.aug_loading {
                    self.load_tft_augments(false, cx);
                }
            }
        }
    }

    /// 拉取云顶段位与近期战绩
    pub(super) fn load_tft_career(&mut self, cx: &mut Context<Self>) {
        if self.tft_view.loading {
            return;
        }
        let service = self.live_service.clone();
        let known_puuid = self
            .own_summoner
            .as_ref()
            .map(|summoner| summoner.puuid.clone())
            .filter(|puuid| !puuid.is_empty());
        let known_version = self.tft_view.client_version.clone();
        let proxy = self.tft_proxy();

        self.tft_view.loading = true;
        self.tft_view.error.clear();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = async move {
                let client = service
                    .get_client()
                    .await
                    .ok_or_else(|| "未连接到英雄联盟客户端".to_string())?;
                let puuid = match known_puuid {
                    Some(puuid) => puuid,
                    None => fetch_current_summoner(&client).await?.puuid,
                };
                let catalog = load_tft_catalog(false, proxy.as_deref())
                    .await
                    .map_err(|err| err.to_string())?;
                let (ranked, summary) = tokio::join!(
                    fetch_tft_ranked_stats(&client, &puuid),
                    fetch_tft_match_history(&client, &puuid, 0, TFT_MATCH_WINDOW, &catalog),
                );
                let ranked = ranked.map_err(|err| err.to_string())?;
                let summary = summary.map_err(|err| err.to_string())?;
                let version = if known_version.is_empty() {
                    fetch_client_version(&client).await
                } else {
                    known_version
                };
                Ok::<_, String>((ranked, summary, version))
            }
            .await;

            let _ = this.update(cx, |view, cx| {
                view.tft_view.loading = false;
                match result {
                    Ok((ranked, summary, version)) => {
                        view.tft_view.error.clear();
                        view.tft_view.ranked = Some(ranked);
                        if !version.is_empty() {
                            view.tft_view.client_version = version;
                        }
                        let paths = tft_icon_paths(&summary);
                        view.tft_view.summary = Some(summary);
                        prewarm_assets(paths, cx);
                    }
                    Err(err) => {
                        log::warn!("[TFT] 生涯数据加载失败: {err}");
                        view.tft_view.error = err;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 拉取海克斯强化百科（LCU/CDragon 资源字典）
    pub(super) fn load_tft_augments(&mut self, force_refresh: bool, cx: &mut Context<Self>) {
        if self.tft_view.aug_loading {
            return;
        }
        let proxy = self.tft_proxy();
        self.tft_view.aug_loading = true;
        self.tft_view.aug_error.clear();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = load_tft_catalog(force_refresh, proxy.as_deref())
                .await
                .map(|catalog| catalog.augments.clone())
                .map_err(|err| err.to_string());

            let _ = this.update(cx, |view, cx| {
                view.tft_view.aug_loading = false;
                match result {
                    Ok(augments) => {
                        view.tft_view.aug_error.clear();
                        view.tft_view.augments = augments;
                    }
                    Err(err) => {
                        log::warn!("[TFT] 海克斯数据加载失败: {err}");
                        view.tft_view.aug_error = err;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 拉取 OP.GG 热门阵容
    pub(super) fn load_tft_comps(&mut self, cx: &mut Context<Self>) {
        if self.tft_view.comps_loading {
            return;
        }
        let proxy = self.tft_proxy();
        self.tft_view.comps_loading = true;
        self.tft_view.comps_error.clear();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = async move {
                let catalog = load_tft_catalog(false, proxy.as_deref())
                    .await
                    .map_err(|err| err.to_string())?;
                fetch_tft_meta_decks(proxy.as_deref(), &catalog)
                    .await
                    .map_err(|err| err.to_string())
            }
            .await;

            let _ = this.update(cx, |view, cx| {
                view.tft_view.comps_loading = false;
                match result {
                    Ok(decks) => {
                        view.tft_view.comps_error.clear();
                        view.tft_view.deck_detail = None;
                        view.tft_view.decks = decks.decks;
                    }
                    Err(err) => {
                        log::warn!("[TFT] 阵容数据加载失败: {err}");
                        view.tft_view.comps_error = err;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn show_tft_toast(&mut self, message: String, success: bool, cx: &mut Context<Self>) {
        let kind = if success {
            ToastKind::Success
        } else {
            ToastKind::Error
        };
        self.toast(kind, message, cx);
    }

    pub(super) fn handle_tft_action(&mut self, action: TftAction, cx: &mut Context<Self>) {
        match action {
            TftAction::SwitchTab(tab) => {
                self.tft_view.active_tab = tab;
                self.tft_view.aug_limit = self.tft_view.aug_limit.max(40);
                cx.notify();
                self.ensure_tft_data(cx);
            }
            TftAction::RefreshCareer => {
                // 不清空 summary：刷新失败时保留上次数据，与阵容/海克斯刷新一致
                self.load_tft_career(cx);
            }
            TftAction::OpenMatch(game_id) => {
                self.tft_view.select_match(game_id);
                cx.notify();
            }
            TftAction::CloseMatchDetail => {
                self.tft_view.close_match();
                cx.notify();
            }
            TftAction::RefreshComps => self.load_tft_comps(cx),
            TftAction::OpenDeck(index) => {
                self.tft_view.select_deck(index);
                cx.notify();
            }
            TftAction::PrevDeck => {
                self.tft_view.shift_deck(false);
                cx.notify();
            }
            TftAction::NextDeck => {
                self.tft_view.shift_deck(true);
                cx.notify();
            }
            TftAction::CloseDeck => {
                self.tft_view.close_deck();
                cx.notify();
            }
            TftAction::CopyTeamCode(code) => {
                if code.trim().is_empty() {
                    self.show_tft_toast("当前阵容没有可复制的阵容码".to_string(), false, cx);
                    return;
                }
                // 对照 Vue I01/I02：以实际写入结果决定提示（剪贴板被占用/失败可感知）
                match platform::try_write_clipboard(&code) {
                    Ok(()) => self.show_tft_toast(
                        "阵容代码已复制到剪贴板，可进入游戏粘贴导入".to_string(),
                        true,
                        cx,
                    ),
                    Err(e) => self.show_tft_toast(format!("复制阵容代码失败: {}", e), false, cx),
                }
            }
            TftAction::RefreshAugments => self.load_tft_augments(true, cx),
            TftAction::LoadMoreAugments => {
                self.tft_view.aug_limit = self.tft_view.aug_limit.saturating_add(40);
                cx.notify();
            }
            TftAction::SetAugmentTier(tier) => {
                self.tft_view.aug_tier = tier;
                self.tft_view.aug_limit = 40;
                cx.notify();
            }
            TftAction::QueryAugments(text) => {
                self.tft_view.aug_query = text;
                self.tft_view.aug_limit = 40;
                cx.notify();
            }
            TftAction::ClearAugmentQuery => {
                self.tft_view.aug_query.clear();
                self.tft_view.aug_limit = 40;
                cx.notify();
            }
        }
    }

    /// 云顶路由渲染
    pub(super) fn render_tft_route(
        &mut self,
        el: Stateful<Div>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let entity = cx.entity().clone();
        let connected = matches!(self.connection_status, ConnectionStatus::Connected { .. });
        let focus_active = self.tft_augment_focus.is_focused(window);
        let view = self.tft_view.render(
            connected,
            &self.tft_augment_focus,
            focus_active,
            move |action, _window, app| {
                entity.update(app, |this, cx| this.handle_tft_action(action, cx));
            },
        );
        el.child(view)
    }
}
