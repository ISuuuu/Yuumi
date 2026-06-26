/**
 * 动态更新系统主题色 CSS 变量
 * @param color 16进制颜色值，如 #6c5ce7 或 #ff6c5ce7
 */
export function updateThemeColor(color: string) {
  if (!color) return;
  const root = document.documentElement;
  
  // 统一转换为 6 位的 hex，用于 --primary-color，防止 CSS 渲染带透明度的 8 位 hex 颜色导致失真
  const hex6 = toHex6(color);
  root.style.setProperty("--primary-color", hex6);
  root.style.setProperty("--primary-color-hover", hex6 + "cc"); // 加上 80% 不透明度的 hover 色
  
  // 提取 RGB，支持转换为各种带透明度的 css rgba 变量
  const hex = hex6.replace("#", "");
  const r = parseInt(hex.substring(0, 2), 16) || 0;
  const g = parseInt(hex.substring(2, 4), 16) || 159;
  const b = parseInt(hex.substring(4, 6), 16) || 170;
  root.style.setProperty("--primary-color-alpha-15", `rgba(${r}, ${g}, ${b}, 0.15)`);
  root.style.setProperty("--primary-color-alpha-30", `rgba(${r}, ${g}, ${b}, 0.3)`);
  root.style.setProperty("--primary-color-alpha-40", `rgba(${r}, ${g}, ${b}, 0.4)`);
}

/**
 * 将 8位 (#aarrggbb) 或 6位 (#rrggbb) 颜色转为 6 位 hex
 */
export function toHex6(color: string): string {
  if (!color) return "";
  if (color.startsWith("#") && color.length === 9) {
    return "#" + color.slice(3);
  }
  return color;
}

/**
 * 动态更新死亡数字颜色 CSS 变量，根据当前主题（浅色/深色）设置 --death-color
 * @param lightColor 浅色主题下的颜色（8位或6位hex）
 * @param darkColor  深色主题下的颜色（8位或6位hex）
 */
export function updateDeathColor(lightColor: string, darkColor: string) {
  const root = document.documentElement;
  const isDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const color = toHex6(isDark ? darkColor : lightColor);
  if (color) {
    root.style.setProperty("--death-color", color);
  }
}

/**
 * 应用界面缩放。
 * 'Auto' 表示跟随系统，移除缩放；否则按百分比字符串（如 '125'）应用。
 * 使用最标准的根字体大小实现页面整体缩放，不使用会导致布局模糊/崩溃的 zoom。
 * @param dpiScale 'Auto' | '100' | '125' | '150'
 */
export function applyDpiScale(dpiScale: string) {
  const root = document.documentElement;
  // 确保先清除之前的遗留属性
  document.body.style.removeProperty("zoom");
  
  if (!dpiScale || dpiScale === "Auto" || dpiScale === "100") {
    root.style.removeProperty("font-size");
  } else {
    const pct = parseInt(dpiScale, 10);
    if (!isNaN(pct) && pct > 0) {
      // 100% 对应根元素 16px 字体大小，125% 对应 20px，150% 对应 24px
      // 整个系统使用 rem 单位的文本和间距会自动无损等比例缩放
      const baseFontSize = 16;
      const targetSize = (baseFontSize * pct) / 100;
      root.style.fontSize = `${targetSize}px`;
    }
  }
}
