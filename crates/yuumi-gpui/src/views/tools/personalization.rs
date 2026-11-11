//! 个人主页与社交装扮组：签名、主页背景、段位展示、在线状态、称号勋章。
//!
//! 归属工单 D09/D10/D11（预览确认回读、重复写入、真实成就读取与确认框）。
//! 自 `views/tools.rs` 按工单边界搬移，行为未改动。

use super::*;
use std::sync::{OnceLock, RwLock};

/// 装配弹层渲染状态快照（工单 D11）
///
/// `badges_modal` 是自由函数，`render_panel` 的调用点按本工单边界不可修改，
/// 因此弹层的真实数据源、搜索、段位筛选与加载/错误状态由此投影；
/// `handle_tools_action` 在 `entity.update` 中写回（不跨 await 持锁），
/// `badges_modal` 每帧读取只读快照。`selected_badges` / `selected_title`
/// 仍由 `render_panel` 作为参数传入，不在此重复保存。
#[derive(Clone, Default)]
pub struct BadgesViewState {
    pub owned_badges: Vec<crate::services::challenges::ChallengeItem>,
    pub owned_titles: Vec<crate::services::challenges::TitleItem>,
    pub badge_search: String,
    pub title_search: String,
    pub tier_filter: String,
    pub loading: bool,
    pub error: Option<String>,
}

fn badges_state() -> &'static RwLock<BadgesViewState> {
    static STATE: OnceLock<RwLock<BadgesViewState>> = OnceLock::new();
    STATE.get_or_init(|| {
        RwLock::new(BadgesViewState {
            tier_filter: "all".to_string(),
            ..Default::default()
        })
    })
}

/// 弹层渲染读取只读快照
pub fn badges_view_state() -> BadgesViewState {
    badges_state().read().map(|s| s.clone()).unwrap_or_default()
}

/// 由 `handle_tools_action` 写回弹层状态
pub fn update_badges_view_state(f: impl FnOnce(&mut BadgesViewState)) {
    if let Ok(mut state) = badges_state().write() {
        f(&mut state);
    }
}

impl ToolsView {

    // ─────────────────────────────────────────────────────────────
    // 个人主页与社交装扮组实现 (T-V10, T-V11, T-V03)
    // ─────────────────────────────────────────────────────────────

    // 行 15: 自定义个性签名 (折叠卡 - T-V10)
    pub(super) fn render_signature_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("signature");
        let act_toggle = on_action.clone();
        let act_apply = on_action.clone();

        collapse_card(
            "tools-card-signature".into(),
            open,
            Some("icons/copy.svg"),
            "修改个人签名",
            "修改好友列表中展示的个性化状态签名文本",
            Some("点击展开".to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("signature"), win, app),
            setting_row(
                "输入新的个性化签名:",
                Some("留空将清空当前展示的状态签名"),
                suffix_input(
                    "input-signature".into(),
                    &self.signature_buffer,
                    "输入新签名...",
                    "保存签名",
                    false,
                    300.0,
                    move |_ev, win, app| act_apply(ToolsAction::ApplySignature, win, app),
                ),
            ),
        )
    }

    // 行 16: 修改个人主页背景 (折叠卡 - T-V10)
    //
    // D09：选择 / 预览 / 确认 / 回读分离。选英雄只加载真实皮肤列表（零写入），
    // 上一个/下一个与缩略图仅改预览态，取消零写入，确认才写 LCU 并回读。
    pub(super) fn render_profile_bg_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("profilebg");
        let act_toggle = on_action.clone();
        let act_open_picker = on_action.clone();
        let act_close_picker = on_action.clone();
        let act_select_champ = on_action.clone();
        let act_cmd = on_action.clone();
        let act_retry = on_action.clone();

        let current_champ_id = self.bg_selected_champion.unwrap_or(157);
        let champ_ids = vec![current_champ_id];

        // 预览态全部取自真实皮肤列表，不再硬编码 157003
        let preview_idx = self
            .bg_preview_index
            .min(self.bg_skins.len().saturating_sub(1));
        let preview = self.bg_skins.get(preview_idx);
        let preview_name = preview
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "未加载皮肤".to_string());
        let preview_id = preview
            .map(|s| format!("皮肤 ID: {}", s.id))
            .unwrap_or_else(|| "选英雄后自动加载真实皮肤列表".to_string());
        let preview_path = preview
            .map(|s| s.load_screen_path.clone())
            .unwrap_or_default();
        let current_bg = self
            .bg_current_skin_id
            .map(|id| format!("当前主页背景（回读）: 皮肤 ID {}", id))
            .unwrap_or_else(|| "当前主页背景: 未回读".to_string());

        // 预览弹层预先构造，避免 when 闭包持有 &self 引用
        let skin_modal = self.render_skin_preview_modal(on_action.clone());

        collapse_card(
            "tools-card-profilebg".into(),
            open,
            Some("icons/user.svg"),
            "修改个人主页背景",
            "设置个人生涯资料卡片背景展示的英雄及皮肤",
            Some("点击展开".to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("profilebg"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(Theme::TEXT_MUTED())
                        .child("选择背景展示的英雄与皮肤立绘:"),
                )
                // 单英雄选择器（保持既有组件与回调形态）
                .child(champion_picker(
                    "bg-champ-picker".into(),
                    &champ_ids,
                    self.open_bg_champion_picker,
                    "",
                    move |_ev, _win, _app| {},
                    move |_ev, win, app| act_open_picker(ToolsAction::TogglePicker("bg_champ"), win, app),
                    move |_ev, win, app| act_close_picker(ToolsAction::TogglePicker("bg_champ"), win, app),
                    move |cid, win, app| act_select_champ(ToolsAction::SelectBgChampion(cid), win, app),
                    move |_cid, _win, _app| {},
                ))
                // 加载态
                .when(self.bg_loading, |this| {
                    this.child(
                        div()
                            .p(px(12.0))
                            .rounded_md()
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .bg(Theme::HOVER_BG())
                            .text_size(px(12.0))
                            .text_color(Theme::TEXT_MUTED())
                            .child("正在读取该英雄的真实皮肤列表…"),
                    )
                })
                // 失败态：真实错误文案 + 重新加载（零写入）
                .when_some(self.bg_error.clone(), |this, err| {
                    let act = act_retry.clone();
                    this.child(
                        div()
                            .p(px(12.0))
                            .rounded_md()
                            .border_1()
                            .border_color(Theme::LOSS_BORDER())
                            .bg(Theme::LOSS_BG())
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(Theme::LOSS_TEXT())
                                    .child(err.clone()),
                            )
                            .child(
                                div()
                                    .px(px(12.0))
                                    .py(px(5.0))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(Theme::LOSS_BORDER())
                                    .text_size(px(12.0))
                                    .text_color(Theme::LOSS_TEXT())
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::CARD_BG()))
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        act(ToolsAction::ProfileBgSkinInput("retry".to_string()), win, app)
                                    })
                                    .child("重试加载"),
                            ),
                    )
                })
                // 皮肤预览卡片（真实名称 / ID / 立绘 / 回读值），「选择皮肤」零写入
                .when(!self.bg_loading && self.bg_error.is_none(), |this| {
                    let act = act_cmd.clone();
                    this.child(
                        div()
                            .p(px(12.0))
                            .rounded_md()
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .bg(Theme::HOVER_BG())
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(
                                        crate::components::lcu_image::LcuImage::new(preview_path.clone())
                                            .size(px(48.0))
                                            .placeholder("🖼️"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(Theme::TEXT_PRIMARY())
                                                    .child(preview_name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(Theme::TEXT_MUTED())
                                                    .child(preview_id.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(Theme::TEXT_MUTED())
                                                    .child(current_bg.clone()),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .px(px(14.0))
                                    .py(px(5.0))
                                    .rounded_md()
                                    .bg(Theme::ACCENT_PRIMARY())
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(gpui::white())
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.85))
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        act(ToolsAction::ProfileBgSkinInput("open".to_string()), win, app)
                                    })
                                    .child("选择皮肤"),
                            ),
                    )
                })
                // 皮肤预览弹层（内联展开面板，等价旧版弹窗语义）
                .when(self.open_bg_skin_modal, |this| this.child(skin_modal)),
        )
    }

    /// D09 皮肤预览弹层：上一个/下一个、缩略图、取消零写入、确认才写入并回读。
    /// 立绘路径交给 `LcuImage`（`yuumi-asset://` 自定义协议），不走 IPC Base64。
    fn render_skin_preview_modal<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let act_close = on_action.clone();
        let act_cancel = on_action.clone();
        let act_prev = on_action.clone();
        let act_next = on_action.clone();
        let act_confirm = on_action.clone();
        let act_retry = on_action.clone();
        let act_thumb = on_action.clone();

        let skins = self.bg_skins.clone();
        let idx = self.bg_preview_index.min(skins.len().saturating_sub(1));
        let current = skins.get(idx).cloned();
        let (name, id_label, path) = match &current {
            Some(s) => (
                s.name.clone(),
                format!("皮肤 ID: {}", s.id),
                s.load_screen_path.clone(),
            ),
            None => (
                "无可用皮肤".to_string(),
                "请先选择英雄加载皮肤列表".to_string(),
                String::new(),
            ),
        };
        let count = skins.len();

        div()
            .w_full()
            .p(px(12.0))
            .rounded_md()
            .border_1()
            .border_color(Theme::SIDEBAR_ACTIVE_BORDER())
            .bg(Theme::CARD_BG())
            .flex()
            .flex_col()
            .gap(px(10.0))
            // 标题 + 关闭（取消）
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::TEXT_PRIMARY())
                            .child("预览并确认主页背景"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(Theme::TEXT_MUTED())
                            .cursor_pointer()
                            .hover(|s| s.text_color(Theme::TEXT_PRIMARY()))
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_close(ToolsAction::ProfileBgSkinInput("cancel".to_string()), win, app)
                            })
                            .child("✕"),
                    ),
            )
            // 主预览图 + 名称 / ID / 计数
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        crate::components::lcu_image::LcuImage::new(path)
                            .size(px(96.0))
                            .placeholder("🖼️"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(3.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY())
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(Theme::TEXT_MUTED())
                                    .child(id_label),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(Theme::TEXT_MUTED())
                                    .child(format!("第 {} / {} 款", idx + 1, count)),
                            ),
                    ),
            )
            // 上一个 / 下一个（循环边界，仅改预览态）
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .px(px(12.0))
                            .py(px(4.0))
                            .rounded_md()
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .bg(Theme::CARD_BG())
                            .text_size(px(12.0))
                            .text_color(Theme::TEXT_SECONDARY())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::HOVER_BG()))
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_prev(ToolsAction::ProfileBgSkinInput("prev".to_string()), win, app)
                            })
                            .child("← 上一个"),
                    )
                    .child(
                        div()
                            .px(px(12.0))
                            .py(px(4.0))
                            .rounded_md()
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .bg(Theme::CARD_BG())
                            .text_size(px(12.0))
                            .text_color(Theme::TEXT_SECONDARY())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::HOVER_BG()))
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_next(ToolsAction::ProfileBgSkinInput("next".to_string()), win, app)
                            })
                            .child("下一个 →"),
                    ),
            )
            // 缩略图条：点击仅改预览态（SelectBgSkin 零写入）
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_wrap()
                    .gap(px(6.0))
                    .children(skins.iter().enumerate().map(move |(i, s)| {
                        let sid = s.id;
                        let thumb_path = s.load_screen_path.clone();
                        let act = act_thumb.clone();
                        let active = i == idx;
                        div()
                            .flex_none()
                            .rounded_md()
                            .border_1()
                            .border_color(if active {
                                Theme::ACCENT_PRIMARY()
                            } else {
                                Theme::CARD_BORDER()
                            })
                            .cursor_pointer()
                            .hover(|s| s.border_color(Theme::ACCENT_PRIMARY()))
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act(ToolsAction::SelectBgSkin(sid), win, app)
                            })
                            .child(
                                crate::components::lcu_image::LcuImage::new(thumb_path)
                                    .size(px(40.0))
                                    .placeholder("·"),
                            )
                    })),
            )
            // 写入失败：保留弹层 + 真实错误文案 + 重试应用
            .when_some(self.bg_error.clone(), |this, err| {
                let act = act_retry.clone();
                this.child(
                    div()
                        .p(px(8.0))
                        .rounded_md()
                        .bg(Theme::LOSS_BG())
                        .border_1()
                        .border_color(Theme::LOSS_BORDER())
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(Theme::LOSS_TEXT())
                                .child(err.clone()),
                        )
                        .child(
                            div()
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded_md()
                                .border_1()
                                .border_color(Theme::LOSS_BORDER())
                                .text_size(px(11.5))
                                .text_color(Theme::LOSS_TEXT())
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    act(ToolsAction::ApplyProfileBackground, win, app)
                                })
                                .child("重试应用"),
                        ),
                )
            })
            // 取消（零写入） / 确认（写入 + 回读）
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .pt(px(4.0))
                    .border_t_1()
                    .border_color(Theme::CARD_BORDER())
                    .child(
                        div()
                            .px(px(14.0))
                            .py(px(6.0))
                            .rounded_md()
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .bg(Theme::CARD_BG())
                            .text_size(px(12.0))
                            .text_color(Theme::TEXT_SECONDARY())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::HOVER_BG()))
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_cancel(ToolsAction::ProfileBgSkinInput("cancel".to_string()), win, app)
                            })
                            .child("取消（不写入）"),
                    )
                    .child(
                        div()
                            .px(px(18.0))
                            .py(px(6.0))
                            .rounded_md()
                            .bg(Theme::ACCENT_PRIMARY())
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(gpui::white())
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_confirm(ToolsAction::ApplyProfileBackground, win, app)
                            })
                            .child("确认应用"),
                    ),
            )
    }

    // 行 17: 修改段位展示 (折叠卡 - T-V11)
    pub(super) fn render_rank_spoof_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("rankspoof");
        let act_toggle = on_action.clone();
        let act_apply = on_action.clone();
        let act_toggle_q = on_action.clone();
        let act_toggle_t = on_action.clone();
        let act_toggle_d = on_action.clone();
        let act_select_q = on_action.clone();
        let act_select_t = on_action.clone();
        let act_select_d = on_action.clone();

        let queue_label = SPOOF_QUEUES
            .iter()
            .find(|(q, _)| *q == self.spoof_queue)
            .map(|(_, n)| *n)
            .unwrap_or("单双排位");

        let tier_label = SPOOF_TIERS
            .iter()
            .find(|(t, _)| *t == self.spoof_tier)
            .map(|(_, n)| *n)
            .unwrap_or("最强王者");

        // 王者、宗师、大师、未定级时小段禁用
        let division_disabled = matches!(
            self.spoof_tier.as_str(),
            "CHALLENGER" | "GRANDMASTER" | "MASTER" | "UNRANKED"
        );

        collapse_card(
            "tools-card-rankspoof".into(),
            open,
            Some("icons/bookmark.svg"),
            "修改生涯与卡片段位展示",
            "修改好友聊天中展示的伪装段位图标与阶位",
            Some("点击展开".to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("rankspoof"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                // 行 1: 140px 队列下拉
                .child(setting_row(
                    "选择展示队列",
                    Some("选择要在个人卡片上模拟的排位赛模式"),
                    select_dropdown(
                        "dropdown-spoof-queue",
                        140.0,
                        queue_label.to_string(),
                        self.open_spoof_queue_dropdown,
                        false,
                        SPOOF_QUEUES.to_vec(),
                        move |win, app| act_toggle_q(ToolsAction::ToggleDropdownMenu("spoof_queue"), win, app),
                        move |val, win, app| act_select_q(ToolsAction::SetSpoofQueue(val.to_string()), win, app),
                    ),
                ))
                // 行 2: 段位与阶位 (130px 段位 + 80px 小段 + 应用按钮)
                .child(setting_row(
                    "段位与阶位",
                    Some("设定好友名片与浮窗预览呈现的段位阶层"),
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(select_dropdown(
                            "dropdown-spoof-tier",
                            130.0,
                            tier_label.to_string(),
                            self.open_spoof_tier_dropdown,
                            false,
                            SPOOF_TIERS.to_vec(),
                            move |win, app| act_toggle_t(ToolsAction::ToggleDropdownMenu("spoof_tier"), win, app),
                            move |val, win, app| act_select_t(ToolsAction::SetSpoofTier(val.to_string()), win, app),
                        ))
                        .child(select_dropdown(
                            "dropdown-spoof-div",
                            80.0,
                            self.spoof_division.clone(),
                            self.open_spoof_division_dropdown,
                            division_disabled,
                            SPOOF_DIVISIONS.iter().map(|&d| (d, d)).collect(),
                            move |win, app| act_toggle_d(ToolsAction::ToggleDropdownMenu("spoof_division"), win, app),
                            move |val, win, app| act_select_d(ToolsAction::SetSpoofDivision(val.to_string()), win, app),
                        ))
                        .child(
                            div()
                                .px(px(12.0))
                                .py(px(5.0))
                                .rounded_md()
                                .bg(Theme::ACCENT_PRIMARY())
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(gpui::white())
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.85))
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    act_apply(ToolsAction::ApplyRankSpoof, win, app)
                                })
                                .child("应用伪装段位"),
                        ),
                )),
        )
    }

    // 行 18: 修改在线状态 (折叠卡 - T-V11)
    pub(super) fn render_online_status_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("onlinestatus");
        let act_toggle = on_action.clone();
        let act_chat = on_action.clone();
        let act_away = on_action.clone();
        let act_offline = on_action.clone();

        let status_desc = match self.online_status.as_str() {
            "away" => "当前状态: 离开 (Away)",
            "offline" | "mobile" => "当前状态: 离线 (Offline)",
            _ => "当前状态: 在线 (Online)",
        };

        collapse_card(
            "tools-card-onlinestatus".into(),
            open,
            Some("icons/menu.svg"),
            "修改在线状态",
            "自由切换在好友列表中的显示状态 (在线 / 离开 / 移动端离线)",
            Some(status_desc.to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("onlinestatus"), win, app),
            setting_row(
                "在线状态",
                Some("切换你在好友列表中的当前状态呈现"),
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(self.render_status_btn("设为在线", self.online_status == "chat", move |win, app| {
                        act_chat(ToolsAction::SetOnlineStatus("chat".to_string()), win, app)
                    }))
                    .child(self.render_status_btn("设为离开", self.online_status == "away", move |win, app| {
                        act_away(ToolsAction::SetOnlineStatus("away".to_string()), win, app)
                    }))
                    .child(self.render_status_btn("设为离线", self.online_status == "offline", move |win, app| {
                        act_offline(ToolsAction::SetOnlineStatus("offline".to_string()), win, app)
                    })),
            ),
        )
    }

    pub(super) fn render_status_btn<F>(&self, label: &'static str, active: bool, on_click: F) -> Div
    where
        F: Fn(&mut Window, &mut App) + 'static + Clone,
    {
        div()
            .px(px(14.0))
            .py(px(5.0))
            .rounded_md()
            .border_1()
            .border_color(if active {
                Theme::ACCENT_PRIMARY()
            } else {
                Theme::CARD_BORDER()
            })
            .bg(if active {
                Theme::ACCENT_LIGHT()
            } else {
                Theme::CARD_BG()
            })
            .text_size(px(12.0))
            .font_weight(if active {
                FontWeight::BOLD
            } else {
                FontWeight::MEDIUM
            })
            .text_color(if active {
                Theme::ACCENT_PRIMARY()
            } else {
                Theme::TEXT_PRIMARY()
            })
            .cursor_pointer()
            .hover(|s| s.border_color(Theme::ACCENT_PRIMARY()))
            .on_mouse_down(MouseButton::Left, move |_ev, win, app| on_click(win, app))
            .child(label)
    }

    // 行 19: 自定义装配 (直显行 - T-V03)
    pub(super) fn render_badges_preferences_row<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let act_open = on_action.clone();

        direct_card_row(
            "tools-row-badges".into(),
            CardGroupPosition::Top,
            "icons/bookmark.svg",
            "装配称号与挑战勋章",
            "自定义佩戴个人名片的称号与 3 枚挑战勋章",
            div()
                .px(px(14.0))
                .py(px(5.0))
                .rounded_md()
                .border_1()
                .border_color(Theme::ACCENT_PRIMARY())
                .bg(Theme::CARD_BG())
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(Theme::ACCENT_PRIMARY())
                .cursor_pointer()
                .hover(|s| s.bg(Theme::ACCENT_LIGHT()))
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                    act_open(ToolsAction::ToggleBadgesModal(true), win, app)
                })
                .child("自定义装配"),
        )
    }

    // 行 20: 卸下全部勋章 (直显行 - T-V03)
    // D11-e：点击「卸下」只展开二次确认框（零写入），确认后才发真实请求
    pub(super) fn render_clear_badges_row<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let act_clear = on_action.clone();

        let row = direct_card_row(
            "tools-row-clearbadges".into(),
            CardGroupPosition::Middle,
            "icons/close.svg",
            "卸下全部勋章",
            "一键卸下所有已佩戴的挑战勋章",
            danger_button("卸下", move |_ev, win, app| {
                act_clear(ToolsAction::ClearBadges, win, app)
            }),
        );

        let confirm = self.render_clear_confirm(
            "确认卸下全部勋章",
            "仅清空挑战勋章（challengeIds），当前称号与头像框保持不变。此操作会写入客户端。",
            ToolsAction::ClearBadges,
            on_action.clone(),
        );

        div()
            .flex()
            .flex_col()
            .child(row)
            .when(self.confirm_clear_badges, |this| this.child(confirm))
    }

    // 行 21: 卸下头像边框 (直显行 - T-V03)
    // D11-e：点击「卸下」只展开二次确认框（零写入），确认后才发真实请求
    pub(super) fn render_clear_border_row<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let act_clear = on_action.clone();

        let row = direct_card_row(
            "tools-row-clearborder".into(),
            CardGroupPosition::Bottom,
            "icons/close.svg",
            "卸下头像边框",
            "移除个人主页中配置的旗帜与头像边框",
            danger_button("卸下", move |_ev, win, app| {
                act_clear(ToolsAction::ClearBorder, win, app)
            }),
        );

        let confirm = self.render_clear_confirm(
            "确认卸下头像边框",
            "仅清除头像边框（preferredBorderType=NONE），不影响已佩戴的挑战勋章与称号。",
            ToolsAction::ClearBorder,
            on_action.clone(),
        );

        div()
            .flex()
            .flex_col()
            .child(row)
            .when(self.confirm_clear_border, |this| this.child(confirm))
    }

    /// D11-e 清除操作的二次确认框（内联展开）；取消零写入
    fn render_clear_confirm<F>(
        &self,
        title: &'static str,
        desc: &'static str,
        confirm_action: ToolsAction,
        on_action: F,
    ) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let act_confirm = on_action.clone();
        let act_cancel = on_action.clone();

        div()
            .mt(px(4.0))
            .p(px(12.0))
            .rounded_md()
            .border_1()
            .border_color(Theme::LOSS_BORDER())
            .bg(Theme::LOSS_BG())
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::LOSS_TEXT())
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(Theme::TEXT_MUTED())
                    .child(desc),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .child(
                        div()
                            .px(px(12.0))
                            .py(px(5.0))
                            .rounded_md()
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .bg(Theme::CARD_BG())
                            .text_size(px(12.0))
                            .text_color(Theme::TEXT_SECONDARY())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::HOVER_BG()))
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_cancel(
                                    ToolsAction::ToggleBadgeItem("cancel-clear".to_string()),
                                    win,
                                    app,
                                )
                            })
                            .child("取消"),
                    )
                    .child(
                        div()
                            .px(px(14.0))
                            .py(px(5.0))
                            .rounded_md()
                            .bg(Theme::LOSS_TEXT())
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(gpui::white())
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_confirm(confirm_action.clone(), win, app)
                            })
                            .child("确认"),
                    ),
            )
    }
}
