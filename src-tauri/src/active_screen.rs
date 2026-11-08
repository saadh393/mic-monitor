use crate::panel_frame::Rect;

fn contains(screen: &Rect, x: f64, y: f64) -> bool {
    x >= screen.x && x < screen.x + screen.w && y >= screen.y && y < screen.y + screen.h
}

pub fn pick(screens: &[Rect], window: Option<Rect>, fallback: usize) -> usize {
    window
        .and_then(|w| screens.iter().position(|screen| contains(screen, w.x + w.w / 2.0, w.y + w.h / 2.0)))
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    fn screens() -> Vec<Rect> {
        vec![rect(0.0, 0.0, 1512.0, 982.0), rect(1512.0, 0.0, 1920.0, 1080.0)]
    }

    #[test]
    fn picks_screen_with_window_centre() {
        assert_eq!(pick(&screens(), Some(rect(1600.0, 100.0, 800.0, 600.0)), 0), 1);
    }

    #[test]
    fn falls_back_without_window() {
        assert_eq!(pick(&screens(), None, 0), 0);
    }

    #[test]
    fn falls_back_when_window_off_screen() {
        assert_eq!(pick(&screens(), Some(rect(-5000.0, -5000.0, 100.0, 100.0)), 1), 1);
    }
}
