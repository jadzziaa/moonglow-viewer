//! The binary writer in the game (the plan's Phase 9, stage A): models it
//! wrote, put in a scratch user folder's `development`, are drawn by the
//! game client as the originals are, and so are models the native compiler
//! made of their decompiled text (stage B). It also measures how the game binds a
//! supermodel's animations: by part number.
//!
//! By hand, on the toolset's off-screen display (never the desktop), with
//! the toolset checked out beside this project for its client sandbox:
//!
//! ```sh
//! DISPLAY=$(~/Projects/moonglow-toolset/tools/aurora/headless.sh start) \
//!   cargo test --release -p mgv-mdl --test client -- --ignored --nocapture
//! ~/Projects/moonglow-toolset/tools/aurora/headless.sh stop   # if you started it
//! ```
//!
//! The pictures are kept in `target/test-output/client/`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use mg_core::{ResRef, ResType};
use mg_gff::Value;
use mg_mdl::Model;
use mg_mdl::binary_write::{PartNumbers, part_numbers, write};
use mg_module::ModuleLocation;
use mg_module::new::{AreaSpec, add_area, new_module};
use mg_resman::{GameInstall, ResKey, ResMan};
use mg_rules::GameData;
use mg_schema::{StructExt, ifo};

/// The scene: an armoire (emitters, animations), a wererat lying on its back
/// (parts on its own skeleton; a pose, so a skin standing in for it is
/// bent by its bones), a man sitting (a dangly-haired model on the `a_ba`
/// supermodel's animations) and a red dragon (skinned wings, on another
/// dragon's animations), seen from above and behind the player's start.
const ENTER: &str = r#"
void Log(string s) { WriteTimestampedLogEntry(s); }
void Still(object o)
{
    ChangeToStandardFaction(o, STANDARD_FACTION_COMMONER);
    ApplyEffectToObject(DURATION_TYPE_PERMANENT, EffectVisualEffect(VFX_DUR_FREEZE_ANIMATION), o);
}
void Sit(object o)
{
    ChangeToStandardFaction(o, STANDARD_FACTION_COMMONER);
    AssignCommand(o, ClearAllActions());
    AssignCommand(o, ActionPlayAnimation(ANIMATION_LOOPING_SIT_CROSS, 1.0, 600.0));
}
void Lie(object o)
{
    ChangeToStandardFaction(o, STANDARD_FACTION_COMMONER);
    AssignCommand(o, ClearAllActions());
    AssignCommand(o, ActionPlayAnimation(ANIMATION_LOOPING_DEAD_BACK, 1.0, 600.0));
}
void main()
{
    object pc = GetEnteringObject();
    if (!GetIsPC(pc)) return;
    // The player is not in the area yet: use the start location.
    location start = GetStartingLocation();
    object area = GetAreaFromLocation(start);
    vector p = GetPositionFromLocation(start);
    CreateObject(OBJECT_TYPE_PLACEABLE, "plc_armoire", Location(area, Vector(p.x - 4.0, p.y + 4.0, p.z), 270.0));
    object rat = CreateObject(OBJECT_TYPE_CREATURE, "nw_wererat", Location(area, Vector(p.x - 1.5, p.y + 4.0, p.z), 270.0));
    DelayCommand(1.0, Lie(rat));
    object man = CreateObject(OBJECT_TYPE_CREATURE, "nw_humanmerc001", Location(area, Vector(p.x + 1.5, p.y + 4.0, p.z), 270.0));
    DelayCommand(1.0, Sit(man));
    Still(CreateObject(OBJECT_TYPE_CREATURE, "nw_drgred001", Location(area, Vector(p.x + 7.0, p.y + 9.0, p.z), 250.0)));
    ApplyEffectToObject(DURATION_TYPE_PERMANENT, EffectVisualEffect(VFX_DUR_CUTSCENE_INVISIBILITY), pc);
    AssignCommand(pc, SetCameraFacing(90.0, 16.0, 50.0, CAMERA_TRANSITION_TYPE_SNAP));
    LockCameraDirection(pc, TRUE);
    LockCameraPitch(pc, TRUE);
    LockCameraDistance(pc, TRUE);
    DelayCommand(8.0, Log("MG_READY"));
}
"#;

/// The client's settings for the scene: no splash, movies or effects that
/// change from one run to the next.
const SETTINGS: &str = r#"[graphics]
	[graphics.fbo]
		[graphics.fbo.hdr-bloom]
			enabled = false
		[graphics.fbo.high-contrast]
			enabled = false
		[graphics.fbo.sharpen]
			enabled = false
		[graphics.fbo.ssao]
			enabled = false
		[graphics.fbo.vibrance]
			enabled = false
	[graphics.grass]
		mode = 0
	[graphics.hilite]
		enabled = false
	[graphics.intro]
		[graphics.intro.splash]
			enabled = false
	[graphics.keyholing]
		enabled = false
	[graphics.lod]
		enabled = false
	[graphics.movies]
		enabled = false
		[graphics.movies.intro]
			enabled = false
	[graphics.shadows]
		[graphics.shadows.creatures]
			mode = 0
		[graphics.shadows.environment]
			enabled = false
	[graphics.skyboxes]
		enabled = false
	[graphics.tile-borders]
		enabled = false
"#;

/// The toolset's checkout, for its client sandbox and its window
/// screenshots (`MOONGLOW_TOOLSET`, or beside this project).
fn toolset() -> Option<PathBuf> {
    let dir = std::env::var_os("MOONGLOW_TOOLSET")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../moonglow-toolset"));
    dir.join("tools/nwclient/run-client.sh").is_file().then_some(dir)
}

fn out_dir() -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-output/client");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Compiles NWScript source with the official compiler.
fn compile(dir: &Path, name: &str, source: &str) -> Vec<u8> {
    let compiler = mg_testkit::nwn_tool("nwn_script_comp").expect("nwn_script_comp");
    let (nss, ncs) = (dir.join(format!("{name}.nss")), dir.join(format!("{name}.ncs")));
    std::fs::write(&nss, source).unwrap();
    let out = Command::new(&compiler)
        .args(["-o", ncs.to_str().unwrap(), nss.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success(), "compile failed: {}", String::from_utf8_lossy(&out.stderr));
    std::fs::read(&ncs).unwrap()
}

/// A scratch user folder with the scene's module and settings.
fn scene(game: &GameData, dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir.join("user/development")).unwrap();
    let mut rng = fastrand::Rng::with_seed(11);
    let mut m = new_module(game, "MgScene", &mut rng).unwrap();
    let spec = AreaSpec {
        name: "Scene".into(),
        tileset: ResRef::from_str("tic01").unwrap(),
        width: 6,
        height: 6,
    };
    let area = add_area(&mut m, game, &spec, &mut rng).unwrap();
    let are_key = ResKey::new(area, ResType::ARE);
    let mut are = m.gff(&are_key).unwrap().unwrap();
    for label in ["SunAmbientColor", "MoonAmbientColor"] {
        are.root.set(label, Value::Dword(0x60_6060));
    }
    for label in ["SunDiffuseColor", "MoonDiffuseColor"] {
        are.root.set(label, Value::Dword(0xC0_C0C0));
    }
    for label in ["SunFogAmount", "MoonFogAmount", "SunShadows", "MoonShadows", "IsNight"] {
        are.root.set(label, Value::Byte(0));
    }
    are.root.set("DayNightCycle", Value::Byte(0));
    m.set_gff(are_key, &are).unwrap();
    m.set(ResKey::parse("mg_enter", ResType::NCS).unwrap(), compile(dir, "mg_enter", ENTER));
    let mut info = m.info().unwrap();
    info.root.write(&ifo::MOD_ON_CLIENT_ENTR, ResRef::from_str("mg_enter").unwrap());
    m.set_info(&info).unwrap();
    std::fs::write(dir.join("user/settings.tml"), SETTINGS).unwrap();
    m.save_as(&ModuleLocation::Archive(dir.join("user/modules/MgScene.mod"))).unwrap();
}

struct Picture {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

fn read_png(path: &Path) -> Picture {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let data = &buf[..info.buffer_size()];
    let data = match info.color_type {
        png::ColorType::Rgb => {
            data.as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect()
        }
        _ => data.to_vec(),
    };
    Picture { width: info.width, height: info.height, data }
}

/// Runs the client on the scene until it is ready and screenshots its
/// window into `shot`.
fn client_screenshot(toolset: &Path, dir: &Path, shot: &Path) -> Option<Picture> {
    // In single player the game's server logs to the client's log.
    let log = dir.join("user/logs/nwclientLog1.txt");
    let _ = std::fs::remove_file(&log);
    let mut child = Command::new(toolset.join("tools/nwclient/run-client.sh"))
        .arg(dir)
        .args(["+TestNewModule", "MgScene"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    let mut ready = false;
    while start.elapsed() < Duration::from_secs(180) {
        if std::fs::read_to_string(&log).is_ok_and(|l| l.contains("MG_READY")) {
            ready = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    std::thread::sleep(Duration::from_secs(2));
    let ok = ready
        && Command::new("python3")
            .arg(toolset.join("tools/aurora/xdrive.py"))
            .args(["winshot", "Neverwinter Nights: Enhanced Edition"])
            .arg(shot)
            .status()
            .is_ok_and(|s| s.success());
    let _ = child.kill();
    let _ = child.wait();
    ok.then(|| read_png(shot))
}

/// The mean difference a channel between two pictures, 0 to 255, over a
/// part of them (fractions: left, top, right, bottom).
fn difference(a: &Picture, b: &Picture, part: [f32; 4]) -> f32 {
    assert_eq!((a.width, a.height), (b.width, b.height), "the window's size changed");
    let (w, h) = (a.width as f32, a.height as f32);
    let (x0, y0) = ((part[0] * w) as u32, (part[1] * h) as u32);
    let (x1, y1) = ((part[2] * w) as u32, (part[3] * h) as u32);
    let (mut sum, mut n) = (0u64, 0u64);
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * a.width + x) * 4) as usize;
            for k in 0..3 {
                sum += u64::from(a.data[i + k].abs_diff(b.data[i + k]));
                n += 1;
            }
        }
    }
    sum as f32 / n.max(1) as f32
}

/// The scene's models, each with its supermodels: (name, file).
fn models(rm: &ResMan) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut todo: Vec<String> =
        ["plc_a01", "c_wererat", "c_ynpc_h_m07", "c_drgred"].map(String::from).to_vec();
    while let Some(name) = todo.pop() {
        if out.iter().any(|(n, _)| *n == name) {
            continue;
        }
        let Ok(data) = rm.get(&ResKey::parse(&name, ResType::MDL).unwrap()) else { continue };
        if !mg_mdl::is_binary(&data) {
            continue;
        }
        if let Some(sup) = Model::read(&data).ok().and_then(|m| m.supermodel) {
            todo.push(sup);
        }
        out.push((name, data.into_owned()));
    }
    out.sort();
    out
}

/// Files by name.
type Files = Vec<(String, Vec<u8>)>;

/// Neverblender's 52-bone test mannequin as the wererat's model: its text
/// under that name and its textures (`NEVERBLENDER`, or the project beside
/// this one; made by its `tools/e2e/run_e2e.py`).
fn neverblender_mannequin() -> Option<(String, Files)> {
    let project = std::env::var_os("NEVERBLENDER")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../neverblender"));
    let dir = project.join("build/fbx/creature");
    let text = std::fs::read_to_string(dir.join("zz_nvbmana.mdl")).ok()?;
    let textures = ["main", "join"]
        .iter()
        .map(|t| {
            let data = std::fs::read(dir.join(format!("zz_nvbmana_{t}.tga"))).ok()?;
            Some((format!("c_wererat_{t}.tga"), data))
        })
        .collect::<Option<Vec<_>>>()?;
    Some((text.replace("zz_nvbmana", "c_wererat"), textures))
}

/// A model's part numbers as a compiler would give them, from its
/// supermodels' given the same way (not read from their files).
fn derived(
    name: &str,
    all: &HashMap<String, Model>,
    done: &mut HashMap<String, PartNumbers>,
) -> PartNumbers {
    if let Some(p) = done.get(name) {
        return p.clone();
    }
    let model = &all[name];
    let sup = model.supermodel.as_deref().filter(|s| all.contains_key(*s)).map(|s| {
        let parts = derived(s, all, done);
        (&all[s], parts)
    });
    let parts = part_numbers(model, sup.as_ref().map(|(m, p)| (*m, p)));
    done.insert(name.to_string(), parts.clone());
    parts
}

#[test]
#[ignore = "runs the game client in the toolset's sandbox, on its off-screen display"]
fn the_client_draws_written_models_as_the_originals() {
    let root = mg_testkit::corpus!();
    let _ = mg_testkit::oracle_tool!("nwn_script_comp");
    let Some(toolset) = toolset() else {
        eprintln!("skipped: no Moonglow Toolset checkout (set MOONGLOW_TOOLSET)");
        return;
    };
    match std::env::var("DISPLAY") {
        Ok(d) if d != ":0" && !d.is_empty() => {}
        _ => {
            eprintln!("skipped: DISPLAY must be the toolset's off-screen display, not the desktop");
            return;
        }
    }
    let install = GameInstall::new(&root, None, "en");
    let game = GameData::open(&install).unwrap();
    let rm = ResMan::for_game(&install).unwrap();
    let files = models(&rm);
    let all: HashMap<String, Model> =
        files.iter().map(|(n, d)| (n.clone(), Model::read(d).unwrap())).collect();
    eprintln!("models: {}", files.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", "));
    let out = out_dir();
    let run = |name: &str, fill: &dyn Fn(&Path)| -> Picture {
        let dir = mg_testkit::scratch_dir(&format!("mgv_client_{name}"));
        scene(&game, &dir);
        fill(&dir.join("user/development"));
        client_screenshot(&toolset, &dir, &out.join(format!("{name}.png"))).unwrap_or_else(|| {
            panic!("{name}: the client did not reach the scene ({})", dir.display())
        })
    };
    // The game's own files, twice: how much two runs differ by themselves.
    let original = run("original", &|_| {});
    let again = run("original-again", &|_| {});
    // Every model written again, with the numbers its file has.
    let rewritten = run("rewritten", &|dev| {
        for (name, data) in &files {
            let parts = PartNumbers::read(data).unwrap();
            std::fs::write(dev.join(format!("{name}.mdl")), write(&all[name], &parts).unwrap())
                .unwrap();
        }
    });
    // Stage B: every model without skin meshes decompiled and compiled
    // again by the native compiler, keeping its numbers.
    let compiled = run("compiled", &|dev| {
        for (name, data) in &files {
            let text = mg_mdl::decompile(data).unwrap();
            let lookup = |n: &str| files.iter().find(|(f, _)| f == n).map(|(_, d)| d.clone());
            match mg_mdl::compile::compile_named(text.as_bytes(), name, &lookup, &lookup) {
                Ok(out) => {
                    eprintln!("compiled {name} natively");
                    std::fs::write(dev.join(format!("{name}.mdl")), out.binary).unwrap();
                }
                Err(e) => eprintln!("left {name} as it is: {e}"),
            }
        }
    });
    // Stage C, more bones than the old game's 17: Neverblender's test
    // mannequin (52 bones a skin, from a CC0 rig; built by its end-to-end
    // run, so only where that is) stands in for the wererat, as the text
    // the game reads itself and compiled natively.
    let mannequin = neverblender_mannequin();
    let many_bones = mannequin.as_ref().map(|(text, textures)| {
        let fill = |dev: &Path, compiled: bool| {
            for (name, data) in textures {
                std::fs::write(dev.join(name), data).unwrap();
            }
            let model = if compiled {
                let none = |_: &str| None;
                mg_mdl::compile::compile_named(text.as_bytes(), "c_wererat", &none, &none)
                    .unwrap()
                    .binary
            } else {
                text.clone().into_bytes()
            };
            std::fs::write(dev.join("c_wererat.mdl"), model).unwrap();
        };
        let as_text = run("bones-text", &|dev| fill(dev, false));
        let again = run("bones-text-again", &|dev| fill(dev, false));
        let natively = run("bones-native", &|dev| fill(dev, true));
        (as_text, again, natively)
    });
    // The supermodels alone written again, numbered as a compiler numbers
    // them from scratch (not as their files are): does the game still
    // find their animations for the models compiled against the old
    // numbers?
    let renumbered = run("renumbered", &|dev| {
        let mut done = HashMap::new();
        let supers: Vec<&String> = all.values().filter_map(|m| m.supermodel.as_ref()).collect();
        for (name, data) in files.iter().filter(|(n, _)| supers.contains(&n)) {
            let parts = derived(name, &all, &mut done);
            let changed = PartNumbers::read(data).unwrap() != parts;
            eprintln!("renumbered {name}: {}", if changed { "numbers differ" } else { "the same" });
            std::fs::write(dev.join(format!("{name}.mdl")), write(&all[name], &parts).unwrap())
                .unwrap();
        }
    });
    // The scene, clear of the game's GUI at the edges; and the sitting man.
    let (part, man) = ([0.15, 0.10, 0.85, 0.80], [0.49, 0.27, 0.60, 0.43]);
    let noise = difference(&original, &again, part);
    let same = difference(&original, &rewritten, part);
    eprintln!(
        "mean difference a channel: two runs of the originals {noise:.3}, rewritten {same:.3}; \
         the man: two runs {:.3}, rewritten {:.3}, his supermodels renumbered {:.3} (pictures \
         in {})",
        difference(&original, &again, man),
        difference(&original, &rewritten, man),
        difference(&original, &renumbered, man),
        out.display()
    );
    assert!(same <= noise * 2.0 + 0.5, "the written models are drawn differently");
    let natively = difference(&original, &compiled, part);
    eprintln!(
        "compiled natively from their text: {natively:.3}; the man {:.3}",
        difference(&original, &compiled, man)
    );
    assert!(natively <= noise * 2.0 + 0.5, "natively compiled models are drawn differently");
    assert!(
        difference(&original, &compiled, man) <= difference(&original, &again, man) * 2.0 + 1.0,
        "the man compiled natively sits differently"
    );
    assert!(
        difference(&original, &rewritten, man) <= difference(&original, &again, man) * 2.0 + 1.0,
        "the man on written supermodels sits differently"
    );
    // The mannequin where the wererat stood.
    match &many_bones {
        Some((as_text, again, natively)) => {
            let rat = [0.36, 0.24, 0.47, 0.43];
            let (noise, compiled) =
                (difference(as_text, again, rat), difference(as_text, natively, rat));
            eprintln!(
                "a skin of 52 bones: two runs of its text {noise:.3}, compiled natively \
                 {compiled:.3}; against the wererat {:.3}",
                difference(&original, as_text, rat)
            );
            assert!(difference(&original, as_text, rat) > 1.0, "the mannequin is not there");
            assert!(compiled <= noise * 2.0 + 1.0, "the compiled skin is drawn differently");
        }
        None => {
            eprintln!("skipped the skin of 52 bones: no Neverblender build beside this project")
        }
    }
    // What this measured (2026-10-07): the game finds a supermodel's
    // animations for a model's nodes by part number, not by name. With his
    // supermodels numbered afresh the man no longer sits as he did: a
    // supermodel compiled again must keep its numbers.
    assert!(
        difference(&original, &renumbered, man) > difference(&original, &again, man) * 2.0 + 1.0,
        "renumbered supermodels drew the same: does the game bind by name now?"
    );
}
