//! Design-system v2 runtime: `design/tokens.json` → `egui::Style`.
//!
//! Tokens stay out of `supazip-core`. This module is the only place that
//! maps the dual-theme color trees onto `egui::Visuals` / `egui::Style`.
//!
//! JetBrains Mono is embedded only when `assets/fonts/JetBrainsMono-Regular.ttf`
//! is present at compile time (see `build.rs`). Without the OFL TTF the
//! GUI keeps egui's built-in families (system fallback).

use std::sync::{Arc, OnceLock};

use eframe::egui::{self, Color32, CornerRadius, FontId, Margin, Shadow, Stroke, TextStyle};
#[cfg(embed_jetbrains_mono)]
use eframe::egui::{FontData, FontDefinitions, FontFamily};
use serde::Deserialize;

use crate::settings::ThemePreference;

/// Compile-time copy of the repo-root token mirror.
/// Path is relative to this file: `supazip-gui/src` → repo `design/`.
const TOKENS_JSON: &str = include_str!("../../../design/tokens.json");

/// When the OFL TTF is added under `assets/fonts/`, `build.rs` sets this
/// cfg and the bytes are baked into the binary. Do not download a font
/// into the repo without the OFL license file alongside it.
#[cfg(embed_jetbrains_mono)]
const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");

#[cfg(embed_jetbrains_mono_bold)]
const JETBRAINS_MONO_BOLD: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf");

/// Resolved palette: dark or light. Settings use [`ThemePreference`]
/// (`Dark` / `Light` / `System`); this enum is the post-resolution theme
/// fed to [`Style::from_tokens`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    Dark,
    Light,
}

/// Maps design tokens into an [`egui::Style`].
pub struct Style;

impl Style {
    /// Build an `egui::Style` from the v2 token file for `theme`.
    pub fn from_tokens(theme: ThemeKind) -> egui::Style {
        let tokens = tokens();
        let color = tokens.colors(theme);
        let mut style = match theme {
            ThemeKind::Dark => egui::Theme::Dark.default_style(),
            ThemeKind::Light => egui::Theme::Light.default_style(),
        };

        style.animation_time = tokens.motion.duration.normal as f32 / 1000.0;

        let body = tokens.typography.size.body as f32;
        let caption = tokens.typography.size.caption as f32;
        let heading = tokens.typography.size.heading as f32;
        let title = tokens.typography.size.title as f32;
        style
            .text_styles
            .insert(TextStyle::Small, FontId::monospace(caption));
        style
            .text_styles
            .insert(TextStyle::Body, FontId::monospace(body));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::monospace(body));
        style
            .text_styles
            .insert(TextStyle::Monospace, FontId::monospace(body));
        style
            .text_styles
            .insert(TextStyle::Heading, FontId::monospace(heading));
        style
            .text_styles
            .insert(TextStyle::Name("title".into()), FontId::monospace(title));

        let item = tokens.spacing.s3 as f32;
        let tight = tokens.spacing.s2 as f32;
        style.spacing.item_spacing = egui::vec2(item, tight);
        style.spacing.button_padding = egui::vec2(item, tight);
        style.spacing.window_margin = Margin::same(tokens.spacing.s4 as i8);
        style.spacing.menu_margin = Margin::same(tokens.spacing.s3 as i8);

        style.visuals = visuals_from_tokens(theme, color, tokens);
        style
    }
}

/// Install both theme styles and the user's preference on `ctx`.
///
/// Safe to call every frame: the mapped styles are built once.
pub fn apply(ctx: &egui::Context, preference: ThemePreference) {
    ctx.set_style_of(egui::Theme::Dark, dark_style().clone());
    ctx.set_style_of(egui::Theme::Light, light_style().clone());
    ctx.set_theme(match preference {
        ThemePreference::Dark => egui::ThemePreference::Dark,
        ThemePreference::Light => egui::ThemePreference::Light,
        ThemePreference::System => egui::ThemePreference::System,
    });
}

/// Register JetBrains Mono when the TTF was present at compile time.
///
/// Without the file, this is a no-op and egui keeps its default families.
pub fn install_fonts(ctx: &egui::Context) {
    #[cfg(embed_jetbrains_mono)]
    {
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "jetbrains_mono".to_owned(),
            Arc::new(FontData::from_static(JETBRAINS_MONO_REGULAR)),
        );
        #[cfg(embed_jetbrains_mono_bold)]
        {
            fonts.font_data.insert(
                "jetbrains_mono_bold".to_owned(),
                Arc::new(FontData::from_static(JETBRAINS_MONO_BOLD)),
            );
        }
        if let Some(proportional) = fonts.families.get_mut(&FontFamily::Proportional) {
            proportional.insert(0, "jetbrains_mono".to_owned());
        }
        if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
            mono.insert(0, "jetbrains_mono".to_owned());
        }
        ctx.set_fonts(fonts);
    }
    #[cfg(not(embed_jetbrains_mono))]
    {
        let _ = ctx;
    }
}

/// Toolbar / status bar: elevation `level_1` (1 px hairline, surface fill).
pub fn chrome_frame(ctx: &egui::Context) -> egui::Frame {
    let color = active_colors(ctx);
    egui::Frame::new()
        .fill(color.bg.surface)
        .stroke(Stroke::new(1.0, color.border.subtle))
        .inner_margin(Margin::same(tokens().spacing.s3 as i8))
        .corner_radius(0.0)
        .shadow(Shadow::NONE)
}

/// Dialogs / modals: elevation `level_2` (hairline + drop shadow).
pub fn dialog_frame(ctx: &egui::Context) -> egui::Frame {
    let color = active_colors(ctx);
    let tokens = tokens();
    egui::Frame::new()
        .fill(color.bg.surface)
        .stroke(Stroke::new(1.0, color.border.subtle))
        .inner_margin(Margin::same(tokens.spacing.s4 as i8))
        .corner_radius(tokens.radius.md as u8)
        .shadow(level_2_shadow())
}

fn level_2_shadow() -> Shadow {
    // tokens.elevation.level_2: "0 1px 3px 0 rgba(0,0,0,0.4), …"
    Shadow {
        offset: [0, 1],
        blur: 3,
        spread: 0,
        color: Color32::from_black_alpha(102),
    }
}

fn dark_style() -> &'static Arc<egui::Style> {
    static STYLE: OnceLock<Arc<egui::Style>> = OnceLock::new();
    STYLE.get_or_init(|| Arc::new(Style::from_tokens(ThemeKind::Dark)))
}

fn light_style() -> &'static Arc<egui::Style> {
    static STYLE: OnceLock<Arc<egui::Style>> = OnceLock::new();
    STYLE.get_or_init(|| Arc::new(Style::from_tokens(ThemeKind::Light)))
}

fn tokens() -> &'static DesignTokens {
    static TOKENS: OnceLock<DesignTokens> = OnceLock::new();
    TOKENS.get_or_init(|| {
        serde_json::from_str(TOKENS_JSON)
            .expect("design/tokens.json (include_str) must parse; check_tokens.py is the gate")
    })
}

fn active_colors(ctx: &egui::Context) -> &ColorTree {
    tokens().colors(match ctx.theme() {
        egui::Theme::Dark => ThemeKind::Dark,
        egui::Theme::Light => ThemeKind::Light,
    })
}

fn visuals_from_tokens(
    theme: ThemeKind,
    color: &ColorTree,
    tokens: &DesignTokens,
) -> egui::Visuals {
    let mut visuals = match theme {
        ThemeKind::Dark => egui::Visuals::dark(),
        ThemeKind::Light => egui::Visuals::light(),
    };
    let radius = CornerRadius::same(tokens.radius.md as u8);
    let widget =
        |bg: Color32, weak: Color32, border: Color32, fg: Color32| egui::style::WidgetVisuals {
            bg_fill: bg,
            weak_bg_fill: weak,
            bg_stroke: Stroke::new(1.0, border),
            corner_radius: radius,
            fg_stroke: Stroke::new(1.0, fg),
            expansion: 0.0,
        };

    visuals.dark_mode = matches!(theme, ThemeKind::Dark);
    visuals.override_text_color = None;
    visuals.weak_text_color = Some(color.fg.muted);
    visuals.widgets.noninteractive = widget(
        color.bg.base,
        color.bg.base,
        color.border.subtle,
        color.fg.secondary,
    );
    visuals.widgets.inactive = widget(
        color.bg.surface,
        color.bg.surface,
        color.border.subtle,
        color.fg.primary,
    );
    visuals.widgets.hovered = widget(
        color.bg.raised,
        color.bg.raised,
        color.border.focus,
        color.accent.primary_hover,
    );
    visuals.widgets.active = widget(
        color.bg.raised,
        color.bg.raised,
        color.accent.pressed,
        color.accent.pressed,
    );
    visuals.widgets.open = widget(
        color.bg.raised,
        color.bg.raised,
        color.border.strong,
        color.fg.primary,
    );
    visuals.selection.bg_fill = color.accent.primary.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, color.accent.primary);
    visuals.hyperlink_color = color.accent.primary;
    visuals.faint_bg_color = color.bg.sunken;
    visuals.extreme_bg_color = color.bg.sunken;
    visuals.text_edit_bg_color = Some(color.bg.sunken);
    visuals.code_bg_color = color.bg.sunken;
    visuals.warn_fg_color = color.semantic.warning;
    visuals.error_fg_color = color.semantic.danger;
    visuals.window_fill = color.bg.surface;
    visuals.window_stroke = Stroke::new(1.0, color.border.subtle);
    visuals.window_corner_radius = radius;
    visuals.window_shadow = level_2_shadow();
    visuals.menu_corner_radius = radius;
    visuals.panel_fill = color.bg.base;
    visuals.popup_shadow = level_2_shadow();
    visuals
}

// ---------------------------------------------------------------------------
// Token JSON shape (mirrors design/tokens.json).
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct DesignTokens {
    #[allow(dead_code)]
    meta: Meta,
    themes: Themes,
    typography: Typography,
    spacing: Spacing,
    radius: Radius,
    motion: Motion,
}

impl DesignTokens {
    fn colors(&self, theme: ThemeKind) -> &ColorTree {
        match theme {
            ThemeKind::Dark => &self.themes.dark.color,
            ThemeKind::Light => &self.themes.light.color,
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct Meta {
    version: String,
}

#[derive(Debug, serde::Deserialize)]
struct Themes {
    dark: ThemeColors,
    light: ThemeColors,
}

#[derive(Debug, serde::Deserialize)]
struct ThemeColors {
    color: ColorTree,
}

#[derive(Debug, serde::Deserialize)]
struct ColorTree {
    bg: Bg,
    fg: Fg,
    accent: Accent,
    semantic: Semantic,
    border: Border,
}

#[derive(Debug, serde::Deserialize)]
struct Bg {
    #[serde(deserialize_with = "hex_color")]
    base: Color32,
    #[serde(deserialize_with = "hex_color")]
    surface: Color32,
    #[serde(deserialize_with = "hex_color")]
    sunken: Color32,
    #[serde(deserialize_with = "hex_color")]
    raised: Color32,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct Fg {
    #[serde(deserialize_with = "hex_color")]
    primary: Color32,
    #[serde(deserialize_with = "hex_color")]
    secondary: Color32,
    #[serde(deserialize_with = "hex_color")]
    muted: Color32,
    #[serde(deserialize_with = "hex_color")]
    inverse: Color32,
}

#[derive(Debug, serde::Deserialize)]
struct Accent {
    #[serde(deserialize_with = "hex_color")]
    primary: Color32,
    #[serde(deserialize_with = "hex_color")]
    primary_hover: Color32,
    #[serde(deserialize_with = "hex_color")]
    pressed: Color32,
}

#[derive(Debug, serde::Deserialize)]
struct Semantic {
    #[serde(deserialize_with = "hex_color")]
    warning: Color32,
    #[serde(deserialize_with = "hex_color")]
    danger: Color32,
}

#[derive(Debug, serde::Deserialize)]
struct Border {
    #[serde(deserialize_with = "hex_color")]
    subtle: Color32,
    #[serde(deserialize_with = "hex_color")]
    strong: Color32,
    #[serde(deserialize_with = "hex_color")]
    focus: Color32,
}

#[derive(Debug, serde::Deserialize)]
struct Typography {
    size: TypeSize,
}

#[derive(Debug, serde::Deserialize)]
struct TypeSize {
    caption: u32,
    body: u32,
    heading: u32,
    title: u32,
}

#[derive(Debug, serde::Deserialize)]
struct Spacing {
    #[serde(rename = "2")]
    s2: u32,
    #[serde(rename = "3")]
    s3: u32,
    #[serde(rename = "4")]
    s4: u32,
}

#[derive(Debug, serde::Deserialize)]
struct Radius {
    md: u32,
}

#[derive(Debug, serde::Deserialize)]
struct Motion {
    duration: MotionDuration,
}

#[derive(Debug, serde::Deserialize)]
struct MotionDuration {
    normal: u32,
}

fn hex_color<'de, D>(deserializer: D) -> Result<Color32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    parse_hex(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid hex color: {s}")))
}

fn parse_hex(hex: &str) -> Option<Color32> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some(Color32::from_rgb(
        ((n >> 16) & 0xff) as u8,
        ((n >> 8) & 0xff) as u8,
        (n & 0xff) as u8,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_json_parses_v2() {
        let t = tokens();
        assert_eq!(t.meta.version, "2.0.0");
        assert_eq!(
            t.colors(ThemeKind::Dark).accent.primary,
            parse_hex("#58A6FF").unwrap()
        );
        assert_eq!(
            t.colors(ThemeKind::Light).accent.primary,
            parse_hex("#0969DA").unwrap()
        );
    }

    #[test]
    fn from_tokens_sets_dark_mode_flag() {
        let dark = Style::from_tokens(ThemeKind::Dark);
        assert!(dark.visuals.dark_mode);
        assert_eq!(dark.visuals.panel_fill, parse_hex("#0E1116").unwrap());
        let light = Style::from_tokens(ThemeKind::Light);
        assert!(!light.visuals.dark_mode);
        assert_eq!(light.visuals.panel_fill, parse_hex("#F6F8FA").unwrap());
    }

    #[test]
    fn parse_hex_accepts_hash_prefix() {
        assert_eq!(
            parse_hex("#58A6FF"),
            Some(Color32::from_rgb(0x58, 0xA6, 0xFF))
        );
        assert_eq!(
            parse_hex("0E1116"),
            Some(Color32::from_rgb(0x0E, 0x11, 0x16))
        );
        assert_eq!(parse_hex("zzz"), None);
    }

    #[test]
    fn dialog_shadow_matches_level_2() {
        let s = level_2_shadow();
        assert_eq!(s.offset, [0, 1]);
        assert_eq!(s.blur, 3);
    }
}
