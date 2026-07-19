mod app;
mod image_ops;
mod navigation;
mod settings;

use app::RustyViewerApp;
use eframe::egui;

#[cfg(target_os = "linux")]
use winit::platform::x11::EventLoopBuilderExtX11;

fn main() -> eframe::Result<()> {
    // Set up standard environment logging (optional but good)
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Work around a winit/Wayland limitation: the winit version pulled in by eframe 0.28.1
    // does not reliably deliver file drag-and-drop events under a native Wayland session
    // (see rust-windowing/winit#1881 and emilk/egui#1563), even with
    // ViewportBuilder::with_drag_and_drop(true) below. Dropped files ARE delivered
    // correctly through XWayland's X11 path.
    //
    // Note: setting the WINIT_UNIX_BACKEND env var does NOT reliably force this winit
    // version to pick X11 (its backend-selection logic doesn't consistently honor it).
    // The mechanism that actually works is telling winit's own EventLoopBuilder to use
    // X11 directly, via the EventLoopBuilderExtX11::with_x11() extension trait, wired in
    // through eframe's event_loop_builder hook below.
    #[cfg(target_os = "linux")]
    let event_loop_builder: Option<eframe::EventLoopBuilderHook> = {
        let running_under_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
        let xwayland_available = std::env::var("DISPLAY").is_ok();
        if running_under_wayland && xwayland_available {
            Some(Box::new(|builder| {
                builder.with_x11();
            }))
        } else {
            // Not on Wayland (or no XWayland to fall back to): leave winit's default
            // backend selection alone.
            None
        }
    };
    #[cfg(not(target_os = "linux"))]
    let event_loop_builder: Option<eframe::EventLoopBuilderHook> = None;

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([500.0, 400.0])
            .with_drag_and_drop(true) // Explicitly enable drag and drop window events
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png"))
                    .expect("failed to decode embedded app icon"),
            ),
        event_loop_builder,
        ..Default::default()
    };

    eframe::run_native(
        "RustyViewer",
        native_options,
        Box::new(|cc| {
            let mut app = RustyViewerApp::default();

            // Load command line argument if it's an image file
            let args: Vec<String> = std::env::args().collect();
            if args.len() > 1 {
                let startup_path = std::path::PathBuf::from(&args[1]);
                if startup_path.exists() && startup_path.is_file() {
                    app.load_image(startup_path, &cc.egui_ctx);
                }
            }

            Ok(Box::new(app))
        }),
    )
}