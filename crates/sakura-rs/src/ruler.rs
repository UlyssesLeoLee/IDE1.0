// ruler.rs — ルーラー (桁ルーラー) (sakura 12.2)
//
// エディタ上部に表示する桁ルーラー.
// 1, 5, 10, 20, 30... のマーカー表示.

/// ルーラー設定
#[derive(Debug, Clone)]
pub struct RulerConfig {
    /// 表示開始桁 (1-indexed)
    pub start: usize,
    /// 表示終了桁
    pub end: usize,
    /// 主マーカー間隔 (10 毎)
    pub major_tick: usize,
    /// 副マーカー間隔 (5 毎)
    pub minor_tick: usize,
}

impl Default for RulerConfig {
    fn default() -> Self {
        Self {
            start: 1,
            end: 80,
            major_tick: 10,
            minor_tick: 5,
        }
    }
}

/// ルーラー 1 マス
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RulerCell {
    pub col: usize,
    pub kind: RulerMark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulerMark {
    /// 主マーカー (10 毎) — 数字表示
    Major,
    /// 副マーカー (5 毎) — 中点
    Minor,
    /// 通常位置 — 空白
    Plain,
}

/// ルーラー描画
pub fn render(config: &RulerConfig) -> Vec<RulerCell> {
    let mut cells = Vec::with_capacity(config.end - config.start + 1);
    for col in config.start..=config.end {
        let kind = if col % config.major_tick == 0 {
            RulerMark::Major
        } else if col % config.minor_tick == 0 {
            RulerMark::Minor
        } else {
            RulerMark::Plain
        };
        cells.push(RulerCell { col, kind });
    }
    cells
}

/// マーカー文字列取得
pub fn marker_for(cell: RulerCell) -> &'static str {
    match cell.kind {
        RulerMark::Major => "│",
        RulerMark::Minor => "·",
        RulerMark::Plain => " ",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_ruler() {
        let r = render(&RulerConfig::default());
        assert_eq!(r.len(), 80);
        assert_eq!(r[0].col, 1);
        assert_eq!(r[9].kind, RulerMark::Major);
        assert_eq!(r[4].kind, RulerMark::Minor);
        assert_eq!(r[0].kind, RulerMark::Plain);
    }

    #[test]
    fn test_custom_range() {
        let cfg = RulerConfig { start: 1, end: 20, major_tick: 5, minor_tick: 1 };
        let r = render(&cfg);
        assert_eq!(r.len(), 20);
        assert_eq!(r[4].kind, RulerMark::Major);
    }

    #[test]
    fn test_marker_for() {
        assert_eq!(marker_for(RulerCell { col: 10, kind: RulerMark::Major }), "│");
        assert_eq!(marker_for(RulerCell { col: 5, kind: RulerMark::Minor }), "·");
        assert_eq!(marker_for(RulerCell { col: 1, kind: RulerMark::Plain }), " ");
    }
}
