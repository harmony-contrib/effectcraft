//! Nonblocking file dialogs and persistent FileShare grants, following the sibling hosts.
//!
//! Atomic writes require a grant to the containing folder, rather than just the file.

use effectcraft_engine::config::{ConfigStore, DirConfig, atomic_write};
use effectcraft_ui_egui::{DialogDone, FileDialog};
use openharmony_ability::OpenHarmonyApp;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};
use winit::platform::ohos::{
    drag_drop::{file_path, file_uri},
    file_access::{self, permissions},
    plugins::files::{FileDialogFilter, FileDialogOptions, FilesExt as _},
    task,
};

const SAVE_FOLDER_MODE: u32 = 1 | 2;
const MAX_GRANTS: usize = 1024;
const MAX_URI_BYTES: usize = 16 * 1024;
const MAX_GRANTS_BYTES: usize = 1024 * 1024;

pub struct Grants {
    directory: PathBuf,
    uris: Mutex<BTreeMap<String, u32>>,
    writable: Mutex<Vec<PathBuf>>,
}

impl Grants {
    pub fn new(directory: PathBuf) -> Self {
        Self { writable: Mutex::new(vec![directory.clone()]), directory, uris: Mutex::new(BTreeMap::new()) }
    }

    pub fn restore(&self) {
        let saved = DirConfig::new(self.directory.clone()).read_bounded("file-grants.json", MAX_GRANTS_BYTES);
        let saved: BTreeMap<String, u32> = match saved {
            Ok(Some(text)) => match serde_json::from_str(&text) {
                Ok(saved) => saved,
                Err(error) => {
                    log::warn!("Cannot read saved file grants: {error}; select the files again");
                    return;
                }
            },
            Ok(None) => return,
            Err(error) => {
                log::warn!("Cannot read saved file grants: {error}; select the files again");
                return;
            }
        };
        if saved.len() > MAX_GRANTS || saved.keys().any(|uri| uri.len() > MAX_URI_BYTES) {
            log::warn!("Saved file grants exceed supported limits; select the files again");
            return;
        }
        for (uri, mode) in saved {
            if !matches!(mode, 1 | SAVE_FOLDER_MODE) {
                log::warn!("Ignoring a saved file grant with an unsupported access mode");
                continue;
            }
            let policy = permissions::PolicyInfo { uri: uri.clone(), operation_mode: mode };
            match permissions::activate_permission(&[policy]) {
                Ok(failed) if failed.is_empty() => self.track(&uri, mode),
                _ => log::warn!("A saved file grant is no longer available; select the file again"),
            }
        }
    }

    fn track(&self, uri: &str, mode: u32) {
        self.uris.lock().unwrap_or_else(PoisonError::into_inner).entry(uri.to_owned()).and_modify(|old| *old |= mode).or_insert(mode);
        if mode == SAVE_FOLDER_MODE
            && let Some(path) = file_path(uri).and_then(|path| path.canonicalize().ok()).filter(|path| path.is_dir())
        {
            let mut writable = self.writable.lock().unwrap_or_else(PoisonError::into_inner);
            if !writable.contains(&path) {
                writable.push(path);
            }
        }
    }

    pub fn persist(&self, uris: &[String], write: bool) -> Result<Vec<PathBuf>, String> {
        if uris.len() > MAX_GRANTS || uris.iter().any(|uri| uri.len() > MAX_URI_BYTES) {
            return Err("Too many selected files or an overlong file URI; select fewer files".into());
        }
        {
            let saved = self.uris.lock().unwrap_or_else(PoisonError::into_inner);
            let added = uris.iter().filter(|uri| !saved.contains_key(*uri)).count();
            if saved.len().saturating_add(added) > MAX_GRANTS {
                return Err("The persistent file grant limit has been reached; choose an already authorized folder".into());
            }
        }
        let paths = uris.iter().map(|uri| file_path(uri).ok_or_else(|| "Cannot resolve the selected file URI".to_owned())).collect::<Result<Vec<_>, _>>()?;
        let individual: Vec<_> = uris.iter().zip(&paths).filter(|(_, path)| !self.can_save(path)).map(|(uri, _)| uri.clone()).collect();
        file_access::persist_paths(&individual, write)?;
        for uri in &individual {
            self.track(uri, if write { SAVE_FOLDER_MODE } else { 1 });
        }
        let uris = self.uris.lock().unwrap_or_else(PoisonError::into_inner);
        let bytes = serde_json::to_vec(&*uris).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_GRANTS_BYTES {
            return Err("Persistent file grants exceed the size limit; authorize their containing folder instead".into());
        }
        atomic_write(&self.directory.join("file-grants.json"), &bytes).map_err(|error| format!("Cannot save file grants: {error}"))?;
        Ok(paths)
    }

    pub fn can_save(&self, path: &Path) -> bool {
        path.parent()
            .and_then(|parent| parent.canonicalize().ok())
            .is_some_and(|parent| self.writable.lock().unwrap_or_else(PoisonError::into_inner).iter().any(|directory| parent.starts_with(directory)))
    }

    pub async fn authorize_save(&self, app: &OpenHarmonyApp, path: &Path) -> Result<bool, String> {
        if self.can_save(path) {
            return Ok(true);
        }
        let parent = path.parent().ok_or("The save path has no containing folder")?;
        let uri = file_uri(parent).ok_or("Cannot resolve the save folder URI")?;
        let response = app.show_file_dialog(FileDialogOptions::new("open-folder").default_location(uri)).await.map_err(|error| error.to_string())?;
        let Some(selected) = response.files.first() else { return Ok(false) };
        let selected = file_path(selected).and_then(|path| path.canonicalize().ok());
        if selected.is_none() || selected != parent.canonicalize().ok() {
            return Err(format!("Select {} to save safely, or use Save As to choose another folder", parent.display()));
        }
        self.persist(&response.files, true)?;
        Ok(true)
    }

    pub fn resolve_want(&self, uri: &str) -> Option<PathBuf> {
        match self.persist(&[uri.to_owned()], false) {
            Ok(paths) => paths.into_iter().next(),
            Err(error) => {
                log::warn!("Cannot keep the launch URI grant: {error}; access may expire when EffectCraft closes");
                file_path(uri)
            }
        }
    }
}

pub fn dialog(ability: OpenHarmonyApp, grants: Arc<Grants>, ctx: egui::Context, request: FileDialog, done: DialogDone) {
    task::spawn(async move {
        let result = show_dialog(&ability, &grants, request).await;
        match result {
            Ok(paths) => done(paths),
            Err(error) => {
                log::error!("EffectCraft file dialog failed: {error}");
                ctx.data_mut(|data| data.insert_temp(egui::Id::unique("effectcraft-ohos-file-error"), error));
                done(Vec::new());
            }
        }
        ctx.request_repaint();
    });
}

async fn show_dialog(app: &OpenHarmonyApp, grants: &Grants, request: FileDialog) -> Result<Vec<String>, String> {
    let filter = |name, extensions: Vec<String>| FileDialogFilter::new().name(name).pattern(extensions.join(";"));
    let (options, save, folder) = match request {
        FileDialog::Open { exts, many } => (FileDialogOptions::new("open-file").allow_many(many).filters(vec![filter("Supported Files", exts)]), false, false),
        FileDialog::OpenProject => {
            (FileDialogOptions::new("open-file").filters(vec![filter("EffectCraft Project", vec!["ecproj".into(), "ecprojx".into()])]), false, false)
        }
        FileDialog::Folder => (FileDialogOptions::new("open-folder"), false, true),
        FileDialog::Save { default, ext, project } => {
            let path = Path::new(&default);
            let suggested = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| "Untitled.ecproj".into());
            let mut options = FileDialogOptions::new("save-file").suggested_name(suggested);
            if !ext.is_empty() {
                options = options.filters(vec![filter(if project { "EffectCraft Project" } else { "Export" }, vec![ext])]);
            }
            if let Some(uri) = path.parent().and_then(file_uri) {
                options = options.default_location(uri);
            }
            (options, true, false)
        }
    };
    let response = app.show_file_dialog(options).await.map_err(|error| error.to_string())?;
    let paths = if save {
        let Some(uri) = response.files.first() else { return Ok(Vec::new()) };
        let path = file_path(uri).ok_or("Cannot resolve the selected save URI")?;
        if !grants.authorize_save(app, &path).await? {
            return Ok(Vec::new());
        }
        vec![path]
    } else {
        grants.persist(&response.files, folder)?
    };
    Ok(paths.into_iter().map(|path| path.to_string_lossy().into_owned()).collect())
}

/// Engine writes keep atomic replacement and report the remedy for an expired/read-only grant.
pub struct GrantedFiles(pub Arc<Grants>);

impl effectcraft_engine::Services for GrantedFiles {
    fn read_file(&self, path: &str) -> std::io::Result<Vec<u8>> {
        effectcraft_engine::Services::read_file(&effectcraft_engine::FsServices, path)
    }

    fn write_file(&self, path: &str, data: &[u8]) -> std::io::Result<()> {
        if !self.0.can_save(Path::new(path)) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Use File > Save As and authorize the containing folder before saving this file",
            ));
        }
        atomic_write(Path::new(path), data)
    }
}
