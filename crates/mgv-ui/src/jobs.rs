//! Work done off the window's thread: external compilers take seconds.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use mgv_mdl::tools::{self, Compiler, EngineCompiler, Nwnmdlcomp};

use crate::settings::CompileWith;

/// What to compile and where to.
#[derive(Debug, Clone)]
pub(crate) struct CompileRequest {
    pub name: String,
    pub text: String,
    /// Folders with custom supermodels.
    pub search: Vec<PathBuf>,
    pub out_dir: PathBuf,
    pub with: CompileWith,
    pub game_root: Option<PathBuf>,
    pub player_dir: Option<PathBuf>,
    pub scratch: PathBuf,
    /// Show the compiled model when done.
    pub view_result: bool,
}

/// A finished job.
#[derive(Debug, Clone)]
pub(crate) enum Done {
    Compiled { name: String, path: PathBuf, with: &'static str, view_result: bool },
    Failed { name: String, message: String },
}

/// Running jobs.
pub struct Jobs {
    tx: Sender<Done>,
    rx: Receiver<Done>,
    running: usize,
}

impl std::fmt::Debug for Jobs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jobs").field("running", &self.running).finish()
    }
}

impl Default for Jobs {
    fn default() -> Jobs {
        let (tx, rx) = channel();
        Jobs { tx, rx, running: 0 }
    }
}

impl Jobs {
    pub(crate) fn busy(&self) -> bool {
        self.running > 0
    }

    /// Jobs finished since the last call.
    pub(crate) fn finished(&mut self) -> Vec<Done> {
        let done: Vec<Done> = self.rx.try_iter().collect();
        self.running -= done.len().min(self.running);
        done
    }

    pub(crate) fn compile(&mut self, r: CompileRequest) {
        self.running += 1;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let name = r.name.clone();
            let done = match compile(&r) {
                Ok((path, with)) => Done::Compiled { name, path, with, view_result: r.view_result },
                Err(message) => Done::Failed { name, message },
            };
            let _ = tx.send(done);
        });
    }
}

fn compile(r: &CompileRequest) -> Result<(PathBuf, &'static str), String> {
    let model = mg_mdl::Model::read(r.text.as_bytes()).map_err(|e| e.to_string())?;
    let compiler = match r.with {
        CompileWith::Auto => tools::choose_compiler(&model),
        CompileWith::Engine => Compiler::Engine,
        CompileWith::Nwnmdlcomp => Compiler::Nwnmdlcomp,
        CompileWith::Native => Compiler::Native,
    };
    let (binary, with) = match compiler {
        Compiler::Nwnmdlcomp => {
            let tool = Nwnmdlcomp::find(r.game_root.clone()).ok_or("nwnmdlcomp not found")?;
            (tool.compile(&r.text, &r.name, &r.search).map_err(|e| e.to_string())?, "nwnmdlcomp")
        }
        Compiler::Engine => {
            let root = r.game_root.as_deref().ok_or("the game's compiler needs the game folder")?;
            let c = EngineCompiler::new(root, r.scratch.clone(), r.player_dir.as_deref())
                .map_err(|e| e.to_string())?;
            (c.compile(&r.text, &r.name).map_err(|e| e.to_string())?, "the game's compiler")
        }
        Compiler::Native => {
            // Models beside the text first, then the game's.
            let game = r.game_root.as_deref().and_then(|root| {
                mg_resman::ResMan::for_game(&mg_resman::GameInstall::new(root, None, "en")).ok()
            });
            let in_game = |n: &str| {
                let key = mg_resman::ResKey::parse(n, mg_core::ResType::MDL)?;
                game.as_ref()?.get(&key).ok().map(|d| d.into_owned())
            };
            let lookup = |n: &str| {
                let file = format!("{}.mdl", n.to_ascii_lowercase());
                r.search
                    .iter()
                    .find_map(|d| std::fs::read(d.join(&file)).ok())
                    .or_else(|| in_game(n))
            };
            let out =
                mgv_mdl::compile::compile_named(r.text.as_bytes(), &r.name, &lookup, &in_game)
                    .map_err(|e| e.to_string())?;
            (out.binary, "Moonglow's compiler")
        }
    };
    std::fs::create_dir_all(&r.out_dir).map_err(|e| format!("{}: {e}", r.out_dir.display()))?;
    let path = r.out_dir.join(format!("{}.mdl", r.name));
    std::fs::write(&path, binary).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((path, with))
}
