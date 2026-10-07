//! What the viewer keeps between runs.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DecompileWith {
    /// In process (exact; see `mgv_mdl::write`).
    #[default]
    Native,
    Nwnmdlcomp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CompileWith {
    /// The viewer's own compiler, in process (kept under the name it had
    /// in settings files when it stood for a choice between the others).
    #[default]
    #[serde(alias = "Native")]
    Auto,
    Engine,
    Nwnmdlcomp,
}

impl std::fmt::Display for CompileWith {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CompileWith::Auto => "Moonglow's compiler",
            CompileWith::Engine => "the game's compiler",
            CompileWith::Nwnmdlcomp => "nwnmdlcomp",
        })
    }
}

/// What the 3D view draws over the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Overlays {
    /// Walkmeshes, faces coloured by surface material (always on for a
    /// walkmesh opened on its own).
    pub walkmesh: bool,
    pub wireframe: bool,
    pub normals: bool,
    /// Nodes as a tree of lines (bones, hooks, emitters, lights).
    pub skeleton: bool,
    /// The shadow casters (`render 0`, `shadow 1`) in blue in place of the
    /// visible meshes. Not kept between runs.
    #[serde(skip)]
    pub casters: bool,
}

/// The settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The game folder; `None`: found where Steam installs it.
    pub game_root: Option<PathBuf>,
    /// Read the player's override and development folders.
    pub use_user_dir: bool,
    /// Folders added above the game.
    pub folders: Vec<PathBuf>,
    /// Haks, modules and ERFs added above the game.
    pub haks: Vec<PathBuf>,
    /// Files opened lately, newest first.
    pub recent: Vec<PathBuf>,
    pub show_grid: bool,
    pub show_axes: bool,
    pub overlays: Overlays,
    pub decompile_with: DecompileWith,
    pub compile_with: CompileWith,
    /// Where compiled models of game resources go (others: `compiled/`
    /// beside their file).
    pub output_folder: Option<PathBuf>,
    /// Pause after the last keystroke before the view reloads, ms.
    pub reload_delay_ms: u32,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            game_root: None,
            use_user_dir: true,
            folders: Vec::new(),
            haks: Vec::new(),
            recent: Vec::new(),
            show_grid: true,
            show_axes: false,
            overlays: Overlays::default(),
            decompile_with: DecompileWith::Native,
            compile_with: CompileWith::Auto,
            output_folder: None,
            reload_delay_ms: 150,
        }
    }
}

/// How many recent files are kept.
const RECENT: usize = 12;

impl Settings {
    /// Puts a file first among the recent ones.
    pub fn remember(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(RECENT);
    }
}

/// The viewer's own data folder: `%APPDATA%\Moonglow Viewer`,
/// `~/Library/Application Support/Moonglow Viewer`, or
/// `$XDG_DATA_HOME/moonglow-viewer` (`~/.local/share/moonglow-viewer`).
pub fn data_dir() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Moonglow Viewer"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support/Moonglow Viewer"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .map(|d| d.join("moonglow-viewer"))
    }
}

/// The scratch user folder the game's compiler runs in (never the
/// player's).
pub fn scratch_dir() -> PathBuf {
    data_dir().unwrap_or_else(std::env::temp_dir).join("engine-compiler")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_files() {
        let mut s = Settings::default();
        for i in 0..20 {
            s.remember(Path::new(&format!("/m/{i}.mdl")));
        }
        s.remember(Path::new("/m/5.mdl"));
        assert_eq!(s.recent.len(), RECENT);
        assert_eq!(s.recent[0], Path::new("/m/5.mdl"));
        assert_eq!(s.recent.iter().filter(|p| p.ends_with("5.mdl")).count(), 1);
    }
}
