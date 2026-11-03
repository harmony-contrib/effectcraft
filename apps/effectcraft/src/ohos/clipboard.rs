//! The pasteboard bridge shares the native plugins used by PhotoCraft and PdfCraft.

use openharmony_ability::OpenHarmonyApp;
use winit::platform::ohos::plugins::{clipboard::ClipboardClient, permission::PermissionExt as _};

pub fn text_service(ability: OpenHarmonyApp) -> Box<dyn Fn() -> Option<String>> {
    Box::new(move || {
        // This existing EffectCraft hook is synchronous. Rust runs on the native
        // Ability thread; the permission dialog is handled on ArkTS's UI thread.
        // Read the answer for this paste instead of returning a stale cache entry.
        let result = pollster::block_on(async {
            ability.request_permission("ohos.permission.READ_PASTEBOARD").await.map_err(|error| error.to_string())?;
            ClipboardClient::new(&ability).map_err(|error| error.to_string())?.read_text().await.map_err(|error| error.to_string())
        });
        match result {
            Ok(text) => text,
            Err(error) => {
                log::warn!("OpenHarmony pasteboard read failed: {error}");
                None
            }
        }
    })
}
