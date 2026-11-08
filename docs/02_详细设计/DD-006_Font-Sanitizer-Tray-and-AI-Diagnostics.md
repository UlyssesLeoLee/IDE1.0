# IDE1.0 字体防畸变净化器、系统托盘常驻与 AI 额度诊断详细设计书

**文档编号**: DD-FONT-TRAY-AI-2026-V1.0  
**制定组织**: IDE1.0 客户端核心架构组  
**基准版本**: v1.0  
**关联设计**:
- [BD-005_Font-Legibility-Tray-Residency-AI-Diagnostics.md](../01_基本设计/BD-005_Font-Legibility-Tray-Residency-AI-Diagnostics.md)

---

## 1. 字体防畸变净化器 (`sanitize_font_family`)

```rust
/// 确保字体适用于正文与代码阅读，若传入非法或艺术扭曲字体则安全回退至主题默认等宽编程字体
pub fn sanitize_font_family(font: &str, theme_id: &str) -> String {
    let clean = font.trim();
    if clean.is_empty() {
        String::new()
    } else if is_illegible_display_font(clean) {
        theme_default_font(theme_id).unwrap_or("CascadiaCode.ttf").to_string()
    } else {
        clean.to_string()
    }
}

/// 底层文件读取安全拦截
pub fn load_ext(editor_file: &str, cjk_file: &str, lang: &str, need_bold: bool) -> Loaded {
    let dir = fonts_dir();
    let safe_editor = std::path::Path::new(editor_file)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let ed_handle = std::thread::spawn(move || {
        let (mut ed, mut ed_bold) = if !safe_editor.is_empty()
            && is_font_file(&safe_editor)
            && !is_illegible_display_font(&safe_editor)
        {
            let b1 = std::fs::read(dir_clone.join(&safe_editor)).ok();
            let b2 = if need_bold {
                find_bold_file(&dir_clone, &safe_editor).and_then(|f| std::fs::read(dir_clone.join(f)).ok())
            } else { None };
            (b1, b2)
        } else {
            (None, None)
        };
        // 若被拦截或未找到，强保底到现代编程字体
        if ed.is_none() {
            for fallback in ["CascadiaCode.ttf", "CascadiaMono.ttf", "consola.ttf"] {
                if let Ok(b) = std::fs::read(dir_clone.join(fallback)) {
                    ed = Some(b);
                    break;
                }
            }
        }
        (ed, ed_bold)
    });
    // ...
}
```

---

## 2. 增强型系统托盘事件处理 (`TrayEvent`)

```rust
pub fn handle_tray_event(&mut self, ctx: &egui::Context, ev: crate::tray::TrayEvent) {
    match ev {
        TrayEvent::ShowHide => {
            let is_min = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
            let is_foc = ctx.input(|i| i.viewport().focused.unwrap_or(false));
            let is_vis = ctx.input(|i| i.viewport().visible().unwrap_or(true));
            if is_vis && !is_min && is_foc {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
        TrayEvent::NewFile => {
            self.new_doc();
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        TrayEvent::OpenFile => {
            self.open_dialog();
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        TrayEvent::OpenCollab => {
            self.collab.open = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        TrayEvent::OpenDiff => {
            self.diff_with_file();
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        TrayEvent::ToggleTopmost => {
            self.always_on_top = !self.always_on_top;
            let level = if self.always_on_top { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal };
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
            if let Some(t) = &self.tray { t.set_topmost_checked(self.always_on_top); }
        }
        TrayEvent::ToggleResident => {
            self.cfg.tray_resident = !self.cfg.tray_resident;
            if let Some(t) = &self.tray { t.set_resident_checked(self.cfg.tray_resident); }
            if self.persist { self.cfg.save(); }
        }
        TrayEvent::DisableResident => {
            self.cfg.tray_resident = false;
            self.tray = None;
            if self.persist { self.cfg.save(); }
        }
        TrayEvent::Quit => {
            self.force_exit = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
```

---

## 3. 响应体深度解析与 HTTP 错误诊断器 (`format_http_error`)

```rust
pub fn format_http_error(
    status: Option<u16>,
    detail: Option<&str>,
    stderr: Option<&str>,
) -> String {
    let clean_detail = detail.map(|s| s.trim()).filter(|s| !s.is_empty()).map(|s| s.to_string());
    let clean_err = stderr.map(|s| s.trim()).filter(|s| !s.is_empty()).map(|s| s.to_string());

    // 1. 深度额度/欠费识别
    let is_quota_issue = {
        let text = format!("{} {}", clean_detail.as_deref().unwrap_or(""), clean_err.as_deref().unwrap_or("")).to_ascii_lowercase();
        text.contains("insufficient_quota") || text.contains("quota_exceeded") || text.contains("out of credits")
            || text.contains("balance insufficient") || text.contains("billing") || text.contains("额度不足")
            || text.contains("余额不足") || text.contains("欠费") || status == Some(402)
    };
    if is_quota_issue {
        let msg = clean_detail.or(clean_err).unwrap_or_else(|| "账户额度已耗尽或余额不足".to_string());
        return format!("【API 额度耗尽 / 账户欠费】服务商返回: {msg}。请前往 API 提供商后台检查账单与额度充值，或在设置中更换有效的 API Key。");
    }

    // 2. 状态码 0 / 网络连通性深度诊断
    if status == Some(0) || status.is_none() {
        let err_hint = if let Some(e) = clean_err {
            let el = e.to_ascii_lowercase();
            if el.contains("could not resolve host") { format!("{e} (域名无法解析，请检查 Base URL 是否正确或网络 DNS)") }
            else if el.contains("connection refused") { format!("{e} (服务连接被拒绝，目标地址可能未启动或端口不通)") }
            else if el.contains("timed out") { format!("{e} (网络连接超时，请检查网络通畅度或系统代理)") }
            else if el.contains("ssl") || el.contains("certificate") { format!("{e} (SSL 证书握手失败，请检查 HTTPS 证书或代理配置)") }
            else { e }
        } else if let Some(d) = clean_detail { d }
        else { "网络连接失败或超时，未能与 AI 服务器建立连接".to_string() };
        return format!("【网络连接失败】{err_hint}。请检查 Base URL 设置、网络连通性或代理配置。");
    }

    // 3. 其他常规 HTTP 状态码规整
    let st = status.unwrap_or(0);
    match st {
        401 | 403 => format!("【API 鉴权失败 (HTTP {st})】服务商返回: {}。请检查输入的 API Key 是否有效。", clean_detail.or(clean_err).unwrap_or_default()),
        404 => format!("【模型或接口不存在 (HTTP 404)】服务商返回: {}。请检查配置中的 Model 名称与 Base URL。", clean_detail.or(clean_err).unwrap_or_default()),
        429 => format!("【API 请求过于频繁 (HTTP 429)】服务商返回: {}。请稍等片刻后重试。", clean_detail.or(clean_err).unwrap_or_default()),
        500..=599 => format!("【AI 服务端内部异常 (HTTP {st})】服务商返回: {}。服务商暂时不可用，请稍后重试。", clean_detail.or(clean_err).unwrap_or_default()),
        _ => clean_detail.or(clean_err).map(|m| format!("HTTP {st}: {m}")).unwrap_or_else(|| format!("HTTP {st}")),
    }
}
```
