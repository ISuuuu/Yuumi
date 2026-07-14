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
  root.style.setProperty(
    "--primary-color-alpha-15",
    `rgba(${r}, ${g}, ${b}, 0.15)`,
  );
  root.style.setProperty(
    "--primary-color-alpha-30",
    `rgba(${r}, ${g}, ${b}, 0.3)`,
  );
  root.style.setProperty(
    "--primary-color-alpha-40",
    `rgba(${r}, ${g}, ${b}, 0.4)`,
  );
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

/**
 * 动态更新对局卡片的 CSS 变量
 * @param winColor 胜利卡片颜色 (8位 hex #aarrggbb 或 6位 hex 或 rgba)
 * @param loseColor 失败卡片颜色 (8位 hex #aarrggbb 或 6位 hex 或 rgba)
 * @param remakeColor 重开卡片颜色 (8位 hex #aarrggbb 或 6位 hex 或 rgba)
 */
export function updateCardColors(
  winColor: string,
  loseColor: string,
  remakeColor: string,
) {
  const root = document.documentElement;

  const DEFAULT_WIN = "#3339b01b";
  const DEFAULT_LOSE = "#33d3190c";
  const DEFAULT_REMAKE = "#33a2a2a2";

  const setCardVars = (prefix: string, color: string, defaultHex: string) => {
    // 如果没有颜色配置，或颜色等于默认配置（不区分大小写），则清除 CSS 自定义属性覆盖，让其回退到样式表的亮暗色自动适应
    if (!color || color.toLowerCase() === defaultHex.toLowerCase()) {
      root.style.removeProperty(`--${prefix}-color`);
      root.style.removeProperty(`--${prefix}-bg`);
      root.style.removeProperty(`--${prefix}-border`);
      root.style.removeProperty(`--${prefix}-glow`);
      return;
    }

    let r = 0,
      g = 0,
      b = 0,
      alpha = 0.1;
    if (color.startsWith("#")) {
      if (color.length === 9) {
        alpha = parseInt(color.slice(1, 3), 16) / 255;
        r = parseInt(color.slice(3, 5), 16);
        g = parseInt(color.slice(5, 7), 16);
        b = parseInt(color.slice(7, 9), 16);
      } else if (color.length === 7) {
        r = parseInt(color.slice(1, 3), 16);
        g = parseInt(color.slice(3, 5), 16);
        b = parseInt(color.slice(5, 7), 16);
        alpha = 0.1; // 默认大约 10% 的透明度
      }
    } else if (color.startsWith("rgba") || color.startsWith("rgb")) {
      const match = color.match(/\d+(\.\d+)?/g);
      if (match) {
        r = parseInt(match[0], 10);
        g = parseInt(match[1], 10);
        b = parseInt(match[2], 10);
        alpha = match[3] !== undefined ? parseFloat(match[3]) : 0.1;
      }
    }

    // 如果解析失败，清除覆盖
    if (isNaN(r) || isNaN(g) || isNaN(b)) {
      root.style.removeProperty(`--${prefix}-color`);
      root.style.removeProperty(`--${prefix}-bg`);
      root.style.removeProperty(`--${prefix}-bg-hover`);
      root.style.removeProperty(`--${prefix}-border`);
      root.style.removeProperty(`--${prefix}-glow`);
      return;
    }

    root.style.setProperty(`--${prefix}-color`, `rgba(${r}, ${g}, ${b}, 1)`);
    root.style.setProperty(
      `--${prefix}-bg`,
      `rgba(${r}, ${g}, ${b}, ${alpha.toFixed(2)})`,
    );
    root.style.setProperty(
      `--${prefix}-bg-hover`,
      `rgba(${r}, ${g}, ${b}, ${Math.min(0.9, alpha * 1.5).toFixed(2)})`,
    );
    root.style.setProperty(
      `--${prefix}-border`,
      `rgba(${r}, ${g}, ${b}, ${(alpha * 2.5).toFixed(2)})`,
    );
    root.style.setProperty(
      `--${prefix}-glow`,
      `rgba(${r}, ${g}, ${b}, ${(alpha * 0.75).toFixed(2)})`,
    );
  };

  setCardVars("win", winColor, DEFAULT_WIN);
  setCardVars("loss", loseColor, DEFAULT_LOSE);
  setCardVars("remake", remakeColor, DEFAULT_REMAKE);
}
