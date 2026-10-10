//! The native decompiler against nwnmdlcomp (L2): on a sample of the game's
//! binary models, nwnmdlcomp's decompile reads as the same model as ours,
//! and nwnmdlcomp compiles our ASCII back into the same model. Normals are
//! left out: nwnmdlcomp neither writes nor reads them (it computes them from
//! smoothing groups), so they are compared loosely.

mod common;

use mg_core::ResType;
use mg_mdl::Model;
use mg_resman::{GameInstall, ResKey, ResMan};
use mgv_mdl::tools::Nwnmdlcomp;
use rayon::prelude::*;

/// Every 400th binary model by name, plus models picked for their node
/// types (skins, dangly meshes, animated meshes, emitters, lights,
/// walkmeshes).
fn sample(rm: &ResMan) -> Vec<String> {
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
    // plc_k01 keys self-illumination in its animations.
    for n in [
        "c_golemerald",
        "plc_a01",
        "tcn01_a01_01",
        "t_door09",
        "c_wererat",
        "vdr_magearmor",
        "plc_k01",
        // Numbered in another order than their trees' (the jelly and the
        // ooze with skins, which stay in the tree's).
        "c_jelly",
        "c_ooze_a01",
        "c_a_bat",
        "plc_a02",
    ] {
        if binary(n) {
            names.push(n.into());
        }
    }
    names.sort();
    names.dedup();
    names
}

#[test]
fn nwnmdlcomp_agrees_with_the_native_decompiler() {
    let root = mg_testkit::corpus!();
    let tool = mg_testkit::oracle_tool!("nwnmdlcomp");
    let tool = Nwnmdlcomp { path: tool, game_root: Some(root.clone()) };
    let rm = ResMan::for_game(&GameInstall::new(&root, None, "en")).unwrap();
    let names = sample(&rm);
    let results: Vec<(String, Vec<String>)> = names
        .par_iter()
        .map(|name| {
            let data = rm.get(&ResKey::parse(name, ResType::MDL).unwrap()).unwrap().into_owned();
            let bin = Model::read(&data).unwrap();
            let mut problems = Vec::new();
            // Their decompile against ours.
            match tool.decompile(&data, name) {
                Ok(text) => match Model::read(text.as_bytes()) {
                    Ok(theirs) => {
                        let ours =
                            Model::read(mg_mdl::decompile(&data).unwrap().as_bytes()).unwrap();
                        for d in common::compare(&ours, &theirs, false) {
                            problems.push(format!("their decompile: {d}"));
                        }
                    }
                    Err(e) => problems.push(format!("their decompile unreadable: {e}")),
                },
                // nwnmdlcomp fails on a few models (nwn.wiki: some EE
                // compiles); that is its limit, not ours.
                Err(e) => eprintln!("{name}: nwnmdlcomp could not decompile it: {e}"),
            }
            // Our ASCII compiled by them.
            let ours = mg_mdl::decompile(&data).unwrap();
            match tool.compile(&ours, name, &[]) {
                Ok(compiled) => match Model::read(&compiled) {
                    Ok(back) => {
                        for d in common::compare(&bin, &back, false) {
                            problems.push(format!("compiled: {d}"));
                        }
                        // Compiled again, the nodes have the numbers they
                        // had (the game finds a supermodel's animations
                        // by them), where those are the model's own.
                        let numbers = |d: &[u8], m: &Model| {
                            let p = mg_mdl::binary_write::PartNumbers::read(d)?;
                            let mut by_name: Vec<(String, i32)> = m
                                .nodes
                                .iter()
                                .zip(&p.numbers)
                                .map(|(n, k)| (n.name.to_lowercase(), *k))
                                .collect();
                            by_name.sort();
                            Some(by_name)
                        };
                        let skinned = bin.nodes.iter().any(|n| n.kind.type_name() == "skin");
                        if bin.supermodel.is_none()
                            && !skinned
                            && !common::duplicate_names(&bin)
                            && numbers(&data, &bin) != numbers(&compiled, &back)
                        {
                            problems.push("compiled: part numbers changed".into());
                        }
                    }
                    Err(e) => problems.push(format!("compiled unreadable: {e}")),
                },
                Err(e) => problems.push(format!("compile failed: {e}")),
            }
            (name.clone(), problems)
        })
        .collect();
    let failed: Vec<&(String, Vec<String>)> =
        results.iter().filter(|(_, p)| !p.is_empty()).collect();
    eprintln!("{} models, {} differ", results.len(), failed.len());
    for (name, p) in &failed {
        eprintln!("{name}: {}", p.iter().take(5).cloned().collect::<Vec<_>>().join("; "));
    }
    assert!(failed.is_empty());
}
