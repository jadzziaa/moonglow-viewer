//! Moonglow Viewer, the desktop application.

// No console window beside the app on Windows (release builds).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use mgv_ui::{Action, Dialogs, Settings, Viewer};

/// Native file dialogs.
struct NativeDialogs;

impl Dialogs for NativeDialogs {
    fn open_file(&mut self, title: &str, filters: &[(&str, &[&str])]) -> Option<PathBuf> {
        let mut d = rfd::FileDialog::new().set_title(title);
        for (name, exts) in filters {
            d = d.add_filter(*name, exts);
        }
        d.pick_file()
    }

    fn save_file(&mut self, title: &str, suggested: &Path) -> Option<PathBuf> {
        let mut d = rfd::FileDialog::new().set_title(title);
        if let Some(dir) = suggested.parent().filter(|p| !p.as_os_str().is_empty()) {
            d = d.set_directory(dir);
        }
        if let Some(name) = suggested.file_name() {
            d = d.set_file_name(name.to_string_lossy());
        }
        d.save_file()
    }

    fn pick_folder(&mut self, title: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().set_title(title).pick_folder()
    }
}

const APP_NAME: &str = "Moonglow Viewer";

/// The desktop entry's name (Linux: Wayland's app ID and X11's class, which
/// match the window to `packaging/linux/<APP_ID>.desktop` and its icon).
const APP_ID: &str = "io.github.moonglow_toolset.MoonglowViewer";

/// Where eframe keeps the settings.
const SETTINGS_KEY: &str = "moonglow-viewer-settings";

struct App {
    viewer: Viewer,
    title: String,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Closing the window keeps unsaved edits like File > Quit does.
        if ui.ctx().input(|i| i.viewport().close_requested()) && !self.viewer.quit_requested {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.viewer.actions.push(Action::Quit);
        }
        // Files dropped on the window open.
        let dropped: Vec<PathBuf> = ui
            .ctx()
            .input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if let Some(p) = dropped.into_iter().next() {
            self.viewer.actions.push(Action::Open(p));
        }
        self.viewer.ui(ui);
        let title = self.viewer.title();
        if title != self.title {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        if self.viewer.quit_requested {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, SETTINGS_KEY, &self.viewer.settings);
    }
}

/// On a crash, a report in the viewer's data folder (`crash-<time>.txt`):
/// the message, where, and the backtrace.
fn crash_reports() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        let Some(dir) = mgv_ui::settings::data_dir() else { return };
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let report = format!(
            "Moonglow Viewer {} crashed.\n\n{info}\n\nBacktrace:\n{}\n",
            env!("CARGO_PKG_VERSION"),
            std::backtrace::Backtrace::force_capture()
        );
        let path = dir.join(format!("crash-{time}.txt"));
        if std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, report)).is_ok() {
            eprintln!("A crash report was written to {}", path.display());
        }
    }));
}

/// The window's icon (`packaging/icons`).
fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!(
        "../../../packaging/icons/moonglow-viewer-256.png"
    ))
    .unwrap_or_default()
}

fn main() -> eframe::Result<()> {
    crash_reports();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_title(APP_NAME)
            .with_app_id(APP_ID)
            .with_icon(window_icon())
            .with_drag_and_drop(true),
        wgpu_options: egui_wgpu::WgpuConfiguration {
            wgpu_setup: egui_wgpu::WgpuSetup::CreateNew(egui_wgpu::WgpuSetupCreateNew {
                // Game textures are mostly BC-compressed: upload them as
                // they are where the GPU can read them.
                device_descriptor: std::sync::Arc::new(|adapter| wgpu::DeviceDescriptor {
                    label: Some("moonglow-viewer"),
                    required_features: adapter.features() & wgpu::Features::TEXTURE_COMPRESSION_BC,
                    required_limits: adapter.limits(),
                    ..Default::default()
                }),
                ..egui_wgpu::WgpuSetupCreateNew::without_display_handle()
            }),
            ..Default::default()
        },
        persistence_path: eframe::storage_dir(APP_NAME).map(|d| d.join("app.ron")),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| {
            let settings: Settings =
                cc.storage.and_then(|s| eframe::get_value(s, SETTINGS_KEY)).unwrap_or_default();
            let mut viewer = Viewer::new(settings, Box::new(NativeDialogs));
            if let Some(rs) = &cc.wgpu_render_state {
                viewer.set_render_state(rs.clone());
            }
            // `moonglow-viewer path/to/model.mdl` opens it at start.
            if let Some(path) = std::env::args_os().nth(1) {
                viewer.actions.push(Action::Open(PathBuf::from(path)));
            }
            Ok(Box::new(App { viewer, title: String::new() }))
        }),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_window_icon_decodes() {
        let icon = super::window_icon();
        assert_eq!((icon.width, icon.height, icon.rgba.len()), (256, 256, 256 * 256 * 4));
    }
}
