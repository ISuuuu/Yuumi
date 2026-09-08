import { marked } from "marked";
import DOMPurify from "dompurify";

/**
 * 安全渲染 Markdown 为 HTML 字符串，经过 DOMPurify 严格消毒，防止 XSS 攻击
 */
export function renderMarkdown(content: string): string {
  if (!content) return "";
  try {
    const rawHtml = marked.parse(content) as string;
    return DOMPurify.sanitize(rawHtml);
  } catch (err) {
    console.error("[Markdown] 解析 Markdown 失败:", err);
    return DOMPurify.sanitize(content);
  }
}
