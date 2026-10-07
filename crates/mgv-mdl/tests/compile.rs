//! The binary writer against the game's models: every compiled model, read
//! and written again, reads back the same; and what the writer derives
//! (planes, neighbours, bounds, part numbers) is what the game's files
//! hold.

mod common;

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use mg_core::{ResRef, ResType};
use mg_mdl::Model;
use mg_resman::{GameInstall, ResKey, ResMan};
use mgv_mdl::binary::{PartNumbers, part_numbers, write};
use rayon::prelude::*;

/// A model's part numbers, with its supermodels' (read from the game).
fn numbered(
    rm: &ResMan,
    model: &Model,
    cache: &Mutex<HashMap<String, Option<(Model, PartNumbers)>>>,
    depth: usize,
) -> PartNumbers {
    let sup = model.supermodel.as_deref().filter(|_| depth < 16).and_then(|name| {
        if let Some(hit) = cache.lock().unwrap().get(name) {
            return hit.clone();
        }
        let loaded = ResRef::from_str(name)
            .ok()
            .and_then(|r| rm.get(&ResKey::new(r, ResType::MDL)).ok())
            .and_then(|d| Some((Model::read(&d).ok()?, d)))
            .map(|(m, d)| {
                // A compiled supermodel's own numbers; an ASCII one's as
                // a compiler would give them.
                let parts =
                    PartNumbers::read(&d).unwrap_or_else(|| numbered(rm, &m, cache, depth + 1));
                (m, parts)
            });
        cache.lock().unwrap().insert(name.to_string(), loaded.clone());
        loaded
    });
    part_numbers(model, sup.as_ref().map(|(m, p)| (m, p)))
}

fn u32_at(d: &[u8], at: usize) -> u32 {
    d.get(at..at + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn f32_at(d: &[u8], at: usize) -> f32 {
    f32::from_bits(u32_at(d, at))
}
fn i16_at(d: &[u8], at: usize) -> i16 {
    d.get(at..at + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]))
}

/// The offsets of a compiled model's nodes (from byte 12), in pre-order.
fn node_offsets(d: &[u8]) -> Vec<usize> {
    let m = &d[12..];
    let mut out = Vec::new();
    let mut stack = vec![u32_at(m, 0x48) as usize];
    while let Some(at) = stack.pop() {
        if out.len() > 100_000 {
            break;
        }
        out.push(at);
        let (list, n) = (u32_at(m, at + 0x48) as usize, u32_at(m, at + 0x4C) as usize);
        for i in (0..n).rev() {
            stack.push(u32_at(m, list + 4 * i) as usize);
        }
    }
    out
}

#[derive(Default)]
struct Tally {
    same: AtomicUsize,
    all: AtomicUsize,
}

impl Tally {
    fn add(&self, same: bool) {
        self.all.fetch_add(1, Ordering::Relaxed);
        if same {
            self.same.fetch_add(1, Ordering::Relaxed);
        }
    }
    fn share(&self) -> f64 {
        let all = self.all.load(Ordering::Relaxed);
        if all == 0 { 1.0 } else { self.same.load(Ordering::Relaxed) as f64 / all as f64 }
    }
    fn text(&self) -> String {
        format!(
            "{} of {} ({:.2}%)",
            self.same.load(Ordering::Relaxed),
            self.all.load(Ordering::Relaxed),
            self.share() * 100.0
        )
    }
}

fn near(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps * (1.0 + a.abs().max(b.abs()))
}

#[test]
fn every_binary_model_writes_back_the_same() {
    let root = mg_testkit::corpus!();
    let rm = ResMan::for_game(&GameInstall::new(root, None, "en")).unwrap();
    let names = rm.list(ResType::MDL);
    let cache = Mutex::new(HashMap::new());
    let written = AtomicUsize::new(0);
    // What the writer derives, against the files BioWare's compiler and
    // nwnmdlcomp wrote (the game's own leaves garbage in places).
    let (parts, counts) = (Tally::default(), Tally::default());
    let (normals, distances, neighbours) = (Tally::default(), Tally::default(), Tally::default());
    let (boxes, radii, averages) = (Tally::default(), Tally::default(), Tally::default());
    let mut failures: Vec<(String, String)> = names
        .par_iter()
        .filter_map(|name| {
            let data = rm.get(&ResKey::new(*name, ResType::MDL)).ok()?;
            if !mg_mdl::is_binary(&data) {
                return None;
            }
            let model = Model::read(&data).ok()?;
            let numbers = numbered(&rm, &model, &cache, 0);
            let bytes = match write(&model, &numbers) {
                Ok(b) => b,
                Err(e) => return Some((name.to_string(), format!("not written: {e}"))),
            };
            written.fetch_add(1, Ordering::Relaxed);
            let back = match Model::read(&bytes) {
                Ok(m) => m,
                Err(e) => return Some((name.to_string(), format!("unreadable: {e}"))),
            };
            // (Compared as text: a NaN is not equal to itself.)
            if back != model && format!("{back:?}") != format!("{model:?}") {
                let node = model
                    .nodes
                    .iter()
                    .zip(&back.nodes)
                    .find(|(a, b)| format!("{a:?}") != format!("{b:?}"))
                    .map(|(a, _)| format!("node {} ({})", a.name, a.kind.type_name()));
                let anim = model
                    .animations
                    .iter()
                    .zip(&back.animations)
                    .find(|(a, b)| format!("{a:?}") != format!("{b:?}"))
                    .map(|(a, _)| format!("animation {}", a.name));
                let what = node.or(anim).unwrap_or_else(|| "the header".into());
                return Some((name.to_string(), format!("differs: {what}")));
            }
            // Derived values against the original's, where a compiler that
            // wrote them wrote the file.
            let routine = u32_at(&data, 12);
            if routine != 0x0040_BBC0 && routine != 0x0046_AB0C {
                return None;
            }
            let (m, mine) = (&data[12..], &bytes[12..]);
            let (theirs, ours) = (node_offsets(&data), node_offsets(&bytes));
            if theirs.len() != ours.len() {
                return None;
            }
            counts.add(u32_at(m, 0x4C) == u32_at(mine, 0x4C));
            for (i, (&a, &b)) in theirs.iter().zip(&ours).enumerate() {
                parts.add(u32_at(m, a + 0x1C) == u32_at(mine, b + 0x1C));
                let Some(mesh) = model.nodes[i].mesh() else { continue };
                if mesh.vertices.is_empty() {
                    continue;
                }
                let v3 =
                    |d: &[u8], at: usize| [f32_at(d, at), f32_at(d, at + 4), f32_at(d, at + 8)];
                let same3 =
                    |x: [f32; 3], y: [f32; 3], eps: f32| (0..3).all(|k| near(x[k], y[k], eps));
                boxes.add(
                    same3(v3(m, a + 0x84), v3(mine, b + 0x84), 1e-4)
                        && same3(v3(m, a + 0x90), v3(mine, b + 0x90), 1e-4),
                );
                radii.add(near(f32_at(m, a + 0x9C), f32_at(mine, b + 0x9C), 1e-3));
                averages.add(same3(v3(m, a + 0xA0), v3(mine, b + 0xA0), 1e-3));
                let (fa, fb) = (u32_at(m, a + 0x78) as usize, u32_at(mine, b + 0x78) as usize);
                for f in 0..mesh.faces.len() {
                    let (x, y) = (fa + f * 32, fb + f * 32);
                    normals.add(same3(v3(m, x), v3(mine, y), 1e-3));
                    distances.add(near(f32_at(m, x + 12), f32_at(mine, y + 12), 1e-3));
                    for e in 0..3 {
                        neighbours
                            .add(i16_at(m, x + 0x14 + 2 * e) == i16_at(mine, y + 0x14 + 2 * e));
                    }
                }
            }
            None
        })
        .collect();
    failures.sort();
    eprintln!(
        "{} models written; as the originals: part numbers {}, node counts {}, face normals {}, \
         plane distances {}, neighbours {}, boxes {}, radii {}, averages {}",
        written.load(Ordering::Relaxed),
        parts.text(),
        counts.text(),
        normals.text(),
        distances.text(),
        neighbours.text(),
        boxes.text(),
        radii.text(),
        averages.text()
    );
    for (name, what) in failures.iter().take(40) {
        eprintln!("  {name}: {what}");
    }
    assert!(failures.is_empty(), "{} models differ", failures.len());
    assert!(written.load(Ordering::Relaxed) > 25_000);
    // Part numbers and node counts fall short where the original's nodes
    // were numbered in their file's order (a compiled model's tree does
    // not keep it): then they are a permutation of these.
    for (what, tally, least) in [
        ("part numbers", &parts, 0.79),
        ("node counts", &counts, 0.98),
        ("face normals", &normals, 0.998),
        ("plane distances", &distances, 0.998),
        ("neighbours", &neighbours, 0.99),
        ("boxes", &boxes, 0.995),
        ("radii", &radii, 0.94),
        ("averages", &averages, 0.94),
    ] {
        assert!(tally.share() >= least, "{what}: {}", tally.text());
    }
}

/// nwnmdlcomp reads what the writer wrote as it reads the original: its
/// decompiler walks every structure of a compiled model.
#[test]
fn nwnmdlcomp_reads_what_the_writer_wrote() {
    let root = mg_testkit::corpus!();
    let tool = mg_testkit::oracle_tool!("nwnmdlcomp");
    let tool = mgv_mdl::tools::Nwnmdlcomp { path: tool, game_root: Some(root.clone()) };
    let rm = ResMan::for_game(&GameInstall::new(&root, None, "en")).unwrap();
    let binary = |n: &str| {
        rm.get(&ResKey::parse(n, ResType::MDL).unwrap()).is_ok_and(|d| mg_mdl::is_binary(&d))
    };
    let mut names: Vec<String> = rm
        .list(ResType::MDL)
        .into_iter()
        .map(|r| r.to_lowercase().to_string())
        .filter(|n| binary(n))
        .step_by(400)
        .collect();
    // Skins, dangly and animated meshes, emitters, lights, walkmeshes.
    for n in ["c_golemerald", "plc_a01", "tcn01_a01_01", "t_door09", "c_wererat", "c_drgred"] {
        if binary(n) {
            names.push(n.into());
        }
    }
    names.sort();
    names.dedup();
    let cache = Mutex::new(HashMap::new());
    let results: Vec<(String, Vec<String>)> = names
        .par_iter()
        .filter_map(|name| {
            let data = rm.get(&ResKey::parse(name, ResType::MDL).unwrap()).unwrap().into_owned();
            let model = Model::read(&data).unwrap();
            // What it makes of the original, where it can read that.
            let theirs = Model::read(tool.decompile(&data, name).ok()?.as_bytes()).ok()?;
            let bytes = write(&model, &numbered(&rm, &model, &cache, 0)).unwrap();
            let problems = match tool.decompile(&bytes, name) {
                Ok(text) => match Model::read(text.as_bytes()) {
                    Ok(ours) => common::compare(&theirs, &ours, false),
                    Err(e) => vec![format!("its decompile is unreadable: {e}")],
                },
                Err(e) => vec![format!("it could not decompile it: {e}")],
            };
            Some((name.clone(), problems))
        })
        .collect();
    let failed: Vec<&(String, Vec<String>)> =
        results.iter().filter(|(_, p)| !p.is_empty()).collect();
    eprintln!("{} models, {} differ", results.len(), failed.len());
    for (name, p) in &failed {
        eprintln!("{name}: {}", p.iter().take(5).cloned().collect::<Vec<_>>().join("; "));
    }
    assert!(results.len() > 50);
    assert!(failed.is_empty());
}

/// A compiled supermodel and its part numbers, read from the game.
fn supermodel_of(rm: &ResMan, model: &Model) -> Option<(Model, PartNumbers)> {
    let name = model.supermodel.as_deref()?;
    let data = rm.get(&ResKey::new(ResRef::from_str(name).ok()?, ResType::MDL)).ok()?;
    Some((Model::read(&data).ok()?, PartNumbers::read(&data)?))
}

/// The native compiler on the game's models (the plan's Phase 9, stage B):
/// every compiled model without skin meshes, decompiled and compiled
/// again, is the model it was, with the part numbers it had.
#[test]
fn decompiled_models_compile_back() {
    let root = mg_testkit::corpus!();
    let rm = ResMan::for_game(&GameInstall::new(root, None, "en")).unwrap();
    let names = rm.list(ResType::MDL);
    let (compiled, skins) = (AtomicUsize::new(0), AtomicUsize::new(0));
    // Numbers kept from the compiled model, and numbered afresh from the
    // text alone (where the model's numbers are its own).
    let (kept, fresh) = (Tally::default(), Tally::default());
    let mut failures: Vec<(String, Vec<String>)> = names
        .par_iter()
        .filter_map(|name| {
            let data = rm.get(&ResKey::new(*name, ResType::MDL)).ok()?;
            let model = Model::read(&data).ok().filter(|_| mg_mdl::is_binary(&data))?;
            if model.nodes.iter().any(|n| n.kind.type_name() == "skin") {
                skins.fetch_add(1, Ordering::Relaxed);
                return None;
            }
            // Two nodes of one name cannot be told apart in text.
            if common::duplicate_names(&model) {
                return None;
            }
            let text = mgv_mdl::decompile(&data).ok()?;
            let numbers = PartNumbers::read(&data)?;
            let sup = supermodel_of(&rm, &model);
            let sources = mgv_mdl::compile::Sources {
                supermodel: sup.as_ref().map(|(m, p)| (m, p)),
                existing: Some((&model, &numbers)),
            };
            let out = match mgv_mdl::compile::compile(text.as_bytes(), &sources) {
                Ok(c) => c,
                Err(e) => return Some((name.to_string(), vec![format!("not compiled: {e}")])),
            };
            compiled.fetch_add(1, Ordering::Relaxed);
            let back = match Model::read(&out.binary) {
                Ok(m) => m,
                Err(e) => return Some((name.to_string(), vec![format!("unreadable: {e}")])),
            };
            let mut problems = common::differences(&model, &back);
            let by_name = |m: &Model, p: &PartNumbers| {
                let mut v: Vec<(String, i32)> = m
                    .nodes
                    .iter()
                    .zip(&p.numbers)
                    .map(|(n, k)| (n.name.to_lowercase(), *k))
                    .collect();
                v.sort();
                v
            };
            let theirs = by_name(&model, &numbers);
            let ours = PartNumbers::read(&out.binary).unwrap();
            let same = by_name(&back, &ours) == theirs && ours.count == numbers.count;
            kept.add(same);
            if !same {
                problems.push("part numbers changed".into());
            }
            // From the text alone.
            let mut sorted = numbers.numbers.clone();
            sorted.sort_unstable();
            if model.supermodel.is_none() && sorted.iter().enumerate().all(|(i, n)| *n == i as i32)
            {
                let alone = mgv_mdl::compile::compile(text.as_bytes(), &Default::default()).ok()?;
                let parts = PartNumbers::read(&alone.binary).unwrap();
                fresh.add(by_name(&Model::read(&alone.binary).ok()?, &parts) == theirs);
            }
            (!problems.is_empty()).then(|| (name.to_string(), problems))
        })
        .collect();
    failures.sort();
    eprintln!(
        "{} models compiled ({} with skins left out), {} differ; part numbers kept from the \
         compiled model: {}; numbered again from the text alone: {}",
        compiled.load(Ordering::Relaxed),
        skins.load(Ordering::Relaxed),
        failures.len(),
        kept.text(),
        fresh.text()
    );
    for (name, d) in failures.iter().take(25) {
        eprintln!("  {name}: {}", d.iter().take(4).cloned().collect::<Vec<_>>().join("; "));
    }
    assert!(compiled.load(Ordering::Relaxed) > 24_000);
    assert!(failures.is_empty(), "{} models differ", failures.len());
    assert!(fresh.share() >= 1.0, "{}", fresh.text());
}
