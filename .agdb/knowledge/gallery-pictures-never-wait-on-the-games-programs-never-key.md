---
type: Gotcha
title: "Gallery pictures: the kit's PaintedPictures, never wait on the game's programs, never key on lib.generation"
description: "How the viewer's gallery pictures work over mg-uikit's PaintedPictures, why they draw with headless::still_unwaited and drop the cache at explicit reload sites, and how the GPU-free tests drive them."
tags: []
status: stable
generated: { by: agdb/0.2.0, at: 2026-11-26T14:14:23Z }
touches: [crates/mgv-ui/src/gallery.rs, crates/mgv-stage/src/headless.rs]
asks:
  - "why does the gallery not use headless::still"
  - what happens to a gallery picture after 20 seconds
  - "why does gallery::Pictures keep its own reasons map and asked set beside PaintedPictures"
  - how do I test gallery picture requests without a GPU
fingerprint: v2-cb261addc00d7b4c
created: 2026-11-25
author: august
---
- headless::still/picture set game_background = false: the frame waits for the game's shader programs to compile. On the UI thread that froze the window for the first picture of each program pair. The gallery uses headless::still_unwaited (programs made in the background; reads back only a whole picture, else None, which gallery::painted makes the kit's Painted::Later) and asks again later, taking the picture as drawn after gallery::WAIT_AT_MOST (20 s since the first try; frame counts are no measure, frames come fast). With as_drawn still_unwaited always reads back, so the kit's "Not drawn within 20 s" never shows.
- Since toolset 5.4.0 the cache is mg-uikit's PaintedPictures<Pictured> (gallery::Pictures wraps it): the retry (Retry { again: AGAIN 50 ms, give_up: WAIT_AT_MOST }), the budget (TIME 4 ms, PER_FRAME 8), keeping (KEPT 512) and dropping waits not asked for this frame are the kit's pump. The viewer's Painter (it needs the frame's &Library) becomes a PicturePainter through gallery::Painting, which keeps the old cadence: once one paints Later, the rest of that pump answer Later undrawn unless as_drawn (never answer Later to an as_drawn ask: the kit turns that into a permanent failure). Kept beside the cache: the per-frame asked set (GridOutput::ahead includes the cells in sight, so without it each drawn cell is asked twice), a reasons map for Pictures::error(&self) (the kit's PaintedPicture is private and get() needs &mut and counts as an ask), the layers check, painter.forget.
- Library::generation bumps on every hot reload of the ASCII buffer (set_buffer) and on open_resource's set_opened_folder(None) even when nothing changes. Keying the picture cache on it would drop every picture on each edit or open. Instead Viewer::data_changed (Refresh, add folder/hak, game folder, watched files) forgets all, Pictures::begin_frame forgets when the resman layer labels or the opened folder change, and apply_buffer forgets one key.
- GPU-free tests: gallery::Painter is a trait; tests set v.pictures.painter to a fake (fake() also sets cache.retry.again = 0: the kit's wait between tries is wall-clock, not frames) and drive Viewer::ui through egui::Context::run_ui. Test the 20 s give-up by setting cache.retry.give_up = 0, not by editing the kit's private tries. The first run_ui can run the UI twice (the menu bar's request_discard), so count Pictures::painted per make, not painter calls per run. GalleryGrid's scroll_to_rect animates: step about 120 frames at 1/60 s after End before checking the last row is drawn. Give the grid the keys by a click event, not memory.request_focus.
