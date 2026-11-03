# EffectCraft for OpenHarmony

This host follows the native Ability and packaging layout in `../photocraft` and
`../pdfcraft`. It builds `libeffectcraft_ohos.so` for `aarch64-unknown-linux-ohos`
and packages it as `ai.storyteller.effectcraft`. The desktop entry point is shared
only through existing engine and frontend APIs; it has no OpenHarmony branches.

`apps/effectcraft-ohos` contains only Ability registration. Its `unsafe_code = "deny"`
exception permits the registration macro's FFI expansion; the application host in
`apps/effectcraft/src/ohos` inherits the workspace's `unsafe_code = "forbid"`.

## Build

The stable Rust toolchain needs the `aarch64-unknown-linux-ohos` target. DevEco
Studio supplies the native SDK, Node, OHPM, Java and Hvigor. Like the reference
apps, the HAP build uses the ArkTS bridge packages from `../openharmony-ability`;
set `OPENHARMONY_ABILITY_DIR` to use another checkout.

```sh
OHOS_NDK_HOME=/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony \
  ./packaging/ohos/build-rust.sh --features ohos-control
./packaging/ohos/build-hap.sh --features ohos-control
```

`DEVECO_HOME` can override `/Applications/DevEco-Studio.app/Contents`. Generated
projects and assets stay in `target/effectcraft-hap`; the app icon is copied from
the attributed EffectCraft icon in `assets/app-icon`. The unsigned result is:

`target/effectcraft-hap/entry/build/default/outputs/default/entry-default-unsigned.hap`

Neither build script connects to a device or installs an application.

## Host services

The Ability's preferences directory holds rotating `logs/effectcraft*.log`,
`settings.json`, `ui.json` (workspace layouts), `ui.ron` (eframe window state),
`file-grants.json`, the recovery sentinel and `Auto-Save` snapshots. HiLog receives
the same session messages. `/system/fonts` is scanned for text tools and the font
catalogue; the interface explicitly loads HarmonyOS Sans SC and TC when present.
Floating panels remain embedded in the Ability window. GLES is the default wgpu
backend unless `WGPU_BACKEND` is set.

File dialogs run asynchronously through the Ability bridge. Save As asks for the
containing folder because an atomic save must create and rename a sibling temporary
file. The folder grant is persisted and reactivated on the next launch. Saving a
file opened with only a read grant reports an actionable Save As instruction.
Launch and subsequent file-open Wants enter the existing dropped-file command path,
including its unsaved-work prompt. Native pasteboard and system appearance hooks use
the same plugins as the reference apps. Audio preview output is unavailable because
the current cpal dependency has no OpenHarmony backend.

`ohos-control` is opt-in and forwards through the Ability crate to the host. It
starts the existing localhost JSON-lines control server on port 7878 and writes
`{"port":7878}` to `control.json` in the preferences directory. Ordinary builds
do not start that server. Device validation is a separate task.
