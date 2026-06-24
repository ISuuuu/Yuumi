/**
 * 动态更新系统主题色 CSS 变量
 * @param color 16进制颜色值，如 #6c5ce7
 */
export function updateThemeColor(color: string) {
  if (!color) return;
  const root = document.documentElement;
  root.style.setProperty("--primary-color", color);
  root.style.setProperty("--primary-color-hover", color + "cc"); // 加上 80% 不透明度的 hover 色
  
  // 提取 RGB，支持转换为各种带透明度的 css rgba 变量
  const hex = color.replace("#", "");
  const r = parseInt(hex.substring(0, 2), 16) || 108;
  const g = parseInt(hex.substring(2, 4), 16) || 92;
  const b = parseInt(hex.substring(4, 6), 16) || 231;
  root.style.setProperty("--primary-color-alpha-15", `rgba(${r}, ${g}, ${b}, 0.15)`);
  root.style.setProperty("--primary-color-alpha-30", `rgba(${r}, ${g}, ${b}, 0.3)`);
  root.style.setProperty("--primary-color-alpha-40", `rgba(${r}, ${g}, ${b}, 0.4)`);
}
