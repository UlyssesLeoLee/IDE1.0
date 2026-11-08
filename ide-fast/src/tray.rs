//! 系统托盘图标 (Windows 通知区域 / 任务栏右下角时钟旁边)。
//!
//! # 设计约束
//! - `#![forbid(unsafe_code)]` 全项目生效; `tray-icon` crate 内部有 unsafe，由它自己承担。
//! - 托盘菜单通过 `tray_icon::menu` 构造，不依赖 Win32 原生原始 API。
//! - `TrayEvent` 通过 `tray_icon::TrayIconEvent::receiver()` 的无锁 channel 轮询，
//!   在 egui 帧循环里调用 `poll()` 处理，不新建线程。
//! - 托盘仅在 Windows 平台编译；Linux / macOS 平台留空存根，编译不报错。
//!
//! # 菜单项
//! | 动作 | 效果 |
//! |---|---|
//! | 显示 / 隐藏 IDE | 恢复最小化或隐藏的窗口；已可见且聚焦则最小化 |
//! | 新建空白文档 | 给主进程发 open-new 命令（同 Ctrl+N）|
//! | 窗口置顶 | 切换是否总在最前 |
//! | 常驻后台 (关闭窗口最小化) | 勾选时关闭窗口仅隐藏至托盘；不勾选则直接退出 |
//! | 退出托盘常驻 | 移除托盘图标，恢复为普通窗口生命周期 |
//! | 分隔线 | — |
//! | 退出 IDE | 触发 App 的正常退出流程 |

#[cfg(windows)]
mod imp {
    use tray_icon::{
        menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
        TrayIcon, TrayIconBuilder, TrayIconEvent,
    };

    use crate::appicon;

    /// 托盘状态机，调用方持有生命周期
    pub struct Tray {
        /// 必须持有，drop 即消失
        _icon: TrayIcon,
        mi_show: MenuItem,
        mi_new: MenuItem,
        mi_open: MenuItem,
        mi_collab: MenuItem,
        mi_diff: MenuItem,
        mi_topmost: CheckMenuItem,
        mi_resident: CheckMenuItem,
        mi_disable: MenuItem,
        mi_quit: MenuItem,
    }

    impl Tray {
        pub fn build(resident: bool, topmost: bool) -> Option<Self> {
            let icon_data = appicon::window_icon()?;
            let ti_icon = tray_icon::Icon::from_rgba(
                icon_data.rgba.clone(),
                icon_data.width,
                icon_data.height,
            )
            .ok()?;

            let mi_show = MenuItem::new("显示 / 隐藏 IDE", true, None);
            let mi_new = MenuItem::new("新建空白文档 (Ctrl+N)", true, None);
            let mi_open = MenuItem::new("打开文件… (Ctrl+O)", true, None);
            let mi_collab = MenuItem::new("🤖 多 Agent 协同工作台…", true, None);
            let mi_diff = MenuItem::new("WinMerge 差异对比…", true, None);
            let mi_topmost = CheckMenuItem::new("窗口置顶 (Always on Top)", true, topmost, None);
            let mi_resident = CheckMenuItem::new("常驻后台 (关闭窗口时隐藏到托盘)", true, resident, None);
            let mi_disable = MenuItem::new("退出托盘常驻", true, None);
            let sep1 = PredefinedMenuItem::separator();
            let sep2 = PredefinedMenuItem::separator();
            let sep3 = PredefinedMenuItem::separator();
            let mi_quit = MenuItem::new("退出 IDE", true, None);

            let menu = Menu::new();
            let _ = menu.append(&mi_show);
            let _ = menu.append(&mi_new);
            let _ = menu.append(&mi_open);
            let _ = menu.append(&sep1);
            let _ = menu.append(&mi_collab);
            let _ = menu.append(&mi_diff);
            let _ = menu.append(&sep2);
            let _ = menu.append(&mi_topmost);
            let _ = menu.append(&mi_resident);
            let _ = menu.append(&mi_disable);
            let _ = menu.append(&sep3);
            let _ = menu.append(&mi_quit);

            let icon = TrayIconBuilder::new()
                .with_icon(ti_icon)
                .with_tooltip("IDE1.0 Fast · 极速文本编辑器")
                .with_menu(Box::new(menu))
                .build()
                .ok()?;

            Some(Self {
                _icon: icon,
                mi_show,
                mi_new,
                mi_open,
                mi_collab,
                mi_diff,
                mi_topmost,
                mi_resident,
                mi_disable,
                mi_quit,
            })
        }

        pub fn set_resident_checked(&self, checked: bool) {
            self.mi_resident.set_checked(checked);
        }

        pub fn set_topmost_checked(&self, checked: bool) {
            self.mi_topmost.set_checked(checked);
        }

        /// 在 egui 帧回调里调用；返回本帧产生的事件列表
        pub fn poll(&self) -> Vec<TrayEvent> {
            let mut out = vec![];
            // 托盘图标点击（左键单击 / 双击）→ 切换显示/隐藏
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
                } else if ev.id == self.mi_open.id() {
                    out.push(TrayEvent::OpenFile);
                } else if ev.id == self.mi_collab.id() {
                    out.push(TrayEvent::OpenCollab);
                } else if ev.id == self.mi_diff.id() {
                    out.push(TrayEvent::OpenDiff);
                } else if ev.id == self.mi_topmost.id() {
                    out.push(TrayEvent::ToggleTopmost);
                } else if ev.id == self.mi_resident.id() {
                    out.push(TrayEvent::ToggleResident);
                } else if ev.id == self.mi_disable.id() {
                    out.push(TrayEvent::DisableResident);
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
        /// 打开文件对话框（等同 Ctrl+O）
        OpenFile,
        /// 打开多 Agent 协同工作台
        OpenCollab,
        /// 打开 WinMerge 差异对比
        OpenDiff,
        /// 切换窗口置顶
        ToggleTopmost,
        /// 切换是否常驻系统托盘
        ToggleResident,
        /// 禁用/退出托盘常驻（移除托盘图标）
        DisableResident,
        /// 请求完全退出
        Quit,
    }
}

// ─── 平台无关公共接口 ───────────────────────────────────────────────

#[cfg(windows)]
pub use imp::{Tray, TrayEvent};

/// 在非 Windows 平台编译时提供空存根，避免到处写 cfg。
#[cfg(not(windows))]
pub struct Tray;

#[cfg(not(windows))]
impl Tray {
    pub fn build(_resident: bool, _topmost: bool) -> Option<Self> {
        None
    }
    pub fn poll(&self) -> Vec<TrayEvent> {
        vec![]
    }
    pub fn set_resident_checked(&self, _checked: bool) {}
    pub fn set_topmost_checked(&self, _checked: bool) {}
}

#[cfg(not(windows))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    ShowHide,
    NewFile,
    OpenFile,
    OpenCollab,
    OpenDiff,
    ToggleTopmost,
    ToggleResident,
    DisableResident,
    Quit,
}
