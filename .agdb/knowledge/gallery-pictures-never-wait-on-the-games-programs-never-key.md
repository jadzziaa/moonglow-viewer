---
type: Gotcha
title: "Gallery pictures: never wait on the game's programs, never key on lib.generation"
description: "Why the viewer's gallery draws thumbnails with headless::still_unwaited and drops its cache at explicit reload sites instead of on Library::generation, and how its GPU-free tests drive it."
tags: []
status: stable
generated: { by: agdb/0.2.0, at: 2026-11-25T15:22:22Z }
touches: [crates/mgv-ui/src/gallery.rs, crates/mgv-stage/src/headless.rs]
asks:
  - "why does the gallery not use headless::still"
  - "when are the gallery's pictures dropped"
  - why did the window freeze while making thumbnails
  - how do I test gallery picture requests without a GPU
fingerprint: v2-71000d2123867176
created: 2026-11-25
author: august
---
- headless::still/picture set game_background = false: the frame waits for the game's shader programs to compile. On the UI thread that froze the window for the first picture of each program pair. The gallery uses headless::still_unwaited (programs made in the background; reads back only a whole picture, else None) and asks again in a later frame, taking the picture as drawn after gallery::WAIT_AT_MOST (20 s since the first try; frame counts are no measure, frames come fast). Pictures::make drops waits not asked for this frame: a stale wait once stopped the repaint requests and the gallery stopped filling. still/picture still wait (mgv, galleries and tests rely on that).
- Library::generation bumps on every hot reload of the ASCII buffer (set_buffer) and on open_resource's set_opened_folder(None) even when nothing changes. Keying the picture cache on it would drop every picture on each edit or open. Instead Viewer::data_changed (Refresh, add folder/hak, game folder, watched files) forgets all, Pictures::begin_frame forgets when the resman layer labels or the opened folder change, and apply_buffer forgets one key.
- GPU-free tests: gallery::Painter is a trait; tests set v.pictures.painter to a fake and drive Viewer::ui through egui::Context::run_ui. The first run_ui can run the UI twice (the menu bar's request_discard), so count Pictures::painted per make, not painter calls per run. GalleryGrid's scroll_to_rect animates: step about 120 frames at 1/60 s after End before checking the last row is drawn. Give the grid the keys by a click event, not memory.request_focus.
