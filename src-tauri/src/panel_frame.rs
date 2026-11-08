#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub fn frame(screen: Rect, visible: Rect, notch: f64, content_w: f64, content_h: f64) -> Rect {
    let (top, h) = if notch > 0.0 { (screen.y + screen.h, content_h + notch) } else { (visible.y + visible.h, content_h) };
    Rect { x: screen.x + (screen.w - content_w) / 2.0, y: top - h, w: content_w, h }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn notch_screen_touches_top() {
        let screen = rect(0.0, 0.0, 1512.0, 982.0);
        assert_eq!(frame(screen, rect(0.0, 0.0, 1512.0, 945.0), 32.0, 380.0, 72.0), rect(566.0, 878.0, 380.0, 104.0));
    }

    #[test]
    fn no_notch_screen_below_menu_bar() {
        let screen = rect(0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(frame(screen, rect(0.0, 0.0, 1920.0, 1055.0), 0.0, 380.0, 72.0), rect(770.0, 983.0, 380.0, 72.0));
    }

    #[test]
    fn centred_on_secondary_screen() {
        let screen = rect(1512.0, -200.0, 2560.0, 1440.0);
        assert_eq!(frame(screen, rect(1512.0, -200.0, 2560.0, 1415.0), 0.0, 380.0, 72.0), rect(2602.0, 1143.0, 380.0, 72.0));
    }

    #[test]
    fn hidden_menu_bar_touches_top() {
        let screen = rect(0.0, 0.0, 1920.0, 1080.0);
        let panel = frame(screen, screen, 0.0, 380.0, 72.0);
        assert_eq!(panel.y + panel.h, 1080.0);
    }
}
