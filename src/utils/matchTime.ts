/**
 * 战绩时间的本地时区格式化。
 *
 * 后端 `MatchDisplay` 同时下发两种时间表示：
 * - `time` / `shortTime`：Rust 侧用 `chrono::DateTime::from_timestamp` 生成的 **UTC** 字符串。
 *   之所以不直接在 Rust 里转本地时区，是为了让单元测试与运行机器时区无关。
 * - `timeStamp`：原始 epoch 毫秒，由前端按浏览器本地时区格式化。
 *
 * 因此凡是面向用户展示的时间/日期，都必须从 `timeStamp` 派生，不要直接使用 `time` /
 * `shortTime` —— 后者会让时间早 8 小时（UTC+8），日期在本地 00:00–07:59 开局的对局上还会差一天。
 */

const pad = (n: number) => n.toString().padStart(2, "0");

/** epoch 毫秒 → 本地 `YYYY/MM/DD HH:mm` */
export function formatLocalDateTime(ts?: number | null): string {
  if (!ts || ts <= 0) return "";
  const d = new Date(ts);
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** epoch 毫秒 → 本地 `MM-DD`（与后端 shortTime 的日期段格式保持一致，便于替换） */
export function formatLocalMonthDay(ts?: number | null): string {
  if (!ts || ts <= 0) return "";
  const d = new Date(ts);
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}
