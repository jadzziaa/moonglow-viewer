---
type: Decision
title: "Moving dock panels and the kept layout (M6 part D): parity of mechanism, forced sizes, lenient load, not the kit's stack"
description: "How the viewer moves panels between its Left, Right and Bottom docks and keeps the layout, what the toolset does differently, and why it stays off mg-uikit 5.4.0's StackLayout, StackDrop and KeptLayout."
tags: []
status: stable
generated: { by: agdb/0.2.0, at: 2026-11-26T14:14:23Z }
touches: [crates/mgv-ui/src/shell.rs, "crates/mgv-ui/src/settings.rs:Layout"]
asks:
  - can the toolset drag panels between docks?
  - "why does the viewer not use the kit's StackLayout, StackDrop or KeptLayout?"
  - what happens to an old or broken layout in the settings?
  - where is the drop preview for a dragged panel drawn?
fingerprint: v2-a85b4b1f974086d8
created: 2026-11-25
author: august
---
The toolset never moves panels between Left/Right/Bottom (mg-ui Panel::dock is fixed); its drag only rearranges the area editor's Right Dock (AreaDropZone, accent preview on a Foreground layer, drop on release, a context menu). The viewer reuses that mechanism across its three docks: shell::drag_source (PanelHeader/DockTabs draggable + context menu), docks record drop zones each frame, folded/closed/empty docks take a 96-wide band of the editor area (shell::drop_target, band), shell::drop_panel outlines with mg-uikit's stack::paint_drop_preview (since 5.4.0) and moves on release (Esc cancels). Sizes are kept as the toolset keeps them (since toolset 5.5.0 in mg-uikit's stack::DockRecord, LAYOUT_VERSION 2, sizes u16), forced on every frame (size_panel) and captured only when the user drags an edge, so a narrow window never overwrites a wide one's sizes. Folds: only a fold made by hand is kept; the width rules (shell_migrate's) fold on top. Settings::layout loads through a serde untagged enum {Layout, IgnoredAny} (lenient_layout): garbage or an unknown version gives the default while the rest of app.ron still loads; version 1 (the viewer's own part D struct) migrates to the DockRecord (settings::AnyLayout::record), pinned by settings::tests::a_part_d_layout_moves_to_the_kit_s_record. Panel ids under egui 0.37: see [[egui-0-37-panels-keep-their-state-under-scope-id-salt-force]].
Why not the kit's 5.4.0 stack (checked when repinning): the viewer's docks hold no StackLayout (a panel's dock comes from shell::place plus Shell::moved, in a fixed PANELS/BOTTOM_ORDER order). StackDrop::target returns None on a pane with no sections (the "band with an empty stack" never hits) and with sections finds bar/Above/Below/Into zones outlining half a section, not the whole dock; stack_heights shares equally with 80 least where the viewer's first panel opens at 55% with 90 least and keeps seams in points by panel. KeptLayout (fronts, shares, column, folded by editor kind) holds none of settings::Layout's fields. Adopting any of it changes what users see.
