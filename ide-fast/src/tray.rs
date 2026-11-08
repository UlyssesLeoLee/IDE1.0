//! 系统托盘图标 (Windows 通知区域 / 任务栏右下角时钟旁边)。
//!
//! # 设计约束
//! - `#![forbid(unsafe_code)]` 全项目生效; `tray-icon` crate 内部有 unsafe，由它自己承担。
//! - 托盘菜单项通过 `tray_icon::menu` 构造，不依赖 Win32 MessageBox / SendMessage 等原始 API。
//! - `TrayEvent` 通过 `tray_icon::TrayIconEvent::receiver()` 的无锁 channel 轮询，
//!   在 egui 帧循环里调用 `poll()` 处理，不新建线程。
//! - 托盘仅在 Windows 平台编译；Linux / macOS 平台留空存根，编译不报错。
//!
//! # 菜单项
//! | 动作 | 效果 |
//! |---|---|
//! | 显示 / 隐藏 IDE | 恢复最小化或隐藏的窗口；已可见则最小化 |
//! | 新建文件 | 给主进程发 open-new 命令（同 Ctrl+N）|
//! | 分隔线 | — |
//! | 退出 IDE | 触发 App 的正常退出流程 |

#[cfg(windows)]
mod imp {
    use tray_icon::{
        menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
        TrayIcon, TrayIconBuilder, TrayIconEvent,
    };

    use crate::appicon;

    /// 托盘状态机，调用方持有生命周期
    pub struct Tray {
        /// 必须持有，drop 即消失
        _icon: TrayIcon,
        mi_show: MenuItem,
        mi_new: MenuItem,
        mi_quit: MenuItem,
    }

    impl Tray {
        pub fn build() -> Option<Self> {
            let icon_data = appicon::window_icon()?;
            // tray-icon 需要 Icon { rgba, width, height }
            let ti_icon = tray_icon::Icon::from_rgba(
                icon_data.rgba.clone(),
                icon_data.width,
                icon_data.height,
            )
            .ok()?;

            let mi_show = MenuItem::new("显示 / 隐藏 IDE", true, None);
            let mi_new = MenuItem::new("新建文件", true, None);
            let mi_quit = MenuItem::new("退出 IDE", true, None);
            let sep = PredefinedMenuItem::separator();

            let menu = Menu::new();
            let _ = menu.append(&mi_show);
            let _ = menu.append(&mi_new);
            let _ = menu.append(&sep);
            let _ = menu.append(&mi_quit);

            let icon = TrayIconBuilder::new()
                .with_icon(ti_icon)
                .with_tooltip("IDE1.0 Fast · 美漫极速编辑器 (Sakura 🌸)")
                .with_menu(Box::new(menu))
                .build()
                .ok()?;

            Some(Self {
                _icon: icon,
                mi_show,
                mi_new,
                mi_quit,
            })
        }

        /// 在 egui 帧回调里调用；返回本帧产生的事件列表
        pub fn poll(&self) -> Vec<TrayEvent> {
            let mut out = vec![];
            // 托盘图标点击（左键单击 / 双击）→ 显示窗口
            while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
                match ev {
                    TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. } => {
                        out.push(TrayEvent::ShowHide);
                    }
                    _ => {}
                }
            }
            // 菜单项点击
            while let Ok(ev) = MenuEvent::receiver().try_recv() {
                if ev.id == self.mi_show.id() {
                    out.push(TrayEvent::ShowHide);
                } else if ev.id == self.mi_new.id() {
                    out.push(TrayEvent::NewFile);
                } else if ev.id == self.mi_quit.id() {
                    out.push(TrayEvent::Quit);
                }
            }
            out
        }
    }

    /// 托盘动作，由 `App::handle_tray` 消费
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TrayEvent {
        /// 显示/恢复或最小化窗口
        ShowHide,
        /// 新建空白文件（等同 Ctrl+N）
        NewFile,
        /// 请求退出
        Quit,
    }
}

// ─── 平台无关公共接口 ───────────────────────────────────────────────────────

#[cfg(windows)]
pub use imp::{Tray, TrayEvent};

/// 在非 Windows 平台编译时提供空存根，避免到处写 cfg。
#[cfg(not(windows))]
pub struct Tray;

#[cfg(not(windows))]
impl Tray {
    pub fn build() -> Option<Self> {
        None
    }
    pub fn poll(&self) -> Vec<TrayEvent> {
        vec![]
    }
}

#[cfg(not(windows))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    ShowHide,
    NewFile,
    Quit,
}
