// syntax_styling.rs — 文字色/背景/太字/下線 シンタックス装飾 (sakura 6.3)
//
// トークンに 色/スタイル属性を付与. フロントエンド (editor.html) がレンダリング.

use crate::type_config::TokenKind;

/// 文字色 (256 色パレット)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub const BLACK: Color = Color(0, 0, 0);
    pub const WHITE: Color = Color(255, 255, 255);
    pub const RED: Color = Color(255, 0, 0);
    pub const GREEN: Color = Color(0, 200, 0);
    pub const BLUE: Color = Color(0, 0, 255);
    pub const GRAY: Color = Color(128, 128, 128);
    pub const YELLOW: Color = Color(200, 200, 0);
    pub const CYAN: Color = Color(0, 180, 180);

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// スタイル属性
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

impl Style {
    pub fn bold() -> Self {
        Self { bold: true, ..Default::default() }
    }
    pub fn italic() -> Self {
        Self { italic: true, ..Default::default() }
    }
    pub fn underline() -> Self {
        Self { underline: true, ..Default::default() }
    }
}

/// トークン表示スタイル (色 + スタイル)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TokenStyle {
    pub fg: Color,
    pub bg: Option<Color>,
    pub style: Style,
}

impl TokenStyle {
    pub fn new(fg: Color) -> Self {
        Self { fg, bg: None, style: Style::default() }
    }

    pub fn with_bg(mut self, bg: Color) -> Self {
        self.bg = Some(bg);
        self
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

/// デフォルトテーマ (Dark)
pub fn default_dark_theme() -> std::collections::HashMap<TokenKind, TokenStyle> {
    let mut m = std::collections::HashMap::new();
    m.insert(TokenKind::Keyword, TokenStyle::new(Color::CYAN).with_style(Style::bold()));
    m.insert(TokenKind::String, TokenStyle::new(Color::GREEN));
    m.insert(TokenKind::Number, TokenStyle::new(Color::YELLOW));
    m.insert(TokenKind::Comment, TokenStyle::new(Color::GRAY).with_style(Style::italic()));
    m.insert(TokenKind::Operator, TokenStyle::new(Color::WHITE));
    m.insert(TokenKind::Punct, TokenStyle::new(Color::WHITE));
    m.insert(TokenKind::Function, TokenStyle::new(Color::BLUE));
    m.insert(TokenKind::Type, TokenStyle::new(Color::CYAN));
    m.insert(TokenKind::Variable, TokenStyle::new(Color::WHITE));
    m.insert(TokenKind::Plain, TokenStyle::new(Color::WHITE));
    m
}

/// デフォルトテーマ (Light)
pub fn default_light_theme() -> std::collections::HashMap<TokenKind, TokenStyle> {
    let mut m = std::collections::HashMap::new();
    m.insert(TokenKind::Keyword, TokenStyle::new(Color(0, 0, 180)).with_style(Style::bold()));
    m.insert(TokenKind::String, TokenStyle::new(Color(180, 0, 0)));
    m.insert(TokenKind::Number, TokenStyle::new(Color(0, 130, 0)));
    m.insert(TokenKind::Comment, TokenStyle::new(Color(128, 128, 128)).with_style(Style::italic()));
    m.insert(TokenKind::Operator, TokenStyle::new(Color::BLACK));
    m.insert(TokenKind::Punct, TokenStyle::new(Color::BLACK));
    m.insert(TokenKind::Function, TokenStyle::new(Color(0, 100, 180)));
    m.insert(TokenKind::Type, TokenStyle::new(Color(100, 0, 180)));
    m.insert(TokenKind::Variable, TokenStyle::new(Color::BLACK));
    m.insert(TokenKind::Plain, TokenStyle::new(Color::BLACK));
    m
}

/// テーマ適用: トークン種別 → スタイル
pub struct Theme {
    pub name: String,
    pub styles: std::collections::HashMap<TokenKind, TokenStyle>,
}

impl Theme {
    pub fn dark() -> Self {
        Self { name: "dark".to_string(), styles: default_dark_theme() }
    }

    pub fn light() -> Self {
        Self { name: "light".to_string(), styles: default_light_theme() }
    }

    pub fn get(&self, kind: TokenKind) -> TokenStyle {
        self.styles.get(&kind).copied().unwrap_or(TokenStyle::new(Color::WHITE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_hex() {
        assert_eq!(Color(255, 0, 0).to_hex(), "#ff0000");
        assert_eq!(Color::WHITE.to_hex(), "#ffffff");
    }

    #[test]
    fn test_dark_theme_keywords_bold() {
        let t = Theme::dark();
        let kw = t.get(TokenKind::Keyword);
        assert!(kw.style.bold);
    }

    #[test]
    fn test_light_theme_comment_italic() {
        let t = Theme::light();
        let c = t.get(TokenKind::Comment);
        assert!(c.style.italic);
    }

    #[test]
    fn test_unknown_token_returns_plain() {
        let t = Theme::dark();
        // Should not panic even for less common
        let s = t.get(TokenKind::Plain);
        assert_eq!(s.fg, Color::WHITE);
    }

    #[test]
    fn test_style_builder() {
        let s = TokenStyle::new(Color::RED).with_bg(Color::WHITE).with_style(Style::underline());
        assert_eq!(s.fg, Color::RED);
        assert_eq!(s.bg, Some(Color::WHITE));
        assert!(s.style.underline);
    }
}
