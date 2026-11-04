//! A downstream host's identity, separate from engine and project-format names.

/// A host-owned name, PNG icon and attribution for a separately distributed application.
/// Leaving the host service unset preserves EffectCraft's original identity.
#[derive(Clone, Copy, Debug)]
pub struct Branding {
    pub name: &'static str,
    pub icon_png: &'static [u8],
    pub attribution: &'static str,
}

impl Branding {
    pub(crate) fn icon_key(self) -> egui::Id {
        egui::Id::unique(("effectcraft-host-brand", self.icon_png.as_ptr(), self.icon_png.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::Branding;

    #[test]
    fn host_icons_have_separate_cache_entries() {
        static FIRST: [u8; 1] = [1];
        static SECOND: [u8; 1] = [2];
        let first = Branding { name: "First", icon_png: &FIRST, attribution: "Original authors" };
        let second = Branding { name: "Second", icon_png: &SECOND, attribution: "Original authors" };
        assert_ne!(first.icon_key(), second.icon_key());
    }
    #[test]
    fn host_name_controls_the_project_window_title() {
        let mut app = crate::EffectcraftApp::new(effectcraft_engine::Session::default());
        assert_eq!(app.display_name(), "EffectCraft");
        app.hooks.branding = Some(Branding { name: "Example", icon_png: &[], attribution: "Original authors" });
        assert_eq!(app.display_name(), "Example");
        assert!(crate::panels::unsaved::window_title(&app).starts_with("Example - "));
    }
}
