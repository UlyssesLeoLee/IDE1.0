// font_manager.rs — フォント変更 (sakura 12.5)
//
// エディタのフォント family / size / weight を管理.
// 0 依存 (font は 文字列識別子のみ).

use serde::{Deserialize, Serialize};

/// フォントウェイト
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontWeight {
    Normal,
    Bold,
    Light,
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::Normal
    }
}

/// フォント設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    pub family: String,
    pub size_pt: u8,
    pub weight: FontWeight,
    pub line_height: f64, // 1.0 = 等倍
    pub monospace_only: bool,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: "Consolas".to_string(),
            size_pt: 14,
            weight: FontWeight::Normal,
            line_height: 1.4,
            monospace_only: true,
        }
    }
}

impl PartialEq for FontConfig {
    fn eq(&self, other: &Self) -> bool {
        self.family == other.family
            && self.size_pt == other.size_pt
            && self.weight == other.weight
            && (self.line_height - other.line_height).abs() < f64::EPSILON
            && self.monospace_only == other.monospace_only
    }
}

impl FontConfig {
    /// CSS 文字列生成 (フロントエンド用)
    pub fn to_css(&self) -> String {
        let weight = match self.weight {
            FontWeight::Normal => "normal",
            FontWeight::Bold => "bold",
            FontWeight::Light => "300",
        };
        format!(
            "font-family: '{}'; font-size: {}pt; font-weight: {}; line-height: {};",
            self.family, self.size_pt, weight, self.line_height
        )
    }

    /// フォント変更
    pub fn set_family(&mut self, family: impl Into<String>) {
        self.family = family.into();
    }

    pub fn set_size(&mut self, size_pt: u8) {
        self.size_pt = size_pt.clamp(6, 72);
    }
}

/// プリセット
pub fn preset_mono_default() -> FontConfig {
    FontConfig {
        family: "Consolas".to_string(),
        size_pt: 14,
        weight: FontWeight::Normal,
        line_height: 1.4,
        monospace_only: true,
    }
}

pub fn preset_japanese() -> FontConfig {
    FontConfig {
        family: "MS Gothic".to_string(),
        size_pt: 12,
        weight: FontWeight::Normal,
        line_height: 1.5,
        monospace_only: false,
    }
}

pub fn preset_compact() -> FontConfig {
    FontConfig {
        family: "Cascadia Code".to_string(),
        size_pt: 12,
        weight: FontWeight::Light,
        line_height: 1.2,
        monospace_only: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let f = FontConfig::default();
        assert_eq!(f.size_pt, 14);
        assert_eq!(f.family, "Consolas");
    }

    #[test]
    fn test_set_family() {
        let mut f = FontConfig::default();
        f.set_family("JetBrains Mono");
        assert_eq!(f.family, "JetBrains Mono");
    }

    #[test]
    fn test_size_clamp() {
        let mut f = FontConfig::default();
        f.set_size(3);
        assert_eq!(f.size_pt, 6);
        f.set_size(200);
        assert_eq!(f.size_pt, 72);
    }

    #[test]
    fn test_css() {
        let f = FontConfig::default();
        let css = f.to_css();
        assert!(css.contains("Consolas"));
        assert!(css.contains("14pt"));
    }

    #[test]
    fn test_presets() {
        let j = preset_japanese();
        assert!(!j.monospace_only);
        let c = preset_compact();
        assert_eq!(c.size_pt, 12);
    }
}
