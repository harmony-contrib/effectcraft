//! Native Ability registration; the editor lives in `effectcraft`.
#![cfg(target_env = "ohos")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[openharmony_ability_derive::ability]
fn openharmony(app: openharmony_ability::OpenHarmonyApp) {
    if let Err(error) = effectcraft::start(app) {
        log::error!("EffectCraft startup failed: {error}");
    }
}
