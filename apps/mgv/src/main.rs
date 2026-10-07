//! `mgv`: Moonglow Viewer without a window. Renders models (stills and
//! turntables), reports what a model holds, decompiles, compiles and checks
//! models.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use mg_mdl::{Model, NodeKind};
use mg_resman::GameInstall;
use mgv_library::Library;
use mgv_mdl::tools::{self, Compiler, EngineCompiler, Nwnmdlcomp};
use mgv_stage::camera::View;
use mgv_stage::headless::{self, Shot};
use mgv_stage::{Stage, Viewport};

#[derive(Parser)]
#[command(name = "mgv", version, about = "Moonglow Viewer's command-line tool")]
struct Cli {
    /// The game folder (default: $NWN_ROOT, else where Steam installs it).
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    /// The game's user folder, for override and development (default:
    /// $NWN_HOME or the platform's).
    #[arg(long, global = true)]
    user_dir: Option<PathBuf>,
    /// Leave the user folder out (only the game's own files).
    #[arg(long, global = true)]
    no_user_dir: bool,
    /// Run without the game (only files given on the command line).
    #[arg(long, global = true)]
    no_game: bool,
    /// Haks, modules or ERFs to add above the game (repeatable).
    #[arg(long = "hak", global = true)]
    haks: Vec<PathBuf>,
    /// Folders of loose files to add above the game (repeatable).
    #[arg(long = "folder", global = true)]
    folders: Vec<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Renders a model or blueprint to a PNG.
    Render {
        /// A file, or a resource name (`plc_a01`, `nw_chicken.utc`).
        input: String,
        /// The PNG to write (default: NAME.png).
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[command(flatten)]
        shot: ShotArgs,
    },
    /// Renders a model turning once around, as an animated PNG (or frames).
    Turntable {
        input: String,
        /// The animated PNG to write (default: NAME.png), or a folder for
        /// numbered frames with --frames-only.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// How many frames.
        #[arg(long, default_value_t = 36)]
        frames: usize,
        /// Write numbered PNG frames into the output folder instead.
        #[arg(long)]
        frames_only: bool,
        #[command(flatten)]
        shot: ShotArgs,
    },
    /// Renders many models into a gallery: thumbnails, index.html and
    /// manifest.json. Re-running skips what has not changed.
    Gallery {
        /// What to show: a name pattern (`plc_*`), a 2DA (`placeables`,
        /// `appearance`, `visualeffects`, `doors`), a hak, module or ERF,
        /// or a folder (prefixes `pattern:`, `2da:`, `hak:`, `folder:`
        /// make it explicit).
        source: String,
        /// The folder to write.
        #[arg(short, long)]
        output: PathBuf,
        /// The page's title (default: the source).
        #[arg(long)]
        title: Option<String>,
        /// Render everything again.
        #[arg(long)]
        full: bool,
        /// At most this many items (the first ones).
        #[arg(long)]
        limit: Option<usize>,
        #[command(flatten)]
        shot: ShotArgs,
    },
    /// Renders many models into one picture, a tile each with its name and
    /// size under it (a contact sheet).
    Sheet {
        /// What to show: model files, folders of them, resource names, or
        /// anything `gallery` takes (a name pattern, a 2DA, a hak).
        #[arg(required = true)]
        inputs: Vec<String>,
        /// The PNG to write.
        #[arg(short, long)]
        output: PathBuf,
        /// Tiles in a row (default: as square a sheet as they make).
        #[arg(long)]
        columns: Option<usize>,
        /// Leave the names and sizes out.
        #[arg(long)]
        no_labels: bool,
        /// At most this many tiles (the first ones).
        #[arg(long)]
        limit: Option<usize>,
        /// A tile's size and picture (`--size` defaults to 320x320 here).
        #[command(flatten)]
        shot: ShotArgs,
    },
    /// Prints what a model holds, as JSON.
    Info { input: String },
    /// Decompiles binary models to ASCII.
    Decompile {
        inputs: Vec<String>,
        /// The folder to write into (default: beside each file, or here for
        /// game resources).
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = DecompileWith::Native)]
        with: DecompileWith,
        /// Replace existing files.
        #[arg(long)]
        force: bool,
    },
    /// Compiles ASCII models to binary.
    Compile {
        inputs: Vec<String>,
        /// The folder to write into (default: `compiled/` beside each file).
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = CompileWith::Auto)]
        with: CompileWith,
        /// The scratch user folder for the game's compiler (default: one
        /// in the system's temporary folder).
        #[arg(long)]
        scratch: Option<PathBuf>,
        #[arg(long)]
        force: bool,
    },
    /// Checks ASCII models and prints what is wrong, by line.
    Lint {
        inputs: Vec<String>,
        /// Also print notes (keywords nwnmdlcomp drops, …).
        #[arg(long)]
        notes: bool,
        /// Fail on warnings too, not only on errors.
        #[arg(long)]
        strict: bool,
    },
}

#[derive(Args, Clone)]
struct ShotArgs {
    /// Picture size, WIDTHxHEIGHT.
    #[arg(long, default_value = "512x512")]
    size: String,
    /// The animation to play (default: what the game plays for the model).
    #[arg(long)]
    anim: Option<String>,
    /// Seconds into it.
    #[arg(long, default_value_t = 0.0)]
    time: f32,
    /// Steps per second.
    #[arg(long, default_value_t = 30.0)]
    fps: f32,
    /// The side to look from, relative to the model's front (creatures
    /// face +Y, placeables −Y).
    #[arg(long, value_enum, default_value_t = ViewArg::ThreeQuarter)]
    view: ViewArg,
    /// Camera angle around the model, degrees from +X whichever way it
    /// faces (90: from +Y; -90: from −Y; overrides --view).
    #[arg(long)]
    yaw: Option<f32>,
    /// Camera angle above the ground, degrees.
    #[arg(long)]
    pitch: Option<f32>,
    /// Camera distance as a multiple of the framed distance.
    #[arg(long, default_value_t = 1.0)]
    zoom: f32,
    /// Background color, R,G,B in 0–1 (gamma space).
    #[arg(long)]
    background: Option<String>,
    /// Leave the model's own lights off.
    #[arg(long)]
    no_model_lights: bool,
    /// Visual effects to apply (`visualeffects.2da` rows; repeatable).
    #[arg(long = "vfx")]
    vfx: Vec<usize>,
    /// The light: `studio`, or `env:ROW` / `env:ROW:night` (an
    /// `environment.2da` preset, as an area's light).
    #[arg(long, default_value = "studio")]
    light: String,
    /// The area light's fog.
    #[arg(long)]
    fog: bool,
    /// A light at the camera, so the sides in view are lit whatever the
    /// sun's direction: strength 0 (none) to 1; about 0.3 matches the
    /// studio sun.
    #[arg(long, default_value_t = 0.0)]
    key_light: f32,
    /// PLT colors for a model on its own (a body part, an animation
    /// base): `LAYER=ROW,…` with the layers skin, hair, metal1, metal2,
    /// cloth1, cloth2, leather1, leather2, tattoo1, tattoo2 and palette
    /// rows 0 to 175 (the rest 0), or ten rows in that order. Creatures
    /// and blueprints keep their own.
    #[arg(long)]
    plt_colors: Option<String>,
    /// Show the shadow casters (meshes with `render 0` and `shadow 1`) in
    /// blue in place of the visible meshes.
    #[arg(long)]
    casters: bool,
}

/// `--plt-colors`: `LAYER=ROW,…` or ten rows.
fn plt_colors(spec: &str) -> Result<[u8; 10]> {
    const LAYERS: [&str; 10] = [
        "skin", "hair", "metal1", "metal2", "cloth1", "cloth2", "leather1", "leather2", "tattoo1",
        "tattoo2",
    ];
    let row = |s: &str| {
        s.trim()
            .parse::<u8>()
            .ok()
            .filter(|r| *r <= 175)
            .ok_or_else(|| anyhow!("--plt-colors: {s}: a palette row, 0 to 175"))
    };
    let mut colors = [0; 10];
    let parts: Vec<&str> = spec.split(',').collect();
    if parts.iter().all(|p| !p.contains('=')) {
        if parts.len() != 10 {
            bail!("--plt-colors: ten rows ({}), or LAYER=ROW,…", LAYERS.join(", "));
        }
        for (c, p) in colors.iter_mut().zip(&parts) {
            *c = row(p)?;
        }
        return Ok(colors);
    }
    for p in parts {
        let (layer, value) =
            p.split_once('=').ok_or_else(|| anyhow!("--plt-colors: {p}: LAYER=ROW"))?;
        let layer = layer.trim().to_ascii_lowercase();
        let i = LAYERS.iter().position(|l| *l == layer).ok_or_else(|| {
            anyhow!("--plt-colors: no layer {layer} (there are: {})", LAYERS.join(", "))
        })?;
        colors[i] = row(value)?;
    }
    Ok(colors)
}

#[derive(Clone, Copy, ValueEnum)]
enum ViewArg {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
    ThreeQuarter,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum DecompileWith {
    Native,
    Nwnmdlcomp,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CompileWith {
    /// The game's compiler, unless the model has skin meshes (nwnmdlcomp).
    Auto,
    Nwnmdlcomp,
    Engine,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if e.downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::BrokenPipe)
            {
                return ExitCode::SUCCESS;
            }
            eprintln!("mgv: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn install(cli: &Cli) -> Option<GameInstall> {
    if cli.no_game {
        return None;
    }
    let mut install = match &cli.root {
        Some(root) => GameInstall::new(root, None, "en"),
        None => GameInstall::detect()?,
    };
    if cli.no_user_dir {
        install.user_dir = None;
    } else if let Some(u) = &cli.user_dir {
        install.user_dir = Some(u.clone());
    }
    Some(install)
}

fn library(cli: &Cli) -> Result<Library> {
    let install = install(cli);
    if let Some(i) = &install
        && !GameInstall::is_install(&i.root)
    {
        bail!("{}: not a game folder (pass --root or set NWN_ROOT)", i.root.display());
    }
    let mut lib = Library::open(install).context("opening the game's resources")?;
    for h in &cli.haks {
        lib.add_archive(h)?;
    }
    for f in &cli.folders {
        lib.add_folder(f);
    }
    Ok(lib)
}

fn run(cli: Cli) -> Result<()> {
    match &cli.cmd {
        Cmd::Render { input, output, shot } => {
            let mut lib = library(&cli)?;
            let (mut stage, mut vp, name) = staged(&mut lib, input, shot)?;
            let img = headless::still(&mut stage, &mut vp, &lib, &shot.to_shot()?);
            let out = output.clone().unwrap_or_else(|| PathBuf::from(format!("{name}.png")));
            mgv_stage::render::save_png(&img, &out)
                .with_context(|| format!("writing {}", out.display()))?;
            eprintln!("{}", out.display());
        }
        Cmd::Turntable { input, output, frames, frames_only, shot } => {
            let mut lib = library(&cli)?;
            let (mut stage, mut vp, name) = staged(&mut lib, input, shot)?;
            let s = shot.to_shot()?;
            let images = headless::turntable(&mut stage, &mut vp, &lib, &s, *frames);
            if *frames_only {
                let dir = output.clone().unwrap_or_else(|| PathBuf::from(&name));
                std::fs::create_dir_all(&dir)?;
                for (i, img) in images.iter().enumerate() {
                    let p = dir.join(format!("{name}_{i:03}.png"));
                    mgv_stage::render::save_png(img, &p)?;
                }
                eprintln!("{}", dir.display());
            } else {
                let out = output.clone().unwrap_or_else(|| PathBuf::from(format!("{name}.png")));
                std::fs::write(&out, headless::apng(&images, s.fps))?;
                eprintln!("{}", out.display());
            }
        }
        Cmd::Gallery { source, output, title, full, limit, shot } => {
            let mut lib = library(&cli)?;
            let src = mgv_gallery::Source::parse(source)?;
            let mut items = mgv_gallery::select(&mut lib, &src)?;
            if let Some(n) = limit {
                items.truncate(*n);
            }
            if items.is_empty() {
                bail!("{source}: nothing to show");
            }
            let gpu = mg_render::Gpu::headless().ok_or_else(|| anyhow!("no GPU adapter found"))?;
            let opts = mgv_gallery::Options {
                shot: shot.to_shot()?,
                out: output.clone(),
                title: title.clone().unwrap_or_else(|| source.clone()),
                incremental: !*full,
            };
            let start = std::time::Instant::now();
            let entries = mgv_gallery::run(&mut lib, &gpu, &items, &opts, &mut |i, n, item| {
                eprint!(
                    "\r[{}/{n}] {:<40}",
                    i + 1,
                    item.label.chars().take(40).collect::<String>()
                );
            })?;
            eprintln!();
            let rendered = entries.iter().filter(|e| e.rendered && e.error.is_none()).count();
            let kept = entries.iter().filter(|e| !e.rendered).count();
            let failed: Vec<&mgv_gallery::Entry> =
                entries.iter().filter(|e| e.error.is_some()).collect();
            for e in &failed {
                eprintln!("mgv: {}: {}", e.item.label, e.error.as_deref().unwrap_or_default());
            }
            eprintln!(
                "{} pictures ({rendered} rendered, {kept} unchanged, {} failed) in {:.1} s: {}",
                entries.len(),
                failed.len(),
                start.elapsed().as_secs_f32(),
                output.join("index.html").display()
            );
        }
        Cmd::Sheet { inputs, output, columns, no_labels, limit, shot } => {
            let mut lib = library(&cli)?;
            let mut items = Vec::new();
            for input in inputs {
                let path = Path::new(input);
                let model = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("mdl"));
                if path.is_file() && model {
                    let stem = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    items.push(mgv_gallery::Item {
                        id: stem.clone(),
                        label: stem,
                        group: None,
                        what: mgv_gallery::What::File(path.to_path_buf()),
                    });
                    continue;
                }
                let found = mgv_gallery::select(&mut lib, &mgv_gallery::Source::parse(input)?)?;
                if found.is_empty() {
                    bail!("{input}: nothing to show");
                }
                items.extend(found);
            }
            if let Some(n) = limit {
                items.truncate(*n);
            }
            let gpu = mg_render::Gpu::headless().ok_or_else(|| anyhow!("no GPU adapter found"))?;
            let mut shot_args = shot.clone();
            // (The option's own default is a single picture's.)
            if shot_args.size == "512x512" {
                shot_args.size = "320x320".into();
            }
            let opts = mgv_gallery::sheet::Options {
                shot: shot_args.to_shot()?,
                columns: columns.unwrap_or(0),
                labels: !*no_labels,
            };
            let sheet =
                mgv_gallery::sheet::render(&mut lib, &gpu, &items, &opts, &mut |i, n, item| {
                    eprint!(
                        "\r[{}/{n}] {:<40}",
                        i + 1,
                        item.label.chars().take(40).collect::<String>()
                    );
                });
            eprintln!();
            for (label, e) in &sheet.failed {
                eprintln!("mgv: {label}: {e}");
            }
            mgv_stage::render::save_png(&sheet.image, output)
                .with_context(|| output.display().to_string())?;
            eprintln!(
                "{} tiles ({} failed): {}",
                items.len(),
                sheet.failed.len(),
                output.display()
            );
            if sheet.failed.len() == items.len() {
                bail!("nothing could be shown");
            }
        }
        Cmd::Info { input } => {
            let mut lib = library(&cli)?;
            let opened = lib.open_input(input)?;
            let model = Model::read(&opened.data).map_err(|e| anyhow!("{input}: {e}"))?;
            let info = info(&lib, &opened.name(), &model, &opened.data);
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        Cmd::Decompile { inputs, output, with, force } => {
            let mut lib = library(&cli)?;
            let tool = (*with == DecompileWith::Nwnmdlcomp)
                .then(|| Nwnmdlcomp::find(lib.install().map(|i| i.root.clone())))
                .map(|t| t.ok_or_else(|| anyhow!("nwnmdlcomp not found")))
                .transpose()?;
            for input in inputs {
                let opened = lib.open_input(input)?;
                let name = opened.name();
                let text = match &tool {
                    None => {
                        mgv_mdl::decompile(&opened.data).map_err(|e| anyhow!("{input}: {e}"))?
                    }
                    Some(t) => t.decompile(&opened.data, &name)?,
                };
                let dir = out_dir(output.as_deref(), opened.path.as_deref(), None);
                let out = dir.join(format!("{name}.mdl"));
                if opened.path.as_deref().is_some_and(|p| same_file(p, &out)) {
                    bail!("{}: would replace the binary it was read from; pass -o", out.display());
                }
                write_new(&out, text.as_bytes(), *force)?;
                eprintln!("{}", out.display());
            }
        }
        Cmd::Compile { inputs, output, with, scratch, force } => {
            let mut lib = library(&cli)?;
            let root = lib.install().map(|i| i.root.clone());
            let player = lib.install().and_then(|i| i.user_dir.clone());
            for input in inputs {
                let opened = lib.open_input(input)?;
                let name = opened.name();
                if mg_mdl::is_binary(&opened.data) {
                    bail!("{input}: already compiled");
                }
                let text = String::from_utf8_lossy(&opened.data).into_owned();
                let model = Model::read(&opened.data).map_err(|e| anyhow!("{input}: {e}"))?;
                let compiler = match with {
                    CompileWith::Auto => tools::choose_compiler(&model),
                    CompileWith::Nwnmdlcomp => Compiler::Nwnmdlcomp,
                    CompileWith::Engine => Compiler::Engine,
                };
                let search: Vec<PathBuf> = opened
                    .path
                    .as_deref()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
                    .into_iter()
                    .collect();
                let binary = match compiler {
                    Compiler::Nwnmdlcomp => Nwnmdlcomp::find(root.clone())
                        .ok_or_else(|| anyhow!("nwnmdlcomp not found"))?
                        .compile(&text, &name, &search)?,
                    Compiler::Engine => {
                        let root = root
                            .as_deref()
                            .ok_or_else(|| anyhow!("the game's compiler needs the game"))?;
                        let dir = scratch.clone().unwrap_or_else(|| {
                            std::env::temp_dir().join(format!("mgv-engine-{}", std::process::id()))
                        });
                        let c = EngineCompiler::new(root, dir, player.as_deref())?;
                        c.compile(&text, &name)?
                    }
                };
                let dir = out_dir(output.as_deref(), opened.path.as_deref(), Some("compiled"));
                let out = dir.join(format!("{name}.mdl"));
                write_new(&out, &binary, *force)?;
                eprintln!("{}", out.display());
            }
        }
        Cmd::Lint { inputs, notes, strict } => {
            let mut lib = library(&cli)?;
            let mut worst = mgv_mdl::Severity::Info;
            for input in inputs {
                let opened = lib.open_input(input)?;
                if mg_mdl::is_binary(&opened.data) {
                    eprintln!("{input}: compiled; nothing to check");
                    continue;
                }
                let text = String::from_utf8_lossy(&opened.data);
                for d in mgv_mdl::lint::check(&text) {
                    if d.severity == mgv_mdl::Severity::Info && !notes {
                        continue;
                    }
                    worst = worst.max(d.severity);
                    let sev = format!("{:?}", d.severity).to_lowercase();
                    println!("{input}:{}: {sev}: {}", d.line + 1, d.message);
                }
            }
            if worst == mgv_mdl::Severity::Error {
                bail!("errors found");
            }
            if *strict && worst == mgv_mdl::Severity::Warning {
                bail!("warnings found");
            }
        }
    }
    Ok(())
}

impl ShotArgs {
    fn to_shot(&self) -> Result<Shot> {
        let (w, h) = self
            .size
            .split_once(['x', 'X'])
            .and_then(|(w, h)| Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?)))
            .filter(|(w, h)| (1..=8192).contains(w) && (1..=8192).contains(h))
            .ok_or_else(|| anyhow!("--size: WIDTHxHEIGHT, each 1 to 8192"))?;
        let background = match &self.background {
            Some(s) => {
                let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                let [r, g, b] = v[..] else { bail!("--background: R,G,B") };
                Some([r, g, b])
            }
            None => None,
        };
        Ok(Shot {
            size: (w, h),
            time: self.time.max(0.0),
            fps: self.fps.clamp(1.0, 240.0),
            view: match self.view {
                ViewArg::Front => View::Front,
                ViewArg::Back => View::Back,
                ViewArg::Left => View::Left,
                ViewArg::Right => View::Right,
                ViewArg::Top => View::Top,
                ViewArg::Bottom => View::Bottom,
                ViewArg::ThreeQuarter => View::ThreeQuarter,
            },
            yaw: self.yaw,
            pitch: self.pitch,
            zoom: self.zoom,
            background,
            key_light: self.key_light.clamp(0.0, 1.0),
            plt_colors: self.plt_colors.as_deref().map(plt_colors).transpose()?,
            casters: self.casters,
        })
    }
}

/// Opens an input and puts it on a stage with a GPU: a file, a resource,
/// or `appearance:ROW` (a creature by `appearance.2da` row).
fn staged(lib: &mut Library, input: &str, shot: &ShotArgs) -> Result<(Stage, Viewport, String)> {
    let gpu = mg_render::Gpu::headless().ok_or_else(|| anyhow!("no GPU adapter found"))?;
    let mut stage = Stage::new(gpu.clone());
    let (shown, name, size) = match input.strip_prefix("appearance:") {
        Some(row) => {
            let row: u16 = row.parse().map_err(|_| anyhow!("appearance:ROW takes a row number"))?;
            let look = mgv_stage::subject::CreatureLook::new(row);
            let shown = mgv_stage::subject::show_creature(&mut stage, lib, &look)?;
            (shown, format!("appearance_{row}"), mgv_stage::vfx::size_of(lib, row.into()))
        }
        None => {
            let opened = lib.open_input(input)?;
            (mgv_stage::subject::show(&mut stage, lib, &opened)?, opened.name(), 3)
        }
    };
    for row in &shot.vfx {
        let effect = mgv_stage::vfx::row(lib, *row)
            .ok_or_else(|| anyhow!("visualeffects.2da has no row {row}"))?;
        let applied = mgv_stage::vfx::apply(&mut stage, lib, shown.base, &effect, size)?;
        for m in &applied.missing {
            eprintln!("mgv: {}: model {m} not found", effect.label);
        }
    }
    for m in &shown.missing {
        eprintln!("mgv: {input}: model {m} not found");
    }
    if let Some(anim) = &shot.anim {
        let a = stage.actor_mut(shown.base).expect("just added");
        if a.animations.find(anim).is_none() {
            let names: Vec<&str> = a.animations.names().collect();
            bail!("{input}: no animation {anim} (it has: {})", names.join(", "));
        }
        a.player.play(Some(anim), mgv_stage::PlayMode::Loop);
    }
    stage.model_lights = !shot.no_model_lights;
    stage.lighting = lighting(lib, &shot.light, shot.fog)?;
    let vp = Viewport::new(&gpu);
    Ok((stage, vp, name))
}

/// `studio` or `env:ROW[:night]`.
fn lighting(lib: &Library, spec: &str, fog: bool) -> Result<mgv_stage::Lighting> {
    if spec == "studio" {
        return Ok(mgv_stage::Lighting::studio());
    }
    let rest =
        spec.strip_prefix("env:").ok_or_else(|| anyhow!("--light: studio or env:ROW[:night]"))?;
    let (row, night) = match rest.strip_suffix(":night") {
        Some(r) => (r, true),
        None => (rest.strip_suffix(":day").unwrap_or(rest), false),
    };
    let envs = mgv_stage::lighting::environments(lib);
    let e = match row.parse::<usize>() {
        Ok(r) => envs.iter().find(|e| e.row == r),
        Err(_) => envs.iter().find(|e| e.label.eq_ignore_ascii_case(row)),
    }
    .ok_or_else(|| {
        let names: Vec<String> = envs.iter().map(|e| format!("{} {}", e.row, e.label)).collect();
        anyhow!("--light: no environment {row} (there are: {})", names.join(", "))
    })?;
    let settings = if night { e.night } else { e.day };
    Ok(mgv_stage::Lighting::area(&mgv_stage::lighting::AreaSettings { fog, ..settings }, lib))
}

/// Where outputs go: the given folder, else beside the input (in `sub`),
/// else the current folder.
fn out_dir(output: Option<&Path>, input: Option<&Path>, sub: Option<&str>) -> PathBuf {
    match (output, input.and_then(Path::parent)) {
        (Some(o), _) => o.to_path_buf(),
        (None, Some(p)) => sub.map_or_else(|| p.to_path_buf(), |s| p.join(s)),
        (None, None) => PathBuf::from("."),
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Writes a file, refusing to replace one unless `force`.
fn write_new(path: &Path, data: &[u8], force: bool) -> Result<()> {
    if path.exists() && !force {
        bail!("{}: exists (pass --force to replace it)", path.display());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, data).with_context(|| format!("writing {}", path.display()))
}

/// What a model holds.
fn info(lib: &Library, name: &str, m: &Model, data: &[u8]) -> serde_json::Value {
    use serde_json::json;
    let mut types = std::collections::BTreeMap::<&str, usize>::new();
    let (mut faces, mut vertices) = (0, 0);
    let mut textures = std::collections::BTreeSet::new();
    for n in &m.nodes {
        *types.entry(n.kind.type_name()).or_default() += 1;
        if let NodeKind::Mesh(mesh) = &n.kind {
            faces += mesh.faces.len();
            vertices += mesh.vertices.len();
            for t in mesh.textures.iter().flatten() {
                textures.insert(t.clone());
            }
            if let Some(mat) = &mesh.material {
                textures.insert(mat.clone());
            }
        }
        if let NodeKind::Emitter(e) = &n.kind
            && let Some(t) = &e.texture
        {
            textures.insert(t.clone());
        }
    }
    let animations: Vec<serde_json::Value> = m
        .animations
        .iter()
        .map(|a| json!({ "name": a.name, "length": a.length, "events": a.events.len() }))
        .collect();
    let inherited: Vec<String> =
        mg_render::anim::animations(&std::sync::Arc::new(m.clone()), &|n| lib.model(n))
            .into_iter()
            .map(|(n, _)| n)
            .filter(|n| !m.animations.iter().any(|a| a.name.eq_ignore_ascii_case(n)))
            .collect();
    let missing: Vec<&String> =
        textures.iter().filter(|t| mg_render::Assets::texture(lib.resman(), t).is_none()).collect();
    json!({
        "name": name,
        "internal_name": m.name,
        "format": if mg_mdl::is_binary(data) { "binary" } else { "ascii" },
        "classification": format!("{:?}", m.classification).to_lowercase(),
        "supermodel": m.supermodel,
        "animation_scale": m.animation_scale,
        "nodes": m.nodes.len(),
        "node_types": types,
        "faces": faces,
        "vertices": vertices,
        "textures": textures,
        "missing_textures": missing,
        "animations": animations,
        "inherited_animations": inherited,
    })
}

#[cfg(test)]
mod tests {
    use super::plt_colors;

    #[test]
    fn plt_colors_by_layer_or_in_order() {
        assert_eq!(plt_colors("metal1=40, Leather1=12").unwrap(), [0, 0, 40, 0, 0, 0, 12, 0, 0, 0]);
        assert_eq!(plt_colors("1,2,3,4,5,6,7,8,9,175").unwrap(), [1, 2, 3, 4, 5, 6, 7, 8, 9, 175]);
        for bad in ["1,2,3", "metal1=176", "steel=1", "metal1", "skin=1,2"] {
            assert!(plt_colors(bad).is_err(), "{bad}");
        }
    }
}
