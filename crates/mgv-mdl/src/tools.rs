//! External model compilers and decompilers.
//!
//! - **nwnmdlcomp**, Torlack's compiler (BSD): fast, offline, a 32-bit
//!   program (Linux and Windows). It predates EE: it drops `normals` and
//!   `tangents` (the game recomputes them), silently drops `materialname`
//!   and `renderhint` (refused here, as Neverblender's `nwn_compile.py`
//!   refuses them), stores Bézier keys in a layout the game reads
//!   differently (refused too), compiles at most 17 bones per skin (EE:
//!   64), and takes only 0 and 1 for `spawntype` (EE-compiled models hold
//!   −1, given to it as 0, which the game draws alike: not a trail).
//! - **The game's own compiler**: `nwmain -userdirectory DIR compilemodel
//!   NAME` compiles `DIR/development/NAME.mdl` into `DIR/modelcompiler/`.
//!   It keeps EE features but needs an OpenGL context (on Linux it runs in
//!   gamescope's headless backend when gamescope is installed) and cannot
//!   compile skin meshes from the command line. It always runs with a
//!   scratch user directory the viewer owns, never the player's.
//!
//! Each run works in a fresh temporary folder, removed afterwards.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use mg_mdl::{MeshExtra, Model};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("{tool} not found")]
    NotFound { tool: &'static str },
    #[error("{0}")]
    Refused(String),
    #[error("{tool}: {message}")]
    Failed { tool: &'static str, message: String },
    #[error("{path}: {message}")]
    Io { path: PathBuf, message: String },
    #[error("timed out after {0:?}")]
    Timeout(Duration),
}

fn io(path: &Path, e: std::io::Error) -> ToolError {
    ToolError::Io { path: path.to_path_buf(), message: e.to_string() }
}

/// A temporary folder, removed when dropped.
#[derive(Debug)]
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Result<TempDir, ToolError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("mgv-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).map_err(|e| io(&p, e))?;
        Ok(TempDir(p))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A program file name on this platform.
fn exe(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

/// Where model tools are looked for, in order: `NWN_TOOLS_BIN`, then
/// `PATH`.
pub fn tool_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(d) = std::env::var_os("NWN_TOOLS_BIN") {
        dirs.push(PathBuf::from(d));
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs
}

/// A program in [`tool_dirs`].
pub fn find_tool(name: &str) -> Option<PathBuf> {
    let file = exe(name);
    tool_dirs().into_iter().map(|d| d.join(&file)).find(|p| p.is_file())
}

/// nwnmdlcomp.
#[derive(Debug, Clone)]
pub struct Nwnmdlcomp {
    pub path: PathBuf,
    /// The game folder, for supermodels in the game's archives (`NWNDIR`).
    pub game_root: Option<PathBuf>,
}

impl Nwnmdlcomp {
    /// nwnmdlcomp from [`tool_dirs`].
    pub fn find(game_root: Option<PathBuf>) -> Option<Nwnmdlcomp> {
        find_tool("nwnmdlcomp").map(|path| Nwnmdlcomp { path, game_root })
    }

    fn run(&self, dir: &Path, mode: &str, file: &str) -> Result<String, ToolError> {
        let mut cmd = Command::new(&self.path);
        cmd.args([mode, file]).current_dir(dir).stdin(Stdio::null());
        if let Some(root) = &self.game_root {
            // It joins paths to NWNDIR by plain concatenation.
            let mut r = root.as_os_str().to_owned();
            r.push(std::path::MAIN_SEPARATOR_STR);
            cmd.env("NWNDIR", r);
        }
        let out = cmd.output().map_err(|e| io(&self.path, e))?;
        let log = String::from_utf8_lossy(&out.stdout).into_owned()
            + &String::from_utf8_lossy(&out.stderr);
        // It exits 0 whatever happens; errors are `Error:` lines.
        let errors: Vec<&str> = log.lines().filter(|l| l.contains("Error:")).collect();
        if !errors.is_empty() {
            return Err(ToolError::Failed { tool: "nwnmdlcomp", message: errors.join("; ") });
        }
        Ok(log)
    }

    /// Decompiles a binary model.
    pub fn decompile(&self, binary: &[u8], name: &str) -> Result<String, ToolError> {
        let tmp = TempDir::new("nwnmdlcomp")?;
        let file = format!("{}.mdl", name.to_ascii_lowercase());
        std::fs::write(tmp.0.join(&file), binary).map_err(|e| io(&tmp.0, e))?;
        self.run(&tmp.0, "-d", &file)?;
        let out = tmp.0.join(format!("{file}.ascii"));
        std::fs::read(&out)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .map_err(|_| ToolError::Failed { tool: "nwnmdlcomp", message: "no output".into() })
    }

    /// Compiles an ASCII model. Custom supermodels (not in the game) are
    /// looked for in `search` folders and staged beside it, as nwnmdlcomp
    /// finds supermodels in its working folder.
    pub fn compile(
        &self,
        ascii: &str,
        name: &str,
        search: &[PathBuf],
    ) -> Result<Vec<u8>, ToolError> {
        check_for_nwnmdlcomp(ascii)?;
        let ascii = &spawntypes_for_nwnmdlcomp(ascii);
        let tmp = TempDir::new("nwnmdlcomp")?;
        let file = format!("{}.mdl", name.to_ascii_lowercase());
        std::fs::write(tmp.0.join(&file), ascii).map_err(|e| io(&tmp.0, e))?;
        stage_supermodels(ascii, search, &tmp.0, 16);
        self.run(&tmp.0, "-c", &file)?;
        let out = tmp.0.join(format!("{file}.mdl"));
        std::fs::read(&out)
            .map_err(|_| ToolError::Failed { tool: "nwnmdlcomp", message: "no output".into() })
    }
}

/// `spawntype` values other than 0 and 1 (EE-compiled models hold −1) as 0,
/// the others as written: nwnmdlcomp refuses them ("Attribute only allows a
/// value of 0 or 1").
fn spawntypes_for_nwnmdlcomp(ascii: &str) -> String {
    let mut out = String::with_capacity(ascii.len());
    for line in ascii.split_inclusive('\n') {
        let mut words = line.split('#').next().unwrap_or("").split_whitespace();
        let other = words.next().is_some_and(|w| w.eq_ignore_ascii_case("spawntype"))
            && words.next().is_some_and(|v| v != "0" && v != "1");
        if other {
            let indent = &line[..line.len() - line.trim_start().len()];
            let end = if line.ends_with("\r\n") {
                "\r\n"
            } else if line.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            out.push_str(&format!("{indent}spawntype 0{end}"));
        } else {
            out.push_str(line);
        }
    }
    out
}

/// The supermodel an ASCII model names (`setsupermodel NAME SUPER`).
pub fn supermodel_of(ascii: &str) -> Option<String> {
    ascii.lines().take(200).find_map(|l| {
        let l = l.split('#').next()?;
        let mut w = l.split_whitespace();
        if !w.next()?.eq_ignore_ascii_case("setsupermodel") {
            return None;
        }
        let sup = w.nth(1)?;
        (!sup.eq_ignore_ascii_case("null")).then(|| sup.to_ascii_lowercase())
    })
}

/// Copies a supermodel chain found in `search` folders into `to`.
fn stage_supermodels(ascii: &str, search: &[PathBuf], to: &Path, depth: usize) {
    let Some(sup) = supermodel_of(ascii) else { return };
    if depth == 0 {
        return;
    }
    let found = search.iter().find_map(|d| {
        std::fs::read_dir(d).ok()?.flatten().map(|e| e.path()).find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(&format!("{sup}.mdl")))
        })
    });
    let Some(path) = found else { return };
    let Ok(data) = std::fs::read(&path) else { return };
    let _ = std::fs::write(to.join(format!("{sup}.mdl")), &data);
    if !mg_mdl::is_binary(&data) {
        stage_supermodels(&String::from_utf8_lossy(&data), search, to, depth - 1);
    }
}

/// EE keywords nwnmdlcomp would drop without a word.
const EE_ONLY: [&str; 2] = ["materialname", "renderhint"];

/// The most bones nwnmdlcomp compiles into one skin.
pub const NWNMDLCOMP_BONES: usize = 17;

/// Refuses what nwnmdlcomp would silently get wrong.
fn check_for_nwnmdlcomp(ascii: &str) -> Result<(), ToolError> {
    let used: Vec<&str> = EE_ONLY
        .into_iter()
        .filter(|k| {
            ascii.lines().any(|l| {
                l.split('#')
                    .next()
                    .and_then(|l| l.split_whitespace().next())
                    .is_some_and(|w| w.eq_ignore_ascii_case(k))
            })
        })
        .collect();
    if !used.is_empty() {
        return Err(ToolError::Refused(format!(
            "uses {}, which nwnmdlcomp would drop; use the game's compiler, or name the \
             material with \"bitmap <mtr name>\" (the game loads an MTR of the texture's name)",
            used.join(" and ")
        )));
    }
    // nwnmdlcomp counts a Bézier key's handles as columns; the game takes
    // the column count as the value's (the toolset's notes_models.md B.8).
    let bezier = ascii
        .lines()
        .filter_map(|l| l.split('#').next()?.split_whitespace().next())
        .find(|w| w.to_ascii_lowercase().ends_with("bezierkey"));
    if let Some(w) = bezier {
        return Err(ToolError::Refused(format!(
            "uses {w}: nwnmdlcomp stores Bézier keys in a layout the game reads differently; \
             use the game's compiler"
        )));
    }
    if let Ok(model) = Model::read(ascii.as_bytes()) {
        for n in &model.nodes {
            if let Some(MeshExtra::Skin(s)) = n.mesh().map(|m| &m.extra)
                && s.bones.len() > NWNMDLCOMP_BONES
            {
                return Err(ToolError::Refused(format!(
                    "skin {} has {} bones; nwnmdlcomp compiles at most {NWNMDLCOMP_BONES}",
                    n.name,
                    s.bones.len()
                )));
            }
        }
    }
    Ok(())
}

/// Whether a model has skin meshes (which the game's compiler cannot
/// compile from the command line).
pub fn has_skin(model: &Model) -> bool {
    model.nodes.iter().any(|n| n.mesh().is_some_and(|m| matches!(m.extra, MeshExtra::Skin(_))))
}

/// The game's own model compiler.
#[derive(Debug, Clone)]
pub struct EngineCompiler {
    /// The game's `nwmain` for this platform.
    pub executable: PathBuf,
    /// A user directory the viewer owns (never the player's).
    pub user_dir: PathBuf,
    pub timeout: Duration,
    /// Run the game through this program instead (with these arguments,
    /// then `compilemodel NAME`), which must give it `user_dir` as its user
    /// directory: a sandbox such as the toolset's
    /// `tools/nwclient/run-client.sh SCRATCH` (user directory
    /// `SCRATCH/user`).
    pub launcher: Option<(PathBuf, Vec<String>)>,
}

impl EngineCompiler {
    /// The compiler of a game install, working in `user_dir`. Refuses the
    /// player's own user directory (`player_dir`).
    pub fn new(
        game_root: &Path,
        user_dir: PathBuf,
        player_dir: Option<&Path>,
    ) -> Result<EngineCompiler, ToolError> {
        if let Some(p) = player_dir
            && same_path(p, &user_dir)
        {
            return Err(ToolError::Refused(
                "the game's compiler runs in a scratch user folder, never the player's".into(),
            ));
        }
        let executable =
            game_executable(game_root).ok_or(ToolError::NotFound { tool: "nwmain" })?;
        Ok(EngineCompiler {
            executable,
            user_dir,
            timeout: Duration::from_secs(60),
            launcher: None,
        })
    }

    /// Compiles an ASCII model named `resref`.
    pub fn compile(&self, ascii: &str, resref: &str) -> Result<Vec<u8>, ToolError> {
        let resref = resref.to_ascii_lowercase();
        if let Ok(m) = Model::read(ascii.as_bytes())
            && has_skin(&m)
        {
            return Err(ToolError::Refused(
                "the game's compiler cannot compile skin meshes from the command line; \
                 use nwnmdlcomp"
                    .into(),
            ));
        }
        let dev = self.user_dir.join("development");
        let out_dir = self.user_dir.join("modelcompiler");
        std::fs::create_dir_all(&dev).map_err(|e| io(&dev, e))?;
        let staged = dev.join(format!("{resref}.mdl"));
        std::fs::write(&staged, ascii).map_err(|e| io(&staged, e))?;
        let output = |dir: &Path| -> Option<PathBuf> {
            std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.eq_ignore_ascii_case(&format!("{resref}.mdl")))
            })
        };
        if let Some(stale) = output(&out_dir) {
            let _ = std::fs::remove_file(stale);
        }
        let started = SystemTime::now();
        let result = self.run(&resref);
        let _ = std::fs::remove_file(&staged);
        let log = result?;
        let lines: Vec<&str> = log.lines().filter(|l| l.contains("aurmodelparse")).collect();
        let success = lines.iter().any(|l| l.contains("Successfully compiled model"));
        let fresh = output(&out_dir).filter(|p| {
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .is_ok_and(|t| t + Duration::from_secs(1) >= started)
        });
        match (success, fresh) {
            (true, Some(p)) => {
                let data = std::fs::read(&p).map_err(|e| io(&p, e))?;
                let _ = std::fs::remove_file(&p);
                Ok(data)
            }
            _ => {
                let details: Vec<&str> =
                    lines.iter().map(|l| l.split("): ").last().unwrap_or(l)).collect();
                Err(ToolError::Failed {
                    tool: "the game's compiler",
                    message: if details.is_empty() {
                        "no output (the game may have crashed)".into()
                    } else {
                        details.join("; ")
                    },
                })
            }
        }
    }

    fn run(&self, resref: &str) -> Result<String, ToolError> {
        let user_dir = self.user_dir.to_string_lossy().into_owned();
        let args = ["-userdirectory", &user_dir, "compilemodel", resref];
        let gamescope = cfg!(target_os = "linux").then(|| find_tool("gamescope")).flatten();
        let mut cmd = match (&self.launcher, &gamescope) {
            (Some((program, extra)), _) => {
                let mut c = Command::new(program);
                c.args(extra).args(["compilemodel", resref]);
                c
            }
            (None, gamescope) => match gamescope {
                Some(g) => {
                    let mut c = Command::new(g);
                    c.args(["--backend", "headless", "-w", "640", "-h", "480", "--"]);
                    c.arg(&self.executable).args(args);
                    // gamescope's own Xwayland, not the desktop's.
                    c.env_remove("WAYLAND_DISPLAY");
                    c
                }
                None => {
                    let mut c = Command::new(&self.executable);
                    c.args(args);
                    c
                }
            },
        };
        if self.launcher.is_none()
            && let Some(dir) = self.executable.parent()
        {
            cmd.current_dir(dir);
        }
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| io(&self.executable, e))?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let reader = std::thread::spawn(move || {
            use std::io::Read;
            let mut s = String::new();
            if let Some(mut o) = stdout {
                let _ = o.read_to_string(&mut s);
            }
            if let Some(mut e) = stderr {
                let _ = e.read_to_string(&mut s);
            }
            s
        });
        let start = Instant::now();
        loop {
            if child.try_wait().map_err(|e| io(&self.executable, e))?.is_some() {
                break;
            }
            if start.elapsed() > self.timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ToolError::Timeout(self.timeout));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Ok(strip_ansi(&reader.join().unwrap_or_default()))
    }
}

/// The game program in an install, for this platform.
pub fn game_executable(root: &Path) -> Option<PathBuf> {
    let candidates: &[&str] = if cfg!(target_os = "windows") {
        &["bin/win32/nwmain.exe"]
    } else if cfg!(target_os = "macos") {
        &["bin/macos/nwmain.app/Contents/MacOS/nwmain"]
    } else if cfg!(target_arch = "aarch64") {
        &["bin/linux-arm64/nwmain-linux"]
    } else {
        &["bin/linux-x86/nwmain-linux"]
    };
    candidates.iter().map(|c| root.join(c)).find(|p| p.is_file())
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Which compiler to use for a model, as Neverblender's `nwn_compile.py`
/// chooses: the game's unless the model has skin meshes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compiler {
    Nwnmdlcomp,
    Engine,
    /// In process ([`crate::compile`]).
    Native,
}

pub fn choose_compiler(model: &Model) -> Compiler {
    if has_skin(model) { Compiler::Nwnmdlcomp } else { Compiler::Engine }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supermodels_and_refusals() {
        let ascii = "# c\nnewmodel x\nsetsupermodel x a_ba\nbeginmodelgeom x\n";
        assert_eq!(supermodel_of(ascii).as_deref(), Some("a_ba"));
        assert_eq!(supermodel_of("setsupermodel x NULL"), None);
        let ee = "newmodel x\nnode trimesh m\n  materialname foo\nendnode\n";
        assert!(matches!(check_for_nwnmdlcomp(ee), Err(ToolError::Refused(_))));
        let commented = "newmodel x\nnode trimesh m\n  # materialname foo\nendnode\n";
        assert!(check_for_nwnmdlcomp(commented).is_ok());
        let bezier = "newanim a x\nnode dummy x\n  positionBezierKey 1\n    0 0 0 0 0 0 0 0 0 0\n";
        assert!(matches!(check_for_nwnmdlcomp(bezier), Err(ToolError::Refused(_))));
    }

    #[test]
    fn spawntypes_nwnmdlcomp_takes() {
        let ascii = "node emitter e\r\n  spawntype -1\r\n  SpawnType 1\n  spawntype 0 # x\nendnode";
        assert_eq!(
            spawntypes_for_nwnmdlcomp(ascii),
            "node emitter e\r\n  spawntype 0\r\n  SpawnType 1\n  spawntype 0 # x\nendnode"
        );
    }

    #[test]
    fn ansi_codes_are_stripped() {
        assert_eq!(strip_ansi("\u{1b}[32mok\u{1b}[0m done"), "ok done");
    }

    #[test]
    fn the_players_folder_is_refused() {
        let dir = std::env::temp_dir();
        let r = EngineCompiler::new(Path::new("/nonexistent"), dir.clone(), Some(&dir));
        assert!(matches!(r, Err(ToolError::Refused(_))));
    }
}
