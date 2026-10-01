//! Where the viewer's resources come from.
//!
//! A [`Library`] is the game's resource stack (`mg_resman`, in the game's
//! load order) with the viewer's own layers on top:
//!
//! | Priority | Layer |
//! | --- | --- |
//! | 100 | editor buffers (unsaved edits, in memory) |
//! | 99 | the opened file's folder |
//! | 98 | folders the user added |
//! | 30 | haks, modules and ERFs the user added |
//! | … | the game's own layers (development, override, keys) |
//!
//! So a model opened from disk finds the textures beside it, and an edit in
//! the viewer shows without being saved. Folder layers are class
//! `Directory`, so their textures beat override's, as the game's texture
//! rule would have them (DDS over TGA within a class).
//!
//! Parsed models are cached by name; anything that changes the stack
//! (a buffer, a file on disk, a layer) bumps [`Library::generation`] and
//! drops the caches.

pub mod detect;
pub mod watch;

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use mg_core::{Language, ResRef, ResType};
use mg_mdl::Model;
use mg_resman::{
    DirContainer, ErfContainer, GameInstall, LayerClass, MemContainer, ResKey, ResMan, priority,
};
use mg_rules::GameData;
use thiserror::Error;

pub use detect::Kind;

/// The editor buffers' layer.
pub const BUFFERS: u32 = 100;
/// The opened file's folder.
pub const OPENED: u32 = 99;
/// Folders the user added.
pub const FOLDERS: u32 = 98;
/// Haks, modules and ERFs the user added (the game's hak priority).
pub const ARCHIVES: u32 = priority::HAK;

const BUFFERS_LABEL: &str = "editor buffers";
const OPENED_LABEL: &str = "opened folder";

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("{path}: {message}")]
    Io { path: PathBuf, message: String },
    #[error(transparent)]
    Rules(#[from] mg_rules::RulesError),
    #[error(transparent)]
    Res(#[from] mg_resman::ResError),
}

impl LibraryError {
    fn io(path: &Path, e: std::io::Error) -> LibraryError {
        LibraryError::Io { path: path.to_path_buf(), message: e.to_string() }
    }
}

/// Something opened: a file from disk or a resource of the stack.
#[derive(Debug, Clone)]
pub struct Opened {
    /// The file, when opened from disk.
    pub path: Option<PathBuf>,
    pub kind: Kind,
    /// Its resource name and type, when it has one (a file whose name is a
    /// valid resource name is then also found through the opened folder's
    /// layer).
    pub key: Option<ResKey>,
    /// The bytes as read.
    pub data: Arc<[u8]>,
}

impl Opened {
    /// The name to show: the resource name, else the file name.
    pub fn name(&self) -> String {
        match (&self.key, &self.path) {
            (Some(k), _) => k.resref.to_lowercase().to_string(),
            (None, Some(p)) => {
                p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
            }
            (None, None) => String::new(),
        }
    }
}

/// The game's resources and the viewer's layers over them.
#[derive(Debug)]
pub struct Library {
    game: GameData,
    install: Option<GameInstall>,
    opened_folder: Option<PathBuf>,
    folders: Vec<PathBuf>,
    archives: Vec<PathBuf>,
    buffers: BTreeMap<ResKey, Arc<[u8]>>,
    models: Mutex<HashMap<ResRef, Option<Arc<Model>>>>,
    generation: u64,
}

impl Library {
    /// The game's resources (`None`: no game, only files opened from disk).
    pub fn open(install: Option<GameInstall>) -> Result<Library, LibraryError> {
        let game = match &install {
            Some(i) => GameData::open(i)?,
            None => GameData::new(ResMan::new(), mg_tlk::Tlk::new(Language::ENGLISH)),
        };
        Ok(Library {
            game,
            install,
            opened_folder: None,
            folders: Vec::new(),
            archives: Vec::new(),
            buffers: BTreeMap::new(),
            models: Mutex::new(HashMap::new()),
            generation: 0,
        })
    }

    /// The game found where Steam installs it or through `NWN_ROOT`, else no
    /// game.
    pub fn detect() -> Library {
        let install = GameInstall::detect();
        match Library::open(install) {
            Ok(l) => l,
            Err(_) => Library::open(None).expect("an empty library always opens"),
        }
    }

    pub fn install(&self) -> Option<&GameInstall> {
        self.install.as_ref()
    }

    /// The game data (2DA tables, talk tables) over the whole stack.
    pub fn game(&self) -> &GameData {
        &self.game
    }

    pub fn resman(&self) -> &ResMan {
        &self.game.resman
    }

    /// Bumped whenever what the library returns may have changed.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Forgets parsed models and tables (after something changed).
    pub fn changed(&mut self) {
        self.generation += 1;
        self.models.lock().expect("model cache poisoned").clear();
        self.game.invalidate();
    }

    /// A resource's bytes from the highest layer that has it.
    pub fn get(&self, key: &ResKey) -> Option<Cow<'_, [u8]>> {
        self.game.resman.get(key).ok()
    }

    /// The label of the layer a resource comes from.
    pub fn origin(&self, key: &ResKey) -> Option<&str> {
        self.game.resman.origin(key)
    }

    /// A model by resource name (case-insensitive), parsed once.
    pub fn model(&self, name: &str) -> Option<Arc<Model>> {
        let resref = ResRef::from_str(name).ok()?;
        if let Some(m) = self.models.lock().expect("model cache poisoned").get(&resref) {
            return m.clone();
        }
        let m = self
            .get(&ResKey::new(resref, ResType::MDL))
            .and_then(|d| Model::read(&d).ok())
            .map(Arc::new);
        self.models.lock().expect("model cache poisoned").insert(resref, m.clone());
        m
    }

    /// Opens a file from disk: its folder becomes the opened-folder layer.
    pub fn open_file(&mut self, path: &Path) -> Result<Opened, LibraryError> {
        let data: Arc<[u8]> = std::fs::read(path).map_err(|e| LibraryError::io(path, e))?.into();
        let key = path.file_name().and_then(|n| n.to_str()).and_then(ResKey::from_filename);
        let ext = path.extension().and_then(|e| e.to_str());
        let kind = detect::detect(ext, &data);
        let folder = path.parent().map(Path::to_path_buf);
        self.set_opened_folder(folder.as_deref());
        Ok(Opened { path: Some(path.to_path_buf()), kind, key, data })
    }

    /// Opens a resource of the stack.
    pub fn open_resource(&self, key: ResKey) -> Result<Opened, LibraryError> {
        let data: Arc<[u8]> = self.game.resman.get(&key)?.into_owned().into();
        let kind = detect::detect(key.restype.extension(), &data);
        Ok(Opened { path: None, kind, key: Some(key), data })
    }

    /// Opens what a command line names: a file if one exists at `input`,
    /// else a resource (`name.ext`, or a bare name: a model first, then the
    /// other types the viewer shows).
    pub fn open_input(&mut self, input: &str) -> Result<Opened, LibraryError> {
        let path = Path::new(input);
        if path.is_file() {
            return self.open_file(path);
        }
        if let Some(key) = ResKey::from_filename(input)
            && self.game.resman.contains(&key)
        {
            return self.open_resource(key);
        }
        const SHOWN: [ResType; 10] = [
            ResType::MDL,
            ResType::UTC,
            ResType::UTP,
            ResType::UTD,
            ResType::UTI,
            ResType::DDS,
            ResType::TGA,
            ResType::PLT,
            ResType::MTR,
            ResType::WOK,
        ];
        for t in SHOWN {
            if let Some(key) = ResKey::parse(input, t)
                && self.game.resman.contains(&key)
            {
                return self.open_resource(key);
            }
        }
        Err(LibraryError::Io {
            path: path.to_path_buf(),
            message: "no such file, and no resource of that name".into(),
        })
    }

    /// Sets (or clears) the opened file's folder.
    pub fn set_opened_folder(&mut self, folder: Option<&Path>) {
        if self.opened_folder.as_deref() == folder && folder.is_some() {
            // The same folder: pick up files added since.
            self.rescan();
            return;
        }
        self.game.resman.remove(OPENED_LABEL);
        self.opened_folder = folder.map(Path::to_path_buf);
        if let Some(f) = folder {
            self.game.resman.add(
                OPENED,
                OPENED_LABEL,
                LayerClass::Directory,
                DirContainer::open(f),
            );
        }
        self.changed();
    }

    pub fn opened_folder(&self) -> Option<&Path> {
        self.opened_folder.as_deref()
    }

    /// Adds a folder of loose files (below the opened folder, above the
    /// game).
    pub fn add_folder(&mut self, folder: &Path) {
        if self.folders.iter().any(|f| f == folder) {
            return;
        }
        self.folders.push(folder.to_path_buf());
        self.game.resman.add(
            FOLDERS,
            folder_label(folder),
            LayerClass::Directory,
            DirContainer::open(folder),
        );
        self.changed();
    }

    pub fn remove_folder(&mut self, folder: &Path) {
        self.folders.retain(|f| f != folder);
        self.game.resman.remove(&folder_label(folder));
        self.changed();
    }

    pub fn folders(&self) -> &[PathBuf] {
        &self.folders
    }

    /// Adds a hak, module or ERF (the last added is the lowest of them, as a
    /// module's hak list).
    pub fn add_archive(&mut self, path: &Path) -> Result<(), LibraryError> {
        if self.archives.iter().any(|a| a == path) {
            return Ok(());
        }
        let erf = ErfContainer::open(path)?;
        self.archives.push(path.to_path_buf());
        self.game.resman.add(ARCHIVES, archive_label(path), LayerClass::Erf, erf);
        self.changed();
        Ok(())
    }

    pub fn remove_archive(&mut self, path: &Path) {
        self.archives.retain(|a| a != path);
        self.game.resman.remove(&archive_label(path));
        self.changed();
    }

    pub fn archives(&self) -> &[PathBuf] {
        &self.archives
    }

    /// Re-reads the folder layers' listings (files added or removed) and
    /// the archives (written again), in place: each layer keeps its place
    /// among its equals. An archive that does not read keeps what it had.
    pub fn rescan(&mut self) {
        let mut labels: Vec<String> = Vec::new();
        if self.opened_folder.is_some() {
            labels.push(OPENED_LABEL.into());
        }
        labels.extend(self.folders.iter().map(|f| folder_label(f)));
        labels.extend(self.archives.iter().map(|a| archive_label(a)));
        for label in labels {
            let _ = self.game.resman.rescan(&label);
        }
        self.changed();
    }

    /// Puts an unsaved edit above everything (`None` removes it).
    pub fn set_buffer(&mut self, key: ResKey, data: Option<Arc<[u8]>>) {
        match data {
            Some(d) => self.buffers.insert(key, d),
            None => self.buffers.remove(&key),
        };
        let mut mem = MemContainer::new();
        for (k, d) in &self.buffers {
            mem.insert(*k, d.clone());
        }
        if self.game.resman.replace(BUFFERS_LABEL, mem.clone()).is_none() {
            self.game.resman.add(BUFFERS, BUFFERS_LABEL, LayerClass::Directory, mem);
        }
        self.changed();
    }

    pub fn buffer(&self, key: &ResKey) -> Option<&Arc<[u8]>> {
        self.buffers.get(key)
    }

    /// Whether a path is a file the library reads through one of its folder
    /// layers (so a change to it on disk changes what the library returns).
    pub fn reads_from(&self, path: &Path) -> bool {
        let Some(dir) = path.parent() else { return false };
        self.opened_folder.as_deref() == Some(dir) || self.folders.iter().any(|f| f == dir)
    }
}

fn folder_label(folder: &Path) -> String {
    format!("folder:{}", folder.display())
}

fn archive_label(path: &Path) -> String {
    format!("archive:{}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE: &str = "newmodel box\nsetsupermodel box NULL\nclassification character\n\
        beginmodelgeom box\nnode dummy box\n  parent NULL\nendnode\n\
        node trimesh plane\n  parent box\n  bitmap boxtex\n  verts 3\n    0 0 0\n    1 0 0\n    0 1 0\n\
          faces 1\n    0 1 2 1 0 0 0 0\nendnode\nendmodelgeom box\ndonemodel box\n";

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mgv-library-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn opened_folder_and_buffers() {
        let dir = scratch("opened");
        std::fs::write(dir.join("box.mdl"), CUBE).unwrap();
        std::fs::write(dir.join("boxtex.txi"), "mipmap 0\n").unwrap();
        let mut lib = Library::open(None).unwrap();
        let opened = lib.open_file(&dir.join("box.mdl")).unwrap();
        assert_eq!(opened.kind, Kind::Model);
        assert_eq!(opened.name(), "box");
        let txi = ResKey::parse("boxtex", ResType::TXI).unwrap();
        assert_eq!(lib.origin(&txi), Some(OPENED_LABEL));
        let m = lib.model("box").expect("model through the opened folder");
        assert_eq!(m.nodes.len(), 2);

        // An unsaved edit wins, and the cache follows it.
        let key = ResKey::parse("box", ResType::MDL).unwrap();
        let edited = CUBE.replace("node trimesh plane", "node trimesh quad");
        let generation = lib.generation();
        lib.set_buffer(key, Some(edited.into_bytes().into()));
        assert!(lib.generation() > generation);
        assert_eq!(lib.origin(&key), Some(BUFFERS_LABEL));
        assert!(lib.model("box").unwrap().node("quad").is_some());
        lib.set_buffer(key, None);
        assert!(lib.model("box").unwrap().node("plane").is_some());

        // New files show after a rescan.
        std::fs::write(dir.join("other.mdl"), CUBE).unwrap();
        assert!(lib.model("other").is_none());
        lib.rescan();
        assert!(lib.model("other").is_some());
        assert!(lib.reads_from(&dir.join("other.mdl")));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn folders_rank_below_the_opened_folder() {
        let (a, b) = (scratch("rank-a"), scratch("rank-b"));
        std::fs::write(a.join("box.mdl"), CUBE).unwrap();
        std::fs::write(b.join("box.mdl"), CUBE.replace("plane", "other")).unwrap();
        let mut lib = Library::open(None).unwrap();
        lib.add_folder(&b);
        assert!(lib.model("box").unwrap().node("other").is_some());
        lib.open_file(&a.join("box.mdl")).unwrap();
        assert!(lib.model("box").unwrap().node("plane").is_some());
        lib.remove_folder(&b);
        assert_eq!(lib.folders().len(), 0);
        std::fs::remove_dir_all(&a).unwrap();
        std::fs::remove_dir_all(&b).unwrap();
    }

    /// Added folders keep their order through a rescan (the first added
    /// wins).
    #[test]
    fn rescans_keep_the_folders_order() {
        let (a, b) = (scratch("order-a"), scratch("order-b"));
        std::fs::write(a.join("box.mdl"), CUBE).unwrap();
        std::fs::write(b.join("box.mdl"), CUBE.replace("plane", "other")).unwrap();
        let mut lib = Library::open(None).unwrap();
        lib.add_folder(&a);
        lib.add_folder(&b);
        assert!(lib.model("box").unwrap().node("plane").is_some());
        lib.rescan();
        assert!(lib.model("box").unwrap().node("plane").is_some(), "still the first folder's");
        std::fs::remove_dir_all(&a).unwrap();
        std::fs::remove_dir_all(&b).unwrap();
    }

    #[test]
    fn missing_files_are_errors() {
        let mut lib = Library::open(None).unwrap();
        assert!(lib.open_file(Path::new("/nonexistent/x.mdl")).is_err());
        assert!(lib.add_archive(Path::new("/nonexistent/x.hak")).is_err());
    }

    #[test]
    fn game_models_resolve() {
        let root = mg_testkit::corpus!();
        let lib = Library::open(Some(GameInstall::new(root, None, "en"))).unwrap();
        let m = lib.model("plc_a01").expect("a base-game placeable");
        assert!(!m.nodes.is_empty());
        assert!(lib.model("PLC_A01").is_some(), "names are case-insensitive");
        assert!(lib.model("no_such_model").is_none());
    }
}
