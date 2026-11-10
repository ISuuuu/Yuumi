//! Windows 平台级深度接入（Win32 API、命名互斥量单实例、透明置顶与无边框控制）

use crate::MAIN_WINDOW_TITLE;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM, WPARAM,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetActiveWindow, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GetForegroundWindow, GetWindowLongW, IsIconic, IsWindowVisible,
    PostMessageW, SetForegroundWindow, SetWindowLongW, SetWindowPos, ShowWindowAsync, GWL_EXSTYLE,
    HTCAPTION, HWND_NOTOPMOST, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, SW_MINIMIZE, SW_RESTORE, SW_SHOW, WM_NCLBUTTONDOWN,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
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

/// 判断窗口是否为工具窗口（悬浮小窗会带 WS_EX_TOOLWINDOW，主窗口不会）
fn is_tool_window(hwnd: HWND) -> bool {
    unsafe {
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        ex_style & (WS_EX_TOOLWINDOW.0 as i32) != 0
    }
}

/// 根据窗口类名或标题查找主窗口 HWND。
///
/// 会跳过带 WS_EX_TOOLWINDOW 的窗口（大乱斗板凳席悬浮窗等），避免与其混淆。
pub fn find_main_window_hwnd() -> Option<HWND> {
    unsafe {
        // 优先通过 GPUI 窗口类名逐个枚举，取第一个非工具窗口
        let class_name = HSTRING::from("Zed::Window");
        let null_hwnd = HWND(std::ptr::null_mut());
        let mut previous = null_hwnd;
        loop {
            let found = match FindWindowExW(null_hwnd, previous, &class_name, PCWSTR::null()) {
                Ok(hwnd) if !hwnd.0.is_null() => hwnd,
                _ => break,
            };
            if !is_tool_window(found) {
                return Some(found);
            }
            previous = found;
        }

        // 兜底通过窗口标题查找
        let wide_title = HSTRING::from(MAIN_WINDOW_TITLE);
        if let Ok(hwnd) = FindWindowW(None, &wide_title) {
            if !hwnd.0.is_null() {
                return Some(hwnd);
            }
        }
        None
    }
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

/// 设置窗口置顶 (HWND_TOPMOST)
pub fn set_window_topmost(hwnd: HWND, topmost: bool) {
    unsafe {
        let insert_after = if topmost {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        let _ = SetWindowPos(
            hwnd,
            insert_after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
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

/// 在 Windows 11 上启用 Mica 亚克力材质极光半透明
pub fn apply_mica_or_acrylic(hwnd: HWND) {
    unsafe {
        use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE};
        // DWMSBT_ACRYLIC = 3, DWMSBT_MAINWINDOW = 2 (Mica)
        let backdrop_type: u32 = 2; // Mica
        let res = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &backdrop_type as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
        log::info!("[Platform] DwmSetWindowAttribute Mica 返回: {:?}", res);
    }
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
