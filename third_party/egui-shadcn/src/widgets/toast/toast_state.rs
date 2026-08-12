//! Toast notification state manager.

/// Manages active toast notifications.
#[derive(Default, Clone)]
pub struct ToastState {
    pub(crate) toasts: Vec<super::toast_entry::ToastEntry>,
}

impl ToastState {
    const COMPACT_DURATION_SECS: f64 = 1.5;

    pub fn new() -> Self {
        Self { toasts: Vec::new() }
    }

    /// Adds a toast notification. Uses context time for creation timestamp.
    pub fn add(
        &mut self,
        title: impl Into<String>,
        variant: crate::tokens::toast_variant::ToastVariant,
        time: f64,
    ) {
        self.toasts.push(super::toast_entry::ToastEntry {
            title: title.into(),
            description: None,
            variant,
            created_at: time,
            duration_secs: Self::COMPACT_DURATION_SECS,
        });
    }

    /// Adds a toast with description.
    pub fn add_with_description(
        &mut self,
        title: impl Into<String>,
        description: impl Into<String>,
        variant: crate::tokens::toast_variant::ToastVariant,
        time: f64,
    ) {
        self.toasts.push(super::toast_entry::ToastEntry {
            title: title.into(),
            description: Some(description.into()),
            variant,
            created_at: time,
            duration_secs: Self::COMPACT_DURATION_SECS,
        });
    }

    /// Removes expired toasts.
    pub fn cleanup(&mut self, current_time: f64) {
        self.toasts
            .retain(|t| current_time - t.created_at < t.duration_secs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_toast_expires_quickly() {
        let mut state = ToastState::new();
        state.add(
            "copied",
            crate::tokens::toast_variant::ToastVariant::Success,
            10.0,
        );

        state.cleanup(11.4);
        assert_eq!(state.toasts.len(), 1);
        state.cleanup(11.5);
        assert!(state.toasts.is_empty());
    }
}
