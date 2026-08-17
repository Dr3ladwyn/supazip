//! Monoline toolbar icons drawn with egui strokes (no emoji, no PNG).
//!
//! Each control keeps its text label so the action stays readable and
//! exposed to screen readers.

use eframe::egui::{self, pos2, vec2, Pos2, Rect, Sense, Stroke, TextStyle, TextWrapMode, Widget};

/// Toolbar glyph set. Coordinates are normalized to the icon rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarIcon {
    Open,
    Recent,
    Extract,
    Create,
    Test,
    Cancel,
}

/// Button that paints a stroke icon to the left of `label`.
pub struct ToolbarButton {
    icon: ToolbarIcon,
    label: String,
}

impl ToolbarButton {
    pub fn new(icon: ToolbarIcon, label: impl Into<String>) -> Self {
        Self {
            icon,
            label: label.into(),
        }
    }
}

impl Widget for ToolbarButton {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let galley = egui::WidgetText::from(self.label).into_galley(
            ui,
            Some(TextWrapMode::Extend),
            f32::INFINITY,
            TextStyle::Button,
        );
        let icon_size = 14.0;
        let gap = 6.0;
        let pad = ui.spacing().button_padding;
        let height = (galley.size().y + pad.y * 2.0).max(icon_size + pad.y * 2.0);
        let size = vec2(pad.x * 2.0 + icon_size + gap + galley.size().x, height);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);
            let paint_rect = rect.expand(visuals.expansion);
            ui.painter()
                .rect_filled(paint_rect, visuals.corner_radius, visuals.weak_bg_fill);
            ui.painter().rect_stroke(
                paint_rect,
                visuals.corner_radius,
                visuals.bg_stroke,
                egui::StrokeKind::Inside,
            );

            let icon_rect = Rect::from_center_size(
                pos2(rect.left() + pad.x + icon_size * 0.5, rect.center().y),
                vec2(icon_size, icon_size),
            );
            paint_icon(ui.painter(), icon_rect, self.icon, visuals.fg_stroke);

            let text_pos = pos2(
                icon_rect.right() + gap,
                rect.center().y - galley.size().y * 0.5,
            );
            ui.painter()
                .galley(text_pos, galley, visuals.fg_stroke.color);
        }

        response
    }
}

fn paint_icon(painter: &egui::Painter, rect: Rect, icon: ToolbarIcon, stroke: Stroke) {
    let p = |x: f32, y: f32| -> Pos2 {
        pos2(
            rect.left() + rect.width() * x,
            rect.top() + rect.height() * y,
        )
    };
    let line = |a: Pos2, b: Pos2| painter.line_segment([a, b], stroke);

    match icon {
        ToolbarIcon::Open => {
            // Folder: tab + body.
            line(p(0.12, 0.28), p(0.42, 0.28));
            line(p(0.42, 0.28), p(0.52, 0.40));
            line(p(0.52, 0.40), p(0.88, 0.40));
            line(p(0.88, 0.40), p(0.88, 0.86));
            line(p(0.88, 0.86), p(0.12, 0.86));
            line(p(0.12, 0.86), p(0.12, 0.28));
        }
        ToolbarIcon::Recent => {
            painter.circle_stroke(rect.center(), rect.width() * 0.38, stroke);
            line(rect.center(), p(0.50, 0.28));
            line(rect.center(), p(0.74, 0.58));
        }
        ToolbarIcon::Extract => {
            // Archive box + upward arrow.
            line(p(0.18, 0.48), p(0.18, 0.88));
            line(p(0.18, 0.88), p(0.82, 0.88));
            line(p(0.82, 0.88), p(0.82, 0.48));
            line(p(0.50, 0.78), p(0.50, 0.14));
            line(p(0.50, 0.14), p(0.30, 0.36));
            line(p(0.50, 0.14), p(0.70, 0.36));
        }
        ToolbarIcon::Create => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.16, 0.16), p(0.84, 0.84)),
                1.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            line(p(0.32, 0.50), p(0.68, 0.50));
            line(p(0.50, 0.32), p(0.50, 0.68));
        }
        ToolbarIcon::Test => {
            line(p(0.16, 0.54), p(0.40, 0.80));
            line(p(0.40, 0.80), p(0.86, 0.22));
        }
        ToolbarIcon::Cancel => {
            line(p(0.22, 0.22), p(0.78, 0.78));
            line(p(0.78, 0.22), p(0.22, 0.78));
        }
    }
}
