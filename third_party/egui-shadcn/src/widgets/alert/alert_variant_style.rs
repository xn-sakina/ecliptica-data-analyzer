//! Maps AlertVariant to concrete style values.

/// Resolves alert colors from variant.
pub fn resolve_alert_style(
    theme: &crate::theme::shadcn_theme::ShadcnTheme,
    variant: crate::tokens::alert_variant::AlertVariant,
) -> super::resolved_alert_style::ResolvedAlertStyle {
    match variant {
        crate::tokens::alert_variant::AlertVariant::Default => {
            super::resolved_alert_style::ResolvedAlertStyle {
                bg: theme.background,
                fg: theme.foreground,
                border: theme.border,
            }
        }
        crate::tokens::alert_variant::AlertVariant::Destructive => {
            super::resolved_alert_style::ResolvedAlertStyle {
                bg: theme.background,
                fg: theme.destructive,
                border: theme.destructive,
            }
        }
        crate::tokens::alert_variant::AlertVariant::Success => {
            semantic_alert(theme, egui::Color32::from_rgb(74, 222, 128))
        }
        crate::tokens::alert_variant::AlertVariant::Warning => {
            semantic_alert(theme, egui::Color32::from_rgb(251, 191, 36))
        }
        crate::tokens::alert_variant::AlertVariant::Info => {
            semantic_alert(theme, egui::Color32::from_rgb(96, 165, 250))
        }
    }
}

fn semantic_alert(
    theme: &crate::theme::shadcn_theme::ShadcnTheme,
    color: egui::Color32,
) -> super::resolved_alert_style::ResolvedAlertStyle {
    super::resolved_alert_style::ResolvedAlertStyle {
        bg: crate::paint::interpolate_color::interpolate_color(theme.background, color, 0.08),
        fg: color,
        border: crate::paint::interpolate_color::interpolate_color(theme.border, color, 0.55),
    }
}
