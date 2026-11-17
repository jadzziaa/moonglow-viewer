---
type: Fact
title: "Worn part's own materials: mg-preview renames part-shaped bitmaps"
description: "Why a worn head's cap drew in the head PLT, and the viewer's workaround (textures::worn_renames)"
tags: []
status: stable
generated: { by: agdb/0.2.0, at: 2026-11-17T00:55:32Z }
touches: [crates/mgv-stage/src/textures.rs, crates/mgv-stage/src/lib.rs]
asks:
  - "Why does a worn head draw its cap or eyes with the head's PLT?"
  - "Where are a creature preview part's texture renames filtered?"
  - "Does an MTR on a body part mesh override the part's PLT in the game?"
  - What does mg-preview plt_fallbacks rename?
fingerprint: v2-05ca536e0848dd2c
created: 2026-11-17
author: august
---
mg-preview's plt_fallbacks (creature.rs) gives a worn part's mesh the part's own texture whenever the mesh's bitmap is shaped like a part name (p<g><r><pheno>_<rest>, e.g. pmh0_head253c), and mg-render applies the renames after resolving the MTR, so the MTR's texture0 gets renamed too. TDoD's heads.py names every head material <head>c/e, so caps and eyes drew in the head PLT when worn (fine alone: no preview renames). Game: an MTR named as a part mesh's bitmap overrides the PLT (nwn.wiki PLT page); the client drew head 253's cap whole. Workaround: mgv_stage::textures::worn_renames drops renames whose source names no PLT and resolves to an MTR, DDS or TGA; upstream fix proposed in docs/PLAN.md 10.4. Base game has 7 MTRs, none part-named; 80 stock creature blueprints rendered byte-identical before/after. Unverified: whether the game keeps a part-shaped plain TGA without an MTR.
