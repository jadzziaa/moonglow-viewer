---
type: Decision
title: "Moving dock panels and the kept layout (M6 part D): parity of mechanism, forced sizes, lenient load"
description: "How the viewer moves panels between its Left, Right and Bottom docks and keeps the layout, and what the toolset does differently."
tags: []
status: stable
generated: { by: agdb/0.2.0, at: 2026-11-25T16:13:43Z }
touches: [crates/mgv-ui/src/shell.rs, "crates/mgv-ui/src/settings.rs:Layout"]
asks:
  - can the toolset drag panels between docks?
  - why are panel sizes forced every frame?
  - what happens to an old or broken layout in the settings?
  - where is the drop preview for a dragged panel drawn?
fingerprint: v2-3139cd6061c7d2c9
created: 2026-11-25
author: august
---
The toolset never moves panels between Left/Right/Bottom (mg-ui Panel::dock is fixed); its drag only rearranges the area editor's Right Dock (AreaDropZone, accent preview on a Foreground layer, drop on release, a context menu). The viewer reuses that mechanism across its three docks: shell::drag_source (PanelHeader/DockTabs draggable + context menu), docks record drop zones each frame, folded/closed/empty docks take a 96-wide band of the editor area, shell::drop_panel paints the outline and moves on release (Esc cancels). Sizes are kept as the toolset keeps them: Option<u32>, forced on every frame (size_panel) and captured only when the user drags an edge, so a narrow window never overwrites a wide one's sizes. Folds: only a fold made by hand is kept; the width rules (shell_migrate's) fold on top. Settings::layout loads through a serde untagged enum {Layout, IgnoredAny} (lenient_layout): garbage or another LAYOUT_VERSION gives the default while the rest of app.ron still loads; verified under RON and JSON.
