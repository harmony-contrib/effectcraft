//! OpenHarmony host for the native EffectCraft editor.
#![cfg(target_env = "ohos")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(feature = "ohos-control")]
mod control_server;
mod ohos;

pub use ohos::start;
