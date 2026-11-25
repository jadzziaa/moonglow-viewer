---
type: Gotcha
title: "A popup opened from beside a ComboBox needs the combo's IdSalt-hashed id"
description: "Why palette::pick computes the combo's popup id as make_persistent_id(egui::IdSalt::new(salt)).with(\"popup\") and not make_persistent_id(salt)."
tags: []
status: stable
generated: { by: agdb/0.2.0, at: 2026-11-25T16:13:43Z }
touches: ["crates/mgv-ui/src/palette.rs:pick"]
asks:
  - why does clicking the color swatch not open the palette grid?
  - "how do I open an egui ComboBox's popup from another widget?"
  - what is the popup id of an egui 0.36 ComboBox?
fingerprint: v2-c0e297e6104198d1
created: 2026-11-25
author: august
---
egui 0.36's ComboBox::from_id_salt(s) stores IdSalt::new(s) (an ahash of s) and its button id is ui.make_persistent_id(that IdSalt), its popup id that with("popup"). ui.make_persistent_id(s) hashes s itself, so the ids differ and Popup::toggle_id on it opens nothing (no error; the palette test saw 0 of 176 cells). Use ui.make_persistent_id(egui::IdSalt::new(salt)).with("popup") in the same ui as the combo. Toggling it on the swatch's click in the frame before the combo shows works: Popup::show skips close-on-click while the popup was not open last frame.
