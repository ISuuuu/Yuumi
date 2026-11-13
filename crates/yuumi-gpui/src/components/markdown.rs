//! 极简 Markdown 渲染器（更新日志弹窗专用）。
//!
//! 只覆盖 GitHub Release 文案实际使用的语法子集：
//! 标题 (`#`~`####`)、无序/有序列表、引用 (`>`)、分隔线 (`---`)，
//! 行内 `**粗体**`、`` `行内代码` ``、`[链接](url)`（可点击，经 `App::open_url` 打开）。
//! 其余语法按普通文本段落降级，渲染观感对齐 Vue 版 `marked` + `markdown-body`。

use std::ops::Range;

use crate::theme::Theme;
use gpui::*;

/// 行内样式段
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum InlineStyle {
    Plain,
    Bold,
    Code,
    /// 链接目标 URL
    Link(String),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct InlineSpan {
    pub text: String,
    pub style: InlineStyle,
}

/// 块级元素
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Block {
    Heading(u8, String),
    /// 无序列表 `marker=None`，有序列表携带编号
    ListItem(Option<String>, String),
    Quote(String),
    Rule,
    Paragraph(String),
}

/// 块级解析：逐行扫描，连续普通行各自独立成段（中文文案不做空格拼接）。
pub(crate) fn parse_blocks(src: &str) -> Vec<Block> {
    src.lines()
        .filter_map(|line| {
            let t = line.trim_start();
            if t.is_empty() {
                return None;
            }
            for (prefix, level) in [("# ", 1u8), ("## ", 2), ("### ", 3), ("#### ", 4)] {
                if let Some(rest) = t.strip_prefix(prefix) {
                    return Some(Block::Heading(level, rest.trim().into()));
                }
            }
            if is_horizontal_rule(t) {
                return Some(Block::Rule);
            }
            if t == ">" || t.starts_with("> ") {
                return Some(Block::Quote(t.trim_start_matches('>').trim().into()));
            }
            for marker in ["- ", "* ", "+ "] {
                if let Some(rest) = t.strip_prefix(marker) {
                    return Some(Block::ListItem(None, rest.into()));
                }
            }
            if let Some(rest) = ordered_list_rest(t) {
                let (num, body) = rest;
                return Some(Block::ListItem(Some(num), body.into()));
            }
            Some(Block::Paragraph(t.into()))
        })
        .collect()
}

fn is_horizontal_rule(t: &str) -> bool {
    t.len() >= 3 && ['-', '*', '_'].iter().any(|c| t.chars().all(|ch| ch == *c))
}

/// 匹配 `1. ` / `12) ` 形式的有序列表项，返回 (编号, 正文)
fn ordered_list_rest(t: &str) -> Option<(String, &str)> {
    let digits: &str = t
        .split_once(|c: char| !c.is_ascii_digit())
        .map(|(d, _)| d)?;
    if digits.is_empty() {
        return None;
    }
    let after = &t[digits.len()..];
    let body = after
        .strip_prefix(". ")
        .or_else(|| after.strip_prefix(") "))?;
    Some((digits.into(), body))
}

/// 行内解析：识别 `**粗体**`、`` `代码` `` 与 `[文本](URL)` 链接；
/// 未闭合的标记字符按普通文本降级，不嵌套解析。
pub(crate) fn parse_inline(text: &str) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    let mut plain = String::new();
    let mut rest = text;

    let flush_plain = |plain: &mut String, spans: &mut Vec<InlineSpan>| {
        if !plain.is_empty() {
            spans.push(InlineSpan {
                text: std::mem::take(plain),
                style: InlineStyle::Plain,
            });
        }
    };

    while !rest.is_empty() {
        let bold_at = rest.find("**");
        let code_at = rest.find('`');
        let link_at = rest
            .find('[')
            .and_then(|at| find_link(&rest[at..]).map(|_| at));
        let next = bold_at.into_iter().chain(code_at).chain(link_at).min();

        match next {
            None => {
                plain.push_str(rest);
                break;
            }
            Some(at) => {
                plain.push_str(&rest[..at]);
                rest = &rest[at..];
                if bold_at == Some(at) {
                    if let Some(end) = rest[2..].find("**") {
                        flush_plain(&mut plain, &mut spans);
                        spans.push(InlineSpan {
                            text: rest[2..2 + end].into(),
                            style: InlineStyle::Bold,
                        });
                        rest = &rest[2 + end + 2..];
                        continue;
                    }
                } else if code_at == Some(at) {
                    if let Some(end) = rest[1..].find('`') {
                        flush_plain(&mut plain, &mut spans);
                        spans.push(InlineSpan {
                            text: rest[1..1 + end].into(),
                            style: InlineStyle::Code,
                        });
                        rest = &rest[1 + end + 1..];
                        continue;
                    }
                } else if let Some((display, url)) = rest
                    .find('[')
                    .filter(|at| *at == 0)
                    .and_then(|_| find_link(rest))
                {
                    flush_plain(&mut plain, &mut spans);
                    spans.push(InlineSpan {
                        text: display,
                        style: InlineStyle::Link(url),
                    });
                    // 跳过 [text](url) 整段
                    let consumed = rest.find(')').map(|p| p + 1).unwrap_or(rest.len());
                    rest = &rest[consumed..];
                    continue;
                }
                // 未闭合标记降级为普通文本字符
                let mut chars = rest.chars();
                plain.push(chars.next().unwrap_or_default());
                rest = chars.as_str();
            }
        }
    }
    flush_plain(&mut plain, &mut spans);
    spans
}

/// 解析从 `[` 开始的链接 `[文本](URL)`，返回 (显示文本, URL)
fn find_link(rest: &str) -> Option<(String, String)> {
    let start = rest.find('[')?;
    let mid = rest[start..].find("](")? + start;
    let end = rest[mid..].find(')')? + mid;
    let display = &rest[start + 1..mid];
    let url = &rest[mid + 2..end];
    (!display.is_empty() && !url.is_empty()).then(|| (display.to_string(), url.to_string()))
}

/// 渲染 Markdown 为块元素列表；字号与配色由各块自持，容器只需提供纵向布局。
pub fn render_markdown(md: &str) -> Vec<AnyElement> {
    let mut elements = Vec::new();
    for (ix, block) in parse_blocks(md).into_iter().enumerate() {
        match block {
            Block::Heading(level, text) => {
                let size = match level {
                    1..=2 => px(15.0),
                    3 => px(14.0),
                    _ => px(13.0),
                };
                elements.push(
                    div()
                        .text_size(size)
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::TEXT_PRIMARY())
                        .child(inline_element(&parse_inline(&text), ix))
                        .into_any_element(),
                );
            }
            Block::ListItem(marker, text) => {
                let marker_text = marker.unwrap_or_else(|| "•".into());
                elements.push(
                    div()
                        .flex()
                        .flex_row()
                        .pl(px(10.0))
                        .gap(px(8.0))
                        .text_size(px(12.5))
                        .line_height(relative(1.5))
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_color(Theme::TEXT_MUTED())
                                .child(marker_text),
                        )
                        .child(inline_element(&parse_inline(&text), ix))
                        .into_any_element(),
                );
            }
            Block::Quote(text) => {
                elements.push(
                    div()
                        .flex()
                        .flex_row()
                        .border_l_2()
                        .border_color(Theme::ACCENT_PRIMARY().opacity(0.45))
                        .pl(px(10.0))
                        .text_size(px(12.5))
                        .line_height(relative(1.5))
                        .text_color(Theme::TEXT_MUTED())
                        .child(inline_element(&parse_inline(&text), ix))
                        .into_any_element(),
                );
            }
            Block::Rule => {
                elements.push(
                    div()
                        .w_full()
                        .h(px(1.0))
                        .bg(Theme::CARD_BORDER())
                        .into_any_element(),
                );
            }
            Block::Paragraph(text) => {
                elements.push(
                    div()
                        .text_size(px(12.5))
                        .line_height(relative(1.5))
                        .text_color(Theme::TEXT_SECONDARY())
                        .child(inline_element(&parse_inline(&text), ix))
                        .into_any_element(),
                );
            }
        }
    }
    elements
}

/// 将行内片段渲染为富文本；含链接时包装为可点击的 InteractiveText（`line_id` 用于元素标识）。
fn inline_element(spans: &[InlineSpan], line_id: usize) -> AnyElement {
    let mut text = String::new();
    let mut runs = Vec::new();
    let mut link_ranges: Vec<Range<usize>> = Vec::new();
    let mut link_urls: Vec<String> = Vec::new();

    for span in spans {
        let start = text.len();
        text.push_str(&span.text);
        let len = text.len() - start;
        if len == 0 {
            continue;
        }
        let font = font(Theme::font_family());
        let (font, color, background, underline) = match &span.style {
            InlineStyle::Plain => (font, Theme::TEXT_SECONDARY(), None, None),
            InlineStyle::Bold => (font.bold(), Theme::TEXT_PRIMARY(), None, None),
            InlineStyle::Code => (font, Theme::TEXT_SECONDARY(), Some(Theme::HOVER_BG()), None),
            InlineStyle::Link(url) => {
                link_ranges.push(start..text.len());
                link_urls.push(url.clone());
                (
                    font,
                    Theme::ACCENT_PRIMARY(),
                    None,
                    Some(UnderlineStyle {
                        thickness: px(1.0),
                        color: Some(Theme::ACCENT_PRIMARY()),
                        wavy: false,
                    }),
                )
            }
        };
        runs.push(TextRun {
            len,
            font,
            color,
            background_color: background,
            underline,
            strikethrough: None,
        });
    }

    let styled = StyledText::new(text).with_runs(runs);
    if link_ranges.is_empty() {
        styled.into_any_element()
    } else {
        InteractiveText::new(ElementId::from(("md-line", line_id)), styled)
            .on_click(link_ranges, move |ix, _window, cx| {
                if let Some(url) = link_urls.get(ix) {
                    cx.open_url(url);
                }
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    // 精确导入规避 gpui::test 宏遮蔽标准 #[test]（见 theme.rs 先例）
    use super::{parse_blocks, parse_inline, Block, InlineSpan, InlineStyle};

    #[test]
    fn parse_blocks_covers_changelog_subset() {
        let blocks = parse_blocks(
            "## [0.5.0] - 2026-11-13\n\n### 新增\n\n- **主窗口**：无边框窗口\n- 支持 `Mica` 特效\n1. 第一项\n2. 第二项\n\n> 引用说明\n\n---\n\n普通段落文本\n",
        );
        assert_eq!(
            blocks,
            vec![
                Block::Heading(2, "[0.5.0] - 2026-11-13".into()),
                Block::Heading(3, "新增".into()),
                Block::ListItem(None, "**主窗口**：无边框窗口".into()),
                Block::ListItem(None, "支持 `Mica` 特效".into()),
                Block::ListItem(Some("1".into()), "第一项".into()),
                Block::ListItem(Some("2".into()), "第二项".into()),
                Block::Quote("引用说明".into()),
                Block::Rule,
                Block::Paragraph("普通段落文本".into()),
            ]
        );
    }

    #[test]
    fn parse_inline_mixed_styles() {
        let spans = parse_inline("由 `yuumi-core` 提供 **Windows 11** Mica，参考 [Keep a Changelog](https://example.com) 文档");
        assert_eq!(
            spans,
            vec![
                InlineSpan {
                    text: "由 ".into(),
                    style: InlineStyle::Plain
                },
                InlineSpan {
                    text: "yuumi-core".into(),
                    style: InlineStyle::Code
                },
                InlineSpan {
                    text: " 提供 ".into(),
                    style: InlineStyle::Plain
                },
                InlineSpan {
                    text: "Windows 11".into(),
                    style: InlineStyle::Bold
                },
                InlineSpan {
                    text: " Mica，参考 ".into(),
                    style: InlineStyle::Plain
                },
                InlineSpan {
                    text: "Keep a Changelog".into(),
                    style: InlineStyle::Link("https://example.com".into()),
                },
                InlineSpan {
                    text: " 文档".into(),
                    style: InlineStyle::Plain
                },
            ]
        );
    }

    #[test]
    fn unterminated_markers_fall_back_to_plain_text() {
        let spans = parse_inline("a **b `c [d]( e");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].style, InlineStyle::Plain);
        // 未闭合标记降级后原文完整保留（无信息丢失）
        assert_eq!(spans[0].text, "a **b `c [d]( e");
    }

    #[test]
    fn backtick_bold_candidates_not_confused_by_link_bracket() {
        // 链接前存在更早的粗体起点时，应优先处理粗体
        let spans = parse_inline("**bold** then [link](https://x.y)");
        assert_eq!(
            spans,
            vec![
                InlineSpan {
                    text: "bold".into(),
                    style: InlineStyle::Bold
                },
                InlineSpan {
                    text: " then ".into(),
                    style: InlineStyle::Plain
                },
                InlineSpan {
                    text: "link".into(),
                    style: InlineStyle::Link("https://x.y".into()),
                },
            ]
        );
    }

    #[test]
    fn horizontal_rule_requires_repeated_char() {
        assert!(parse_blocks("---").contains(&Block::Rule));
        assert!(!parse_blocks("- item")
            .iter()
            .any(|b| matches!(b, Block::Rule)));
    }
}
