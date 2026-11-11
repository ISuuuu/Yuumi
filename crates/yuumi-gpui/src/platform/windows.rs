//! Windows 平台级深度接入（Win32 API、命名互斥量单实例、透明置顶与无边框控制）

use crate::MAIN_WINDOW_TITLE;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM, WPARAM,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetActiveWindow, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetForegroundWindow, GetWindowLongW, GetWindowThreadProcessId, IsIconic, IsWindow,
    IsWindowVisible, PostMessageW, SetForegroundWindow, SetWindowLongW, SetWindowPos,
    ShowWindowAsync, GWL_EXSTYLE, HTCAPTION, HWND_NOTOPMOST, HWND_TOPMOST, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_MINIMIZE, SW_RESTORE, SW_SHOW,
    WM_NCLBUTTONDOWN, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

/// 命名互斥量守卫，负责释放句柄
pub struct SingleInstanceGuard {
    handle: HANDLE,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

/// 命名互斥量单实例错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SingleInstanceError {
    AlreadyRunning,
    CreationFailed(String),
}

impl std::fmt::Display for SingleInstanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SingleInstanceError::AlreadyRunning => write!(f, "应用已有实例正在运行"),
            SingleInstanceError::CreationFailed(e) => write!(f, "创建互斥量失败: {}", e),
        }
    }
}

impl std::error::Error for SingleInstanceError {}

/// 尝试获取全局命名互斥量。若已存在，返回 Err(SingleInstanceError::AlreadyRunning)
pub fn acquire_single_instance(name: &str) -> Result<SingleInstanceGuard, SingleInstanceError> {
    unsafe {
        let wide_name = HSTRING::from(name);
        let handle = match CreateMutexW(None, true, &wide_name) {
            Ok(h) => h,
            Err(e) => return Err(SingleInstanceError::CreationFailed(e.to_string())),
        };

        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return Err(SingleInstanceError::AlreadyRunning);
        }

        Ok(SingleInstanceGuard { handle })
    }
}

/// 主窗口 HWND 缓存（P0-5）：`initialize_window_appearance` 创建主窗时登记，
/// 查找优先命中缓存，避免按类名枚举误命中 Zed 编辑器 / OP.GG 独立窗口等其他同类名窗口。
static MAIN_WINDOW_HWND: AtomicIsize = AtomicIsize::new(0);

/// 登记主窗口 HWND（仅主窗口初始化外观时调用）
pub fn cache_main_window_hwnd(hwnd: HWND) {
    MAIN_WINDOW_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
}

/// 校验 HWND 仍属于当前进程的有效窗口
fn hwnd_belongs_to_current_process(hwnd: HWND) -> bool {
    unsafe {
        if !IsWindow(hwnd).as_bool() {
            return false;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        pid == std::process::id()
    }
}

/// 根据窗口标题查找主窗口 HWND。
///
/// P0-5 精确化：优先返回启动时缓存的主窗句柄；缓存失效时兜底按标题查找，
/// 且必须校验 `GetWindowThreadProcessId == std::process::id()`（防止命中
/// 其他进程恰好同名标题的窗口，如正在运行的 Zed 编辑器）。
pub fn find_main_window_hwnd() -> Option<HWND> {
    // 1. 优先使用启动时登记的缓存句柄
    let cached_raw = MAIN_WINDOW_HWND.load(Ordering::SeqCst);
    if cached_raw != 0 {
        let hwnd = HWND(cached_raw as *mut _);
        if hwnd_belongs_to_current_process(hwnd) {
            return Some(hwnd);
        }
        // 缓存失效（窗口销毁/句柄复用）：清空后走兜底
        MAIN_WINDOW_HWND.store(0, Ordering::SeqCst);
    }

    // 2. 兜底：按标题查找并校验进程归属
    unsafe {
        let wide_title = HSTRING::from(MAIN_WINDOW_TITLE);
        if let Ok(hwnd) = FindWindowW(None, &wide_title) {
            if !hwnd.0.is_null() && hwnd_belongs_to_current_process(hwnd) {
                return Some(hwnd);
            }
        }
    }
    None
}

/// 根据窗口标题查找 HWND
pub fn find_window_by_title(title: &str) -> Option<HWND> {
    unsafe {
        let wide_title = HSTRING::from(title);
        match FindWindowW(None, &wide_title) {
            Ok(hwnd) if !hwnd.0.is_null() => Some(hwnd),
            _ => None,
        }
    }
}

/// 激活并将窗口置于最前
pub fn restore_and_foreground(hwnd: HWND) {
    unsafe {
        let _ = ShowWindowAsync(hwnd, SW_RESTORE);
        let _ = ShowWindowAsync(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetActiveWindow(hwnd);
        let _ = SetFocus(hwnd);
    }
}

/// 隐藏窗口（用于关闭到系统托盘）
pub fn hide_window(hwnd: HWND) {
    unsafe {
        let _ = ShowWindowAsync(hwnd, SW_HIDE);
    }
}

/// 检查窗口是否处于可见状态
pub fn is_window_visible(hwnd: HWND) -> bool {
    unsafe { IsWindowVisible(hwnd).as_bool() }
}

/// 检查窗口是否处于最小化状态
pub fn is_window_minimized(hwnd: HWND) -> bool {
    unsafe { IsIconic(hwnd).as_bool() }
}

/// 检查指定窗口是否为当前系统前台激活窗口
pub fn is_foreground_window(hwnd: HWND) -> bool {
    unsafe { GetForegroundWindow() == hwnd }
}

/// 最小化窗口
pub fn minimize_window(hwnd: HWND) {
    unsafe {
        let _ = ShowWindowAsync(hwnd, SW_MINIMIZE);
    }
}

/// 以 Win32 原生方式启动窗口拖拽（等价于按住系统标题栏拖动）
///
/// 使用 PostMessageW 异步投递到消息队列，避免在 GPUI 事件回调（如 on_mouse_down）
/// 同步持有 AppCell 借用时同步调用 SendMessageW 进入系统模态拖拽循环，
/// 从而彻底防止借用冲突（"RefCell already borrowed"）。
pub fn begin_window_drag(hwnd: HWND) {
    unsafe {
        let _ = ReleaseCapture();
        let _ = PostMessageW(
            hwnd,
            WM_NCLBUTTONDOWN,
            WPARAM(HTCAPTION as usize),
            LPARAM(0),
        );
    }
}

pub fn native_window_handle(window: &gpui::Window) -> Result<HWND, String> {
    match HasWindowHandle::window_handle(window)
        .map_err(|e| e.to_string())?
        .as_raw()
    {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut _)),
        _ => Err("无法获取 Windows 窗口句柄".into()),
    }
}

/// 设置窗口置顶，保留当前的可见性和焦点。
pub fn set_window_topmost(hwnd: HWND, topmost: bool) {
    unsafe {
        let insert_after = if topmost {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        if let Err(error) = SetWindowPos(
            hwnd,
            insert_after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        ) {
            log::warn!("[Platform] 设置窗口置顶失败: {error}");
        }
    }
}

/// 将窗口设置为工具窗口 (ToolWindow: 隐藏任务栏图标)
pub fn set_window_toolwindow(hwnd: HWND) {
    unsafe {
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        let new_style = ex_style | (WS_EX_TOOLWINDOW.0 as i32);
        let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, new_style);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

/// 设置窗口鼠标穿透 (WS_EX_TRANSPARENT | WS_EX_LAYERED)
pub fn set_window_clickthrough(hwnd: HWND, enable: bool) {
    unsafe {
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        let new_style = if enable {
            ex_style | (WS_EX_TRANSPARENT.0 as i32) | (WS_EX_LAYERED.0 as i32)
        } else {
            ex_style & !(WS_EX_TRANSPARENT.0 as i32)
        };
        let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, new_style);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

/// 在 Windows 11 上启用或清除 Mica 背景。
pub fn set_mica(hwnd: HWND, enabled: bool) -> Result<(), String> {
    unsafe {
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMSBT_MAINWINDOW, DWMSBT_NONE, DWMWA_SYSTEMBACKDROP_TYPE,
        };
        let backdrop = if enabled {
            DWMSBT_MAINWINDOW
        } else {
            DWMSBT_NONE
        };
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            std::ptr::from_ref(&backdrop).cast(),
            std::mem::size_of_val(&backdrop) as u32,
        )
        .map_err(|e| e.to_string())
    }
}

/// 读取 Windows「应用模式」系统深色偏好（AppsUseLightTheme=0 表示深色）。
/// 读取失败时按浅色处理。
pub fn system_prefers_dark() -> bool {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let subkey = widen("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let value = widen("AppsUseLightTheme");

    let mut data: u32 = 0;
    let mut data_size = std::mem::size_of::<u32>() as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(std::ptr::from_mut(&mut data).cast()),
            Some(std::ptr::from_mut(&mut data_size)),
        )
    };
    result.is_ok() && data == 0
}

/// UTF-16 NUL 结尾字符串
fn widen(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 在 Windows 11 上启用原生圆角
pub fn set_window_rounded_corners(hwnd: HWND) {
    unsafe {
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE,
        };
        // DWMWCP_ROUND = 2
        let preference: u32 = 2;
        let res = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
        log::info!(
            "[Platform] DwmSetWindowAttribute RoundedCorners 返回: {:?}",
            res
        );
    }
}

// 重新导出系统工具服务中的常用方法以方便平台级访问
pub use crate::services::system_tools::{
    clear_game_cache, detect_lol_path, fix_client_window, open_folder, open_log_folder,
    select_folder,
};

/// CF_UNICODETEXT 的标准剪贴板格式编号（Win32 文档常量，避免引入 Ole 特性依赖）
const CF_UNICODETEXT_FORMAT: u32 = 13;

/// 将文本写入系统剪贴板并返回真实写入结果。
///
/// gpui 的 `write_to_clipboard` 无返回值，无法感知被占用/失败；
/// 复制类反馈（阵容码、对局 ID）以本函数的实际写入结果为准（对照设计文档阶段 4）。
pub fn try_write_clipboard(text: &str) -> Result<(), String> {
    use windows::Win32::Foundation::GlobalFree;
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    unsafe {
        OpenClipboard(None).map_err(|e| format!("打开剪贴板失败: {}", e))?;
        // CloseClipboard 必须无条件执行：剪贴板被其他进程打开时立即释放，避免长时间锁死
        let result = (|| -> Result<(), String> {
            EmptyClipboard().map_err(|e| format!("清空剪贴板失败: {}", e))?;
            // CF_UNICODETEXT：带 NUL 终止的 UTF-16 编码
            let mut utf16: Vec<u16> = text.encode_utf16().collect();
            utf16.push(0);
            let byte_len = utf16.len() * std::mem::size_of::<u16>();
            let handle: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, byte_len)
                .map_err(|e| format!("分配剪贴板内存失败: {}", e))?;
            let dst = GlobalLock(handle);
            if dst.is_null() {
                let _ = GlobalUnlock(handle);
                let _ = GlobalFree(handle);
                return Err("锁定剪贴板内存失败".into());
            }
            std::ptr::copy_nonoverlapping(utf16.as_ptr(), dst as *mut u16, utf16.len());
            let _ = GlobalUnlock(handle);
            // 写入成功后内存所有权移交系统；失败则自行释放
            match SetClipboardData(
                CF_UNICODETEXT_FORMAT,
                windows::Win32::Foundation::HANDLE(handle.0),
            ) {
                Ok(_) => Ok(()),
                Err(e) => {
                    let _ = GlobalFree(handle);
                    Err(format!("写入剪贴板数据失败: {}", e))
                }
            }
        })();
        let _ = CloseClipboard();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_instance_named_mutex() {
        let mutex_name = "Global\\Yuumi_Test_SingleInstance_Mutex_Unit";
        let guard1 = acquire_single_instance(mutex_name);
        assert!(guard1.is_ok(), "首次获取互斥量应当成功");

        let guard2 = acquire_single_instance(mutex_name);
        assert!(guard2.is_err(), "重复获取同名互斥量应当返回 Err");

        drop(guard1);

        let guard3 = acquire_single_instance(mutex_name);
        assert!(guard3.is_ok(), "释放前一个守卫后应当能重新获取");
    }
}
