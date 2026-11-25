---
type: Decision
title: "The viewer's docks are mg-uikit's; egui_dock holds only the editor tabs"
description: "Where the viewer's docks live and why egui_dock holds only the editor tabs"
tags: []
status: stable
generated: { by: agdb/0.2.0, at: 2026-11-25T14:28:02Z }
touches: [crates/mgv-ui/src/shell.rs, "crates/mgv-ui/src/lib.rs:Viewer::focus"]
asks:
  - "Why aren't the Inspector and Timeline in the egui_dock DockState?"
  - How do I bring a panel into view in the viewer?
  - "Should the viewer patch egui_dock to the toolset's vendored copy?"
  - "How can I test the viewer's UI layout without a GPU?"
fingerprint: v2-7cf8b0e52114db95
created: 2026-11-25
author: august
---
Since M6 part B only the editor area (3D View, Texture, ASCII) is an egui_dock DockArea, on stock egui_dock 0.21 with mg-uikit's dock feature. The side and bottom docks are mg-uikit's PanelHeader, DockTabs and FoldStrip drawn in crates/mgv-ui/src/shell.rs; shell::place decides where each panel stands (from 2560 wide, Log and Effects move into the Right Dock's two columns). Viewer::focus routes dock panels to Shell::show_panel: never push a dock panel into the DockState. The toolset's vendored egui_dock was deliberately not used: its additions serve floating editor windows, which the viewer has none of. GPU-free UI checks: shell::tests drive Viewer::ui through egui::Context::run_ui with no render state, and check header placement with mg_uikit::header::record_headers and places.
