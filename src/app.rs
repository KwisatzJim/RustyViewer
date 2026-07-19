use std::path::PathBuf;
use image::DynamicImage;
use egui::{TextureHandle, Vec2, Key, Rect, Pos2};

enum BatchProgress {
    Progress(usize, String),
    Done(String),
    Error(String),
}

pub struct RustyViewerApp {
    // Current loaded file path
    image_path: Option<PathBuf>,
    // Sibling images in the same directory
    siblings: Vec<PathBuf>,
    // Current index in siblings
    sibling_index: usize,

    // Original image loaded from disk (or updated after a destructive filter)
    base_image: Option<DynamicImage>,
    // Adjusted image with non-destructive sliders applied (cached for exporting/saving/copying)
    adjusted_image: Option<DynamicImage>,
    // GPU texture handle of the adjusted image
    texture: Option<TextureHandle>,

    // Non-destructive sliders
    brightness: f32, // -1.0 to 1.0
    contrast: f32,   // -1.0 to 1.0
    saturation: f32, // -1.0 to 1.0
    gamma: f32,      // 0.1 to 3.0 (default 1.0)
    r_tint: f32,     // -1.0 to 1.0 (default 0.0)
    g_tint: f32,     // -1.0 to 1.0 (default 0.0)
    b_tint: f32,     // -1.0 to 1.0 (default 0.0)

    // Viewport state
    zoom: f32,
    pan: Vec2,
    fit_to_window: bool,

    // Layout toggles
    show_adjustments: bool,

    // Clipboard integration
    clipboard: Option<arboard::Clipboard>,

    // --- Advanced Features State ---
    // Undo Stack
    undo_stack: Vec<DynamicImage>,

    // Selection box for cropping
    selection_start: Option<Pos2>,
    selection_rect: Option<Rect>,

    // The actual on-screen rect/size the image was drawn at last frame in the
    // Central Panel (post-layout, i.e. after side panels have claimed their space).
    // This is the source of truth for mapping a screen-space selection back to
    // image pixel coordinates; it must NOT be re-derived from ctx.available_rect(),
    // which does not reflect side panel widths or the real top bar height.
    last_image_rect: Rect,
    last_display_size: Vec2,

    // Resize Dialog State
    show_resize_dialog: bool,
    resize_width: String,
    resize_height: String,
    keep_aspect_ratio: bool,

    // Batch Dialog State
    show_batch_dialog: bool,
    batch_input_files: Vec<PathBuf>,
    batch_output_dir: String,
    batch_output_format: String, // "PNG", "JPEG", "WebP", "BMP"
    batch_resize: bool,
    batch_resize_w: String,
    batch_resize_h: String,
    batch_keep_aspect_ratio: bool,
    batch_rotate: bool,
    batch_rotate_angle: i32, // 90, 180, 270
    batch_grayscale: bool,
    batch_invert: bool,
    batch_auto_adjust: bool,
    
    // Batch execution state
    batch_running: bool,
    batch_progress: usize,
    batch_total: usize,
    batch_status_message: String,
    batch_rx: Option<std::sync::mpsc::Receiver<BatchProgress>>,
}

impl Default for RustyViewerApp {
    fn default() -> Self {
        Self {
            image_path: None,
            siblings: Vec::new(),
            sibling_index: 0,
            base_image: None,
            adjusted_image: None,
            texture: None,
            brightness: 0.0,
            contrast: 0.0,
            saturation: 0.0,
            gamma: 1.0,
            r_tint: 0.0,
            g_tint: 0.0,
            b_tint: 0.0,
            zoom: 1.0,
            pan: Vec2::ZERO,
            fit_to_window: true,
            show_adjustments: true,
            clipboard: arboard::Clipboard::new().ok(),
            undo_stack: Vec::new(),
            selection_start: None,
            selection_rect: None,
            last_image_rect: Rect::NOTHING,
            last_display_size: Vec2::ZERO,
            show_resize_dialog: false,
            resize_width: "800".to_string(),
            resize_height: "600".to_string(),
            keep_aspect_ratio: true,
            show_batch_dialog: false,
            batch_input_files: Vec::new(),
            batch_output_dir: crate::settings::initial_batch_output_dir().to_string_lossy().to_string(),
            batch_output_format: "PNG".to_string(),
            batch_resize: false,
            batch_resize_w: "800".to_string(),
            batch_resize_h: "600".to_string(),
            batch_keep_aspect_ratio: true,
            batch_rotate: false,
            batch_rotate_angle: 90,
            batch_grayscale: false,
            batch_invert: false,
            batch_auto_adjust: false,
            batch_running: false,
            batch_progress: 0,
            batch_total: 0,
            batch_status_message: String::new(),
            batch_rx: None,
        }
    }
}

impl RustyViewerApp {
    /// Load an image path into memory and discover sibling files in the parent directory
    pub fn load_image(&mut self, path: PathBuf, ctx: &egui::Context) {
        match image::open(&path) {
            Ok(img) => {
                let (siblings, index) = crate::navigation::get_image_siblings(&path);
                self.siblings = siblings;
                self.sibling_index = index;
                self.image_path = Some(path);

                self.base_image = Some(img);

                // Reset non-destructive adjustments
                self.brightness = 0.0;
                self.contrast = 0.0;
                self.saturation = 0.0;
                self.gamma = 1.0;
                self.r_tint = 0.0;
                self.g_tint = 0.0;
                self.b_tint = 0.0;

                // Reset viewport & selection
                self.zoom = 1.0;
                self.pan = Vec2::ZERO;
                self.fit_to_window = true;
                self.selection_rect = None;
                self.selection_start = None;

                // Clear Undo stack for a fresh image load
                self.undo_stack.clear();

                self.update_texture(ctx);
            }
            Err(e) => {
                eprintln!("Failed to open image at {}: {:?}", path.display(), e);
            }
        }
    }

    /// Update the GPU texture cache by applying non-destructive adjustments to the base image
    fn update_texture(&mut self, ctx: &egui::Context) {
        if let Some(base) = &self.base_image {
            let adjusted = crate::image_ops::apply_adjustments(
                base,
                self.brightness,
                self.contrast,
                self.saturation,
                self.gamma,
                self.r_tint,
                self.g_tint,
                self.b_tint,
            );

            let color_img = crate::image_ops::to_color_image(&adjusted);
            let filename = self.image_path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "clipboard_image.png".to_string());

            self.texture = Some(ctx.load_texture(filename, color_img, Default::default()));
            self.adjusted_image = Some(adjusted);
        } else {
            self.texture = None;
            self.adjusted_image = None;
        }
    }

    /// Push current base image state onto the Undo history stack
    fn push_undo(&mut self) {
        if let Some(base) = &self.base_image {
            self.undo_stack.push(base.clone());
            if self.undo_stack.len() > 10 {
                self.undo_stack.remove(0); // Limit memory footprint to 10 states
            }
        }
    }

    /// Revert the last destructive image operation
    fn undo(&mut self, ctx: &egui::Context) {
        if let Some(prev) = self.undo_stack.pop() {
            self.base_image = Some(prev);
            self.update_texture(ctx);
        }
    }

    /// Apply a destructive pixels modification (e.g. rotation, flip, grayscale)
    fn apply_destructive_op<F>(&mut self, op: F, ctx: &egui::Context)
    where
        F: FnOnce(&DynamicImage) -> DynamicImage,
    {
        if let Some(base) = self.base_image.clone() {
            self.push_undo();
            self.base_image = Some(op(&base));
            self.update_texture(ctx);
        }
    }

    /// Advance to the next image in the directory
    fn next_image(&mut self, ctx: &egui::Context) {
        if self.siblings.is_empty() {
            return;
        }
        self.sibling_index = (self.sibling_index + 1) % self.siblings.len();
        let path = self.siblings[self.sibling_index].clone();
        self.load_image(path, ctx);
    }

    /// Go back to the previous image in the directory
    fn prev_image(&mut self, ctx: &egui::Context) {
        if self.siblings.is_empty() {
            return;
        }
        if self.sibling_index == 0 {
            self.sibling_index = self.siblings.len() - 1;
        } else {
            self.sibling_index -= 1;
        }
        let path = self.siblings[self.sibling_index].clone();
        self.load_image(path, ctx);
    }

    /// Open native file dialog to select an image to view
    fn open_file_dialog(&mut self, ctx: &egui::Context) {
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff", "tif", "ico"]);

        if let Some(path) = &self.image_path {
            if let Some(parent) = path.parent() {
                dialog = dialog.set_directory(parent);
            }
        }

        if let Some(path) = dialog.pick_file() {
            self.load_image(path, ctx);
        }
    }

    /// Open native save file dialog to write the adjusted image to disk
    fn save_image_as(&self) {
        if let Some(img) = &self.adjusted_image {
            let mut dialog = rfd::FileDialog::new()
                .add_filter("PNG Image", &["png"])
                .add_filter("JPEG Image", &["jpg", "jpeg"])
                .add_filter("WebP Image", &["webp"])
                .add_filter("BMP Image", &["bmp"]);

            if let Some(path) = &self.image_path {
                if let Some(parent) = path.parent() {
                    dialog = dialog.set_directory(parent);
                }
                if let Some(filename) = path.file_name() {
                    dialog = dialog.set_file_name(filename.to_string_lossy().to_string());
                }
            }

            if let Some(save_path) = dialog.save_file() {
                if let Err(e) = img.save(&save_path) {
                    eprintln!("Failed to save image to {}: {:?}", save_path.display(), e);
                }
            }
        }
    }

    /// Copy current adjusted image to clipboard (mutable)
    fn copy_to_clipboard_mut(&mut self) {
        if let (Some(img), Some(cb)) = (&self.adjusted_image, &mut self.clipboard) {
            let rgba = img.to_rgba8();
            let img_data = arboard::ImageData {
                width: img.width() as usize,
                height: img.height() as usize,
                bytes: std::borrow::Cow::Borrowed(rgba.as_flat_samples().samples),
            };
            if let Err(e) = cb.set_image(img_data) {
                eprintln!("Clipboard copy failed: {:?}", e);
            }
        }
    }

    /// Paste an image from the clipboard
    fn paste_from_clipboard(&mut self, ctx: &egui::Context) {
        if let Some(cb) = &mut self.clipboard {
            match cb.get_image() {
                Ok(img_data) => {
                    let width = img_data.width as u32;
                    let height = img_data.height as u32;
                    if let Some(buffer) = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(
                        width,
                        height,
                        img_data.bytes.into_owned(),
                    ) {
                        self.image_path = None;
                        self.siblings.clear();
                        self.sibling_index = 0;

                        self.base_image = Some(DynamicImage::ImageRgba8(buffer));

                        // Reset adjustments
                        self.brightness = 0.0;
                        self.contrast = 0.0;
                        self.saturation = 0.0;
                        self.gamma = 1.0;
                        self.r_tint = 0.0;
                        self.g_tint = 0.0;
                        self.b_tint = 0.0;

                        // Reset viewport
                        self.zoom = 1.0;
                        self.pan = Vec2::ZERO;
                        self.fit_to_window = true;
                        self.selection_rect = None;
                        self.selection_start = None;

                        self.undo_stack.clear();

                        self.update_texture(ctx);
                    }
                }
                Err(e) => {
                    eprintln!("Clipboard paste failed: {:?}", e);
                }
            }
        }
    }

    /// Crop base image to the user's active selection rectangle
    fn crop_to_selection(&mut self, ctx: &egui::Context, image_rect: Rect, display_size: Vec2) {
        let sel_rect = match self.selection_rect {
            Some(r) => r,
            None => return,
        };

        // Intersect selection box with image viewport bounds to clamp bounds
        let active_sel = sel_rect.intersect(image_rect);
        if !active_sel.is_positive() {
            return;
        }

        if let Some(img) = self.base_image.clone() {
            let min_norm_x = (active_sel.min.x - image_rect.min.x) / display_size.x;
            let min_norm_y = (active_sel.min.y - image_rect.min.y) / display_size.y;
            let max_norm_x = (active_sel.max.x - image_rect.min.x) / display_size.x;
            let max_norm_y = (active_sel.max.y - image_rect.min.y) / display_size.y;

            let img_w = img.width() as f32;
            let img_h = img.height() as f32;

            let px = (min_norm_x * img_w).round().max(0.0) as u32;
            let py = (min_norm_y * img_h).round().max(0.0) as u32;
            let pw = ((max_norm_x - min_norm_x) * img_w).round().max(1.0) as u32;
            let ph = ((max_norm_y - min_norm_y) * img_h).round().max(1.0) as u32;

            self.push_undo();
            self.base_image = Some(crate::image_ops::crop_image(&img, px, py, pw, ph));
            self.selection_rect = None;
            self.selection_start = None;
            self.update_texture(ctx);
        }
    }

    /// Check and handle keyboard shortcut combinations
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context, image_rect: Rect, display_size: Vec2) {
        // If any text field has focus (e.g. in Resize or Batch dialogs), ignore shortcuts to prevent conflicts
        if ctx.wants_keyboard_input() {
            return;
        }

        let mut next = false;
        let mut prev = false;
        let mut fit = false;
        let mut actual = false;
        let mut rot_cw = false;
        let mut rot_ccw = false;
        let mut flip_h = false;
        let mut flip_v = false;
        let mut gray = false;
        let mut invert = false;
        let mut open = false;
        let mut save = false;
        let mut copy = false;
        let mut paste = false;
        let mut zoom_in = false;
        let mut zoom_out = false;

        // New shortcuts
        let mut undo = false;
        let mut crop = false;
        let mut resize = false;
        let mut batch = false;
        let mut auto_levels = false;

        ctx.input(|i| {
            let cmd = i.modifiers.command;
            let shift = i.modifiers.shift;

            // Navigation
            next = i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::Space);
            prev = i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::Backspace);

            // Fit to Window / Zoom (Num0 or F in IrfanView)
            fit = i.key_pressed(Key::Num0) || i.key_pressed(Key::F);
            actual = i.key_pressed(Key::Num1);

            // File Operations: 'O' or 'Ctrl+O' for open
            open = (i.key_pressed(Key::O) && !cmd) || (cmd && i.key_pressed(Key::O));
            // 'S' (Save As) or 'Ctrl+S' for save
            save = (i.key_pressed(Key::S) && !cmd) || (cmd && i.key_pressed(Key::S));
            // 'B' (Batch Dialog)
            batch = i.key_pressed(Key::B) && !cmd;

            // Clipboard Operations
            copy = cmd && i.key_pressed(Key::C);
            paste = cmd && i.key_pressed(Key::V);

            // Edit Operations
            undo = cmd && i.key_pressed(Key::Z);
            crop = cmd && i.key_pressed(Key::Y);
            resize = cmd && i.key_pressed(Key::R);
            auto_levels = shift && i.key_pressed(Key::U);

            // Single key manipulations
            if !cmd {
                if i.key_pressed(Key::R) {
                    rot_cw = true;
                }
                if i.key_pressed(Key::L) {
                    rot_ccw = true;
                }
                if i.key_pressed(Key::H) {
                    flip_h = true;
                }
                if i.key_pressed(Key::V) {
                    flip_v = true;
                }
                if i.key_pressed(Key::G) {
                    gray = true;
                }
                if i.key_pressed(Key::I) {
                    invert = true;
                }
                if i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals) {
                    zoom_in = true;
                }
                if i.key_pressed(Key::Minus) {
                    zoom_out = true;
                }
            }
        });

        if next {
            self.next_image(ctx);
        }
        if prev {
            self.prev_image(ctx);
        }
        if fit {
            self.fit_to_window = true;
        }
        if actual {
            self.fit_to_window = false;
            self.zoom = 1.0;
            self.pan = Vec2::ZERO;
        }
        if rot_cw {
            self.apply_destructive_op(|img| img.rotate90(), ctx);
        }
        if rot_ccw {
            self.apply_destructive_op(|img| img.rotate270(), ctx);
        }
        if flip_h {
            self.apply_destructive_op(|img| img.fliph(), ctx);
        }
        if flip_v {
            self.apply_destructive_op(|img| img.flipv(), ctx);
        }
        if gray {
            self.apply_destructive_op(|img| img.grayscale(), ctx);
        }
        if invert {
            if self.base_image.is_some() {
                self.push_undo();
                if let Some(ref mut base) = self.base_image {
                    base.invert();
                }
                self.update_texture(ctx);
            }
        }
        if open {
            self.open_file_dialog(ctx);
        }
        if save {
            self.save_image_as();
        }
        if copy {
            self.copy_to_clipboard_mut();
        }
        if paste {
            self.paste_from_clipboard(ctx);
        }
        if zoom_in {
            self.zoom = (self.zoom * 1.1).clamp(0.01, 100.0);
            self.fit_to_window = false;
        }
        if zoom_out {
            self.zoom = (self.zoom / 1.1).clamp(0.01, 100.0);
            self.fit_to_window = false;
        }
        if undo {
            self.undo(ctx);
        }
        if crop {
            self.crop_to_selection(ctx, image_rect, display_size);
        }
        if resize {
            if let Some(img) = &self.base_image {
                self.resize_width = img.width().to_string();
                self.resize_height = img.height().to_string();
            }
            self.show_resize_dialog = true;
        }
        if batch {
            // Set up default output dir if empty
            if self.batch_output_dir.is_empty() {
                if let Some(path) = &self.image_path {
                    if let Some(parent) = path.parent() {
                        self.batch_output_dir = parent.join("rusty_viewer_batch_output").to_string_lossy().to_string();
                    }
                } else {
                    self.batch_output_dir = std::env::current_dir()
                        .unwrap_or_default()
                        .join("rusty_viewer_batch_output")
                        .to_string_lossy()
                        .to_string();
                }
            }
            self.show_batch_dialog = true;
        }
        if auto_levels {
            if let Some(base) = self.base_image.clone() {
                self.push_undo();
                self.base_image = Some(crate::image_ops::auto_adjust(&base));
                self.update_texture(ctx);
            }
        }
    }

    /// Format file size of the active image
    fn get_image_file_size(&self) -> String {
        if let Some(path) = &self.image_path {
            if let Ok(meta) = std::fs::metadata(path) {
                let bytes = meta.len();
                const KB: u64 = 1024;
                const MB: u64 = KB * 1024;
                if bytes >= MB {
                    return format!("{:.2} MB", bytes as f64 / MB as f64);
                } else if bytes >= KB {
                    return format!("{:.1} KB", bytes as f64 / KB as f64);
                } else {
                    return format!("{} B", bytes);
                }
            }
        }
        String::new()
    }
}

impl eframe::App for RustyViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        const MOD_NAME: &str = "⌘";
        #[cfg(not(target_os = "macos"))]
        const MOD_NAME: &str = "Ctrl+";
        // Set beautiful theme presets
        let mut visuals = egui::Visuals::dark();
        visuals.window_rounding = 8.0.into();
        visuals.widgets.hovered.rounding = 4.0.into();
        visuals.widgets.active.rounding = 4.0.into();
        visuals.widgets.inactive.rounding = 4.0.into();
        ctx.set_visuals(visuals);

        // Use the image rect/display size captured from the *actual* layout in the
        // Central Panel on the previous frame. This is accurate (it accounts for the
        // adjustments side panel width and the real top bar height), unlike trying to
        // re-derive it here from ctx.available_rect(), which does not yet reflect the
        // space panels are about to claim this frame. Panel sizes are stable between
        // frames, so this is correct once the UI has rendered at least one frame.
        let image_rect = self.last_image_rect;
        let display_size = self.last_display_size;

        // Process keyboard shortcuts
        self.handle_keyboard_shortcuts(ctx, image_rect, display_size);

        // Non-blocking poll of batch background channel
        let mut clear_batch_rx = false;
        if let Some(ref rx) = self.batch_rx {
            while let Ok(progress) = rx.try_recv() {
                match progress {
                    BatchProgress::Progress(count, msg) => {
                        self.batch_progress = count;
                        self.batch_status_message = msg;
                    }
                    BatchProgress::Done(msg) => {
                        self.batch_running = false;
                        self.batch_status_message = msg;
                        clear_batch_rx = true;
                    }
                    BatchProgress::Error(msg) => {
                        self.batch_running = false;
                        self.batch_status_message = msg;
                        clear_batch_rx = true;
                    }
                }
            }
        }
        if clear_batch_rx {
            self.batch_rx = None;
        }

        // Resize Dialog Window
        if self.show_resize_dialog {
            let mut open = true;
            egui::Window::new("Resize/Resample Image")
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        if let Some(img) = &self.base_image {
                            ui.colored_label(egui::Color32::LIGHT_BLUE, format!("Original size: {} x {} pixels", img.width(), img.height()));
                        }
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            ui.label("New Width (px): ");
                            let w_edit = ui.text_edit_singleline(&mut self.resize_width);
                            if w_edit.changed() && self.keep_aspect_ratio {
                                if let (Some(img), Ok(w)) = (&self.base_image, self.resize_width.parse::<u32>()) {
                                    let aspect = img.height() as f32 / img.width() as f32;
                                    self.resize_height = ((w as f32 * aspect) as u32).to_string();
                                }
                            }
                        });
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label("New Height (px):");
                            let h_edit = ui.text_edit_singleline(&mut self.resize_height);
                            if h_edit.changed() && self.keep_aspect_ratio {
                                if let (Some(img), Ok(h)) = (&self.base_image, self.resize_height.parse::<u32>()) {
                                    let aspect = img.width() as f32 / img.height() as f32;
                                    self.resize_width = ((h as f32 * aspect) as u32).to_string();
                                }
                            }
                        });
                        ui.add_space(6.0);

                        ui.checkbox(&mut self.keep_aspect_ratio, "Keep Aspect Ratio");
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            if ui.button("Apply (OK)").clicked() {
                                if let (Ok(w), Ok(h)) = (self.resize_width.parse::<u32>(), self.resize_height.parse::<u32>()) {
                                    if w > 0 && h > 0 {
                                        self.push_undo();
                                        if let Some(img) = &self.base_image {
                                            self.base_image = Some(crate::image_ops::resize_image(img, w, h));
                                            self.update_texture(ctx);
                                        }
                                    }
                                }
                                self.show_resize_dialog = false;
                            }
                            if ui.button("Cancel").clicked() {
                                self.show_resize_dialog = false;
                            }
                        });
                    });
                });
            if !open {
                self.show_resize_dialog = false;
            }
        }

        // Batch Processing Dialog Window
        if self.show_batch_dialog {
            let mut open = true;
            egui::Window::new("Batch Conversion / Rename")
                .open(&mut open)
                .default_width(700.0)
                .default_height(450.0)
                .show(ctx, |ui| {
                    ui.columns(2, |columns| {
                        // Left Column: Inputs list
                        columns[0].vertical(|ui| {
                            ui.heading("Input Images");
                            ui.small("Drag & drop files here to add");
                            ui.add_space(4.0);

                            egui::ScrollArea::vertical()
                                .max_height(240.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if self.batch_input_files.is_empty() {
                                        ui.colored_label(egui::Color32::GRAY, "(No files added yet)");
                                    } else {
                                        let mut remove_idx = None;
                                        for (idx, path) in self.batch_input_files.iter().enumerate() {
                                            ui.horizontal(|ui| {
                                                ui.label(format!("{}.", idx + 1));
                                                ui.small(path.file_name().unwrap_or_default().to_string_lossy());
                                                if ui.small_button("❌").clicked() {
                                                    remove_idx = Some(idx);
                                                }
                                            });
                                        }
                                        if let Some(idx) = remove_idx {
                                            self.batch_input_files.remove(idx);
                                        }
                                    }
                                });
                            
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui.button("Add Files...").clicked() && !self.batch_running {
                                    if let Some(paths) = rfd::FileDialog::new()
                                        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff", "tif", "ico"])
                                        .pick_files() {
                                            self.batch_input_files.extend(paths);
                                        }
                                }
                                if ui.button("Clear List").clicked() && !self.batch_running {
                                    self.batch_input_files.clear();
                                }
                            });
                        });

                        // Right Column: Output controls and Filters
                        columns[1].vertical(|ui| {
                            ui.heading("Batch Settings");
                            ui.add_space(4.0);

                            ui.horizontal(|ui| {
                                ui.label("Output Format:");
                                egui::ComboBox::from_label("")
                                    .selected_text(&self.batch_output_format)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.batch_output_format, "PNG".to_string(), "PNG");
                                        ui.selectable_value(&mut self.batch_output_format, "JPEG".to_string(), "JPEG");
                                        ui.selectable_value(&mut self.batch_output_format, "WebP".to_string(), "WebP");
                                        ui.selectable_value(&mut self.batch_output_format, "BMP".to_string(), "BMP");
                                    });
                            });
                            ui.add_space(6.0);

                            ui.label("Output Directory:");
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.batch_output_dir);
                                if ui.button("Browse...").clicked() && !self.batch_running {
                                    let mut dialog = rfd::FileDialog::new();
                                    if !self.batch_output_dir.is_empty() {
                                        dialog = dialog.set_directory(&self.batch_output_dir);
                                    }
                                    if let Some(dir) = dialog.pick_folder() {
                                        self.batch_output_dir = dir.to_string_lossy().to_string();
                                        crate::settings::save_last_batch_output_dir(&self.batch_output_dir);
                                    }
                                }
                            });

                            ui.separator();
                            ui.label("Batch Operations:");
                            
                            ui.checkbox(&mut self.batch_resize, "Resize");
                            if self.batch_resize {
                                ui.horizontal(|ui| {
                                    ui.label("W:");
                                    ui.add(egui::TextEdit::singleline(&mut self.batch_resize_w).desired_width(50.0));
                                    ui.label("H:");
                                    ui.add_enabled(
                                        !self.batch_keep_aspect_ratio,
                                        egui::TextEdit::singleline(&mut self.batch_resize_h).desired_width(50.0),
                                    );
                                });
                                ui.checkbox(&mut self.batch_keep_aspect_ratio, "Keep Aspect Ratio");
                                if self.batch_keep_aspect_ratio {
                                    ui.small("Height is computed per-image from its own original aspect ratio.");
                                }
                            }

                            ui.checkbox(&mut self.batch_rotate, "Rotate");
                            if self.batch_rotate {
                                egui::ComboBox::from_label("Angle")
                                    .selected_text(format!("{}°", self.batch_rotate_angle))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.batch_rotate_angle, 90, "90° CW");
                                        ui.selectable_value(&mut self.batch_rotate_angle, 180, "180°");
                                        ui.selectable_value(&mut self.batch_rotate_angle, 270, "90° CCW");
                                    });
                            }

                            ui.checkbox(&mut self.batch_grayscale, "Grayscale");
                            ui.checkbox(&mut self.batch_invert, "Invert Colors");
                            ui.checkbox(&mut self.batch_auto_adjust, "Auto-Adjust levels");
                        });
                    });

                    ui.separator();

                    ui.horizontal(|ui| {
                        if self.batch_running {
                            ui.add(egui::ProgressBar::new(self.batch_progress as f32 / self.batch_total as f32)
                                .text(format!("{}/{}", self.batch_progress, self.batch_total)));
                        } else {
                            if ui.button("🚀 Start Batch").clicked() && !self.batch_input_files.is_empty() && !self.batch_output_dir.is_empty() {
                                crate::settings::save_last_batch_output_dir(&self.batch_output_dir);

                                self.batch_running = true;
                                self.batch_progress = 0;
                                self.batch_total = self.batch_input_files.len();
                                self.batch_status_message = "Starting batch job...".to_string();

                                let files = self.batch_input_files.clone();
                                let out_dir = PathBuf::from(&self.batch_output_dir);
                                let format = self.batch_output_format.clone();

                                let resize = self.batch_resize;
                                let r_w = self.batch_resize_w.parse::<u32>().unwrap_or(800);
                                let r_h = self.batch_resize_h.parse::<u32>().unwrap_or(600);
                                let keep_aspect = self.batch_keep_aspect_ratio;

                                let rotate = self.batch_rotate;
                                let r_angle = self.batch_rotate_angle;
                                let gray = self.batch_grayscale;
                                let invert = self.batch_invert;
                                let auto_levels = self.batch_auto_adjust;

                                let (tx, rx) = std::sync::mpsc::channel();
                                self.batch_rx = Some(rx);

                                // Spawn worker thread to execute batch process safely without locking GUI
                                std::thread::spawn(move || {
                                    let _ = std::fs::create_dir_all(&out_dir);

                                    for (index, file_path) in files.iter().enumerate() {
                                        let name = file_path.file_name().unwrap_or_default().to_string_lossy().to_string();
                                        let _ = tx.send(BatchProgress::Progress(index, format!("Processing: {}", name)));

                                        match image::open(file_path) {
                                            Ok(mut img) => {
                                                if resize {
                                                    let (target_w, target_h) = if keep_aspect {
                                                        let (ow, oh) = (img.width().max(1), img.height().max(1));
                                                        let computed_h = ((r_w as f32) * (oh as f32 / ow as f32)).round().max(1.0) as u32;
                                                        (r_w, computed_h)
                                                    } else {
                                                        (r_w, r_h)
                                                    };
                                                    img = crate::image_ops::resize_image(&img, target_w, target_h);
                                                }
                                                if rotate {
                                                    match r_angle {
                                                        90 => img = img.rotate90(),
                                                        180 => img = img.rotate180(),
                                                        270 => img = img.rotate270(),
                                                        _ => {}
                                                    }
                                                }
                                                if gray {
                                                    img = img.grayscale();
                                                }
                                                if invert {
                                                    img.invert();
                                                }
                                                if auto_levels {
                                                    img = crate::image_ops::auto_adjust(&img);
                                                }

                                                let ext = format.to_lowercase();
                                                let out_filename = file_path
                                                    .file_stem()
                                                    .map(|s| format!("{}.{}", s.to_string_lossy(), ext))
                                                    .unwrap_or_else(|| format!("batch_{}.{}", index, ext));
                                                
                                                let save_path = out_dir.join(out_filename);
                                                if let Err(e) = img.save(&save_path) {
                                                    let _ = tx.send(BatchProgress::Error(format!("Error saving {}: {:?}", name, e)));
                                                    return;
                                                }
                                            }
                                            Err(e) => {
                                                let _ = tx.send(BatchProgress::Error(format!("Error opening {}: {:?}", name, e)));
                                                return;
                                            }
                                        }
                                    }

                                    let _ = tx.send(BatchProgress::Done(format!("Successfully processed {} images!", files.len())));
                                });
                            }
                        }
                        ui.label(&self.batch_status_message);
                    });
                });
            if !open {
                self.show_batch_dialog = false;
            }
        }

        // Top Menu Bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.add(egui::Button::new("Open File...").shortcut_text(format!("{}O", MOD_NAME))).clicked() {
                        self.open_file_dialog(ctx);
                        ui.close_menu();
                    }
                    if ui.add_enabled(
                        self.adjusted_image.is_some(),
                        egui::Button::new("Save As...").shortcut_text(format!("{}S", MOD_NAME))
                    ).clicked() {
                        self.save_image_as();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add(egui::Button::new("Batch Conversion/Rename...").shortcut_text("B")).clicked() {
                        if self.batch_output_dir.is_empty() {
                            if let Some(path) = &self.image_path {
                                if let Some(parent) = path.parent() {
                                    self.batch_output_dir = parent.join("rusty_viewer_batch_output").to_string_lossy().to_string();
                                }
                            } else {
                                self.batch_output_dir = std::env::current_dir()
                                    .unwrap_or_default()
                                    .join("rusty_viewer_batch_output")
                                    .to_string_lossy()
                                    .to_string();
                            }
                        }
                        self.show_batch_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add(egui::Button::new("Paste from Clipboard").shortcut_text(format!("{}V", MOD_NAME))).clicked() {
                        self.paste_from_clipboard(ctx);
                        ui.close_menu();
                    }
                    if ui.add_enabled(
                        self.adjusted_image.is_some(),
                        egui::Button::new("Copy to Clipboard").shortcut_text(format!("{}C", MOD_NAME))
                    ).clicked() {
                        self.copy_to_clipboard_mut();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Edit", |ui| {
                    let has_img = self.base_image.is_some();
                    let has_undo = !self.undo_stack.is_empty();
                    
                    if ui.add_enabled(
                        has_undo,
                        egui::Button::new("Undo").shortcut_text(format!("{}Z", MOD_NAME))
                    ).clicked() {
                        self.undo(ctx);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add_enabled(
                        has_img && self.selection_rect.is_some(),
                        egui::Button::new("Crop Selection").shortcut_text(format!("{}Y", MOD_NAME))
                    ).clicked() {
                        self.crop_to_selection(ctx, image_rect, display_size);
                        ui.close_menu();
                    }
                    if ui.add_enabled(
                        has_img,
                        egui::Button::new("Resize/Resample...").shortcut_text(format!("{}R", MOD_NAME))
                    ).clicked() {
                        if let Some(img) = &self.base_image {
                            self.resize_width = img.width().to_string();
                            self.resize_height = img.height().to_string();
                        }
                        self.show_resize_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add_enabled(has_img, egui::Button::new("Rotate 90° CW").shortcut_text("R")).clicked() {
                        self.apply_destructive_op(|img| img.rotate90(), ctx);
                        ui.close_menu();
                    }
                    if ui.add_enabled(has_img, egui::Button::new("Rotate 90° CCW").shortcut_text("L")).clicked() {
                        self.apply_destructive_op(|img| img.rotate270(), ctx);
                        ui.close_menu();
                    }
                    if ui.add_enabled(has_img, egui::Button::new("Rotate 180°")).clicked() {
                        self.apply_destructive_op(|img| img.rotate180(), ctx);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add_enabled(has_img, egui::Button::new("Flip Horizontal").shortcut_text("H")).clicked() {
                        self.apply_destructive_op(|img| img.fliph(), ctx);
                        ui.close_menu();
                    }
                    if ui.add_enabled(has_img, egui::Button::new("Flip Vertical").shortcut_text("V")).clicked() {
                        self.apply_destructive_op(|img| img.flipv(), ctx);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add_enabled(has_img, egui::Button::new("Convert to Grayscale").shortcut_text("G")).clicked() {
                        self.apply_destructive_op(|img| img.grayscale(), ctx);
                        ui.close_menu();
                    }
                    if ui.add_enabled(has_img, egui::Button::new("Invert Colors").shortcut_text("I")).clicked() {
                        self.push_undo();
                        if let Some(ref mut base) = self.base_image {
                            base.invert();
                        }
                        self.update_texture(ctx);
                        ui.close_menu();
                    }
                    if ui.add_enabled(
                        has_img,
                        egui::Button::new("Auto-Adjust Colors").shortcut_text("Shift+U")
                    ).clicked() {
                        if let Some(base) = self.base_image.clone() {
                            self.push_undo();
                            self.base_image = Some(crate::image_ops::auto_adjust(&base));
                            self.update_texture(ctx);
                        }
                        ui.close_menu();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.add(egui::Button::new("Zoom In").shortcut_text("+")).clicked() {
                        self.zoom = (self.zoom * 1.1).clamp(0.01, 100.0);
                        self.fit_to_window = false;
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new("Zoom Out").shortcut_text("-")).clicked() {
                        self.zoom = (self.zoom / 1.1).clamp(0.01, 100.0);
                        self.fit_to_window = false;
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new("Fit to Window").shortcut_text("F")).clicked() {
                        self.fit_to_window = true;
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new("Actual Size 100%").shortcut_text("1")).clicked() {
                        self.fit_to_window = false;
                        self.zoom = 1.0;
                        self.pan = Vec2::ZERO;
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.checkbox(&mut self.show_adjustments, "Show Adjustments Panel");
                });
            });
        });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(path) = &self.image_path {
                    let filename = path.file_name().unwrap_or_default().to_string_lossy();
                    ui.label(format!("📁 {}", filename));
                    
                    let size_str = self.get_image_file_size();
                    if !size_str.is_empty() {
                        ui.separator();
                        ui.label(size_str);
                    }
                } else if self.base_image.is_some() {
                    ui.label("📁 [Clipboard Image]");
                } else {
                    ui.label("No file loaded");
                }

                if let Some(base) = &self.base_image {
                    ui.separator();
                    ui.label(format!("📐 {} x {}", base.width(), base.height()));

                    ui.separator();
                    ui.label(format!("🔍 Zoom: {:.0}%", self.zoom * 100.0));

                    if !self.siblings.is_empty() {
                        ui.separator();
                        ui.label(format!("📄 [{}/{}]", self.sibling_index + 1, self.siblings.len()));
                    }

                    if self.selection_rect.is_some() {
                        ui.separator();
                        ui.colored_label(egui::Color32::LIGHT_GREEN, format!("Selection active ({}Y to crop)", MOD_NAME));
                    }
                }
            });
        });

        // Right Adjustments Panel (Sliders)
        if self.show_adjustments && self.base_image.is_some() {
            egui::SidePanel::right("adjustments_panel")
                .resizable(false)
                .default_width(230.0)
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        ui.add_space(4.0);
                        ui.heading("Adjustments");
                        ui.separator();

                        let mut changed = false;

                        ui.label("Brightness");
                        if ui.add(egui::Slider::new(&mut self.brightness, -1.0..=1.0).show_value(true)).changed() {
                            changed = true;
                        }
                        ui.add_space(4.0);

                        ui.label("Contrast");
                        if ui.add(egui::Slider::new(&mut self.contrast, -1.0..=1.0).show_value(true)).changed() {
                            changed = true;
                        }
                        ui.add_space(4.0);

                        ui.label("Saturation");
                        if ui.add(egui::Slider::new(&mut self.saturation, -1.0..=1.0).show_value(true)).changed() {
                            changed = true;
                        }
                        ui.add_space(4.0);

                        ui.label("Gamma");
                        if ui.add(egui::Slider::new(&mut self.gamma, 0.1..=3.0).show_value(true)).changed() {
                            changed = true;
                        }
                        ui.add_space(8.0);

                        ui.colored_label(egui::Color32::GRAY, "Color Tint Bias");
                        ui.horizontal(|ui| {
                            ui.label("R:");
                            if ui.add(egui::Slider::new(&mut self.r_tint, -1.0..=1.0).show_value(false)).changed() {
                                changed = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("G:");
                            if ui.add(egui::Slider::new(&mut self.g_tint, -1.0..=1.0).show_value(false)).changed() {
                                changed = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("B:");
                            if ui.add(egui::Slider::new(&mut self.b_tint, -1.0..=1.0).show_value(false)).changed() {
                                changed = true;
                            }
                        });

                        if changed {
                            self.update_texture(ctx);
                        }

                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui.button("Reset Sliders").clicked() {
                                self.brightness = 0.0;
                                self.contrast = 0.0;
                                self.saturation = 0.0;
                                self.gamma = 1.0;
                                self.r_tint = 0.0;
                                self.g_tint = 0.0;
                                self.b_tint = 0.0;
                                self.update_texture(ctx);
                            }
                            if ui.add_enabled(!self.undo_stack.is_empty(), egui::Button::new("↩ Undo")).clicked() {
                                self.undo(ctx);
                            }
                        });

                        ui.add_space(16.0);
                        ui.heading("Quick Actions");
                        ui.separator();

                        ui.horizontal(|ui| {
                            if ui.button("⟲ CCW").clicked() {
                                self.apply_destructive_op(|img| img.rotate270(), ctx);
                            }
                            if ui.button("⟳ CW").clicked() {
                                self.apply_destructive_op(|img| img.rotate90(), ctx);
                            }
                        });

                        ui.horizontal(|ui| {
                            if ui.button("Flip ↔").clicked() {
                                self.apply_destructive_op(|img| img.fliph(), ctx);
                            }
                            if ui.button("Flip ↕").clicked() {
                                self.apply_destructive_op(|img| img.flipv(), ctx);
                            }
                        });

                        ui.horizontal(|ui| {
                            if ui.button("Grayscale").clicked() {
                                self.apply_destructive_op(|img| img.grayscale(), ctx);
                            }
                            if ui.button("Invert").clicked() {
                                self.push_undo();
                                if let Some(ref mut base) = self.base_image {
                                    base.invert();
                                }
                                self.update_texture(ctx);
                            }
                        });

                        if ui.button("Auto-Adjust Colors").clicked() {
                            if let Some(base) = self.base_image.clone() {
                                self.push_undo();
                                self.base_image = Some(crate::image_ops::auto_adjust(&base));
                                self.update_texture(ctx);
                            }
                        }

                        ui.add_space(16.0);
                        ui.heading("Information");
                        ui.separator();
                        
                        if let Some(path) = &self.image_path {
                            ui.label("Path:");
                            ui.small(path.to_string_lossy());
                        } else {
                            ui.label("Source: Clipboard");
                        }
                    });
                });
        }

        // Central Panel for rendering the image
        egui::CentralPanel::default().show(ctx, |ui| {
            // Drag-and-drop listener that handles files dropped anywhere on the window
            let mut dropped_files = Vec::new();
            ctx.input(|i| {
                for file in &i.raw.dropped_files {
                    if let Some(path) = &file.path {
                        dropped_files.push(path.clone());
                    }
                }
            });

            if !dropped_files.is_empty() {
                if self.show_batch_dialog {
                    for path in dropped_files {
                        if crate::navigation::is_supported_image(&path) {
                            self.batch_input_files.push(path);
                        }
                    }
                } else if let Some(path) = dropped_files.first() {
                    self.load_image(path.clone(), ctx);
                }
            }

            if let Some(texture) = &self.texture {
                let tex_size = texture.size_vec2();
                let available_size = ui.available_size();

                if self.fit_to_window {
                    let zoom_x = available_size.x / tex_size.x;
                    let zoom_y = available_size.y / tex_size.y;
                    self.zoom = zoom_x.min(zoom_y);
                    self.pan = Vec2::ZERO;
                }

                let display_size = tex_size * self.zoom;

                // Allocate standard layout interaction space
                let (rect, response) = ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());

                // Scroll wheel zooming
                if response.hovered() {
                    ui.input(|i| {
                        let scroll = i.smooth_scroll_delta.y;
                        if scroll != 0.0 {
                            let zoom_factor = (scroll * 0.0015).exp();
                            let old_zoom = self.zoom;
                            self.zoom = (self.zoom * zoom_factor).clamp(0.01, 100.0);
                            self.fit_to_window = false;

                            // Zoom centered at mouse position
                            if let Some(mouse_pos) = i.pointer.hover_pos() {
                                let center = rect.center();
                                let to_mouse = mouse_pos - center - self.pan;
                                self.pan -= to_mouse * (self.zoom / old_zoom - 1.0);
                            }
                        }
                    });
                }

                // Drag handling:
                // - Left Drag (Primary) -> Selection box for crop
                // - Right/Middle Drag (Secondary) -> Panning
                if response.dragged_by(egui::PointerButton::Secondary) || response.dragged_by(egui::PointerButton::Middle) {
                    self.pan += response.drag_delta();
                    self.fit_to_window = false;
                } else if response.dragged_by(egui::PointerButton::Primary) {
                    if let Some(start) = response.interact_pointer_pos() {
                        if self.selection_start.is_none() {
                            self.selection_start = Some(start - response.drag_delta());
                        }
                        if let Some(current) = response.hover_pos() {
                            self.selection_rect = Some(Rect::from_two_pos(self.selection_start.unwrap(), current));
                        }
                    }
                }

                // If user clicks without dragging, clear selection box
                if response.clicked_by(egui::PointerButton::Primary) && !response.dragged_by(egui::PointerButton::Primary) {
                    self.selection_rect = None;
                    self.selection_start = None;
                }

                // Reset selection start on drag release
                if response.drag_stopped_by(egui::PointerButton::Primary) {
                    self.selection_start = None;
                }

                // Double click toggles fit-to-window
                if response.double_clicked() {
                    self.fit_to_window = !self.fit_to_window;
                    if !self.fit_to_window {
                        self.zoom = 1.0;
                        self.pan = Vec2::ZERO;
                    }
                }

                // Draw the image texture centered + panned
                let center = rect.center() + self.pan;
                let final_image_rect = egui::Rect::from_center_size(center, display_size);

                // Cache the true, post-layout image rect/size so crop math (which may
                // run before this panel is laid out next frame) uses accurate coordinates.
                self.last_image_rect = final_image_rect;
                self.last_display_size = display_size;

                ui.painter().image(
                    texture.id(),
                    final_image_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );

                // Overlay the crop selection box if present
                if let Some(sel_box) = self.selection_rect {
                    let stroke = egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(0, 130, 240));
                    ui.painter().rect_stroke(sel_box, 0.0, stroke);

                    let fill = egui::Color32::from_rgba_unmultiplied(0, 130, 240, 25);
                    ui.painter().rect_filled(sel_box, 0.0, fill);
                }

            } else {
                // Placeholder/Welcome screen when empty
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("RustyViewer");
                        ui.label("Drag & drop an image here to view");
                        ui.add_space(10.0);
                        
                        ui.vertical_centered(|ui| {
                            let button_size = egui::vec2(200.0, 32.0);
                            if ui.add_sized(button_size, egui::Button::new("📁 Open File...")).clicked() {
                                self.open_file_dialog(ctx);
                            }
                            ui.add_space(8.0);
                            if self.clipboard.is_some() {
                                if ui.add_sized(button_size, egui::Button::new("📋 Paste Image")).clicked() {
                                    self.paste_from_clipboard(ctx);
                                }
                                ui.add_space(8.0);
                            }
                            if ui.add_sized(button_size, egui::Button::new("🚀 Batch Conversion (B)")).clicked() {
                                self.show_batch_dialog = true;
                            }
                        });
                    });
                });
            }
        });
    }
}