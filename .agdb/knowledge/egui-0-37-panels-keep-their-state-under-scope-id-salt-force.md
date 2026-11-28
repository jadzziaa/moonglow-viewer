---
type: Gotcha
title: "egui 0.37 panels keep their state under scope_id + salt: force sizes through shell::panel_id"
description: "Why the docks ignored their kept sizes after the move to egui 0.37, and how the viewer finds a panel's real PanelState id."
tags: []
status: stable
generated: { by: agdb/0.2.0, at: 2026-11-28T04:45:36Z }
touches: ["crates/mgv-ui/src/shell.rs:panel_id", "crates/mgv-ui/src/shell.rs:sized"]
asks:
  - why do the docks open at their default width after egui 0.37
  - "where does egui keep a Panel's PanelState"
  - "kept dock sizes are ignored, Left Dock stays 260"
  - "how does the viewer force a panel's size each frame"
fingerprint: v2-c212362ffacbeaea
created: 2026-11-28
author: august
---
In egui 0.37 Panel::left/right/top/bottom take an id SALT (impl AsIdSalt); the panel's Id is parent_ui.scope_id().with_salt(IdSalt::new(salt)), and its PanelState and resize handle (id.with("__resize")) live under that Id. The viewer forces dock sizes every frame by writing PanelState before showing the panel (shell::size_panel) and reads edge drags from the resize handle (edge_dragged). Under 0.36 the value passed to Panel::left was the Id itself; after the bump the writes went to the bare salt, egui never read them, and every dock (Left, Right, Bottom, the stacked panels' seams) opened at its default: the_layout_round_trips_through_the_settings, the width-rules test and two gallery tests failed (Left Dock 260 instead of 300/440, gallery one column). Fix: shell::sized takes the ui the panel is shown in and derives shell::panel_id(ui, salt). Any new code that touches a panel's state must do the same, with the same ui that is passed to Panel::show.
