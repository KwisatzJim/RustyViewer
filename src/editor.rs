//! Image documents and file operations, independent of the desktop interface.
use crate::{image_ops, navigation};
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    io::Cursor,
    path::{Path, PathBuf},
    sync::Arc,
};

const MAX_PIXELS: u64 = 80_000_000;
const HISTORY_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Adjustments {
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub gamma: f32,
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}
impl Default for Adjustments {
    fn default() -> Self {
        Self {
            brightness: 0.,
            contrast: 0.,
            saturation: 0.,
            gamma: 1.,
            red: 0.,
            green: 0.,
            blue: 0.,
        }
    }
}
impl Adjustments {
    pub fn validate(&self) -> Result<(), String> {
        for v in [
            self.brightness,
            self.contrast,
            self.saturation,
            self.red,
            self.green,
            self.blue,
        ] {
            if !v.is_finite() || !(-1.0..=1.0).contains(&v) {
                return Err("Adjustment must be between -1 and 1.".into());
            }
        }
        if !self.gamma.is_finite() || !(0.1..=3.).contains(&self.gamma) {
            return Err("Gamma must be between 0.1 and 3.".into());
        }
        Ok(())
    }
    pub fn apply(&self, image: &DynamicImage) -> DynamicImage {
        image_ops::apply_adjustments(
            image,
            self.brightness,
            self.contrast,
            self.saturation,
            self.gamma,
            [self.red, self.green, self.blue],
        )
    }
}

#[derive(Clone)]
struct Revision {
    image: Arc<DynamicImage>,
    adjustments: Adjustments,
    id: u64,
}
#[derive(Default)]
pub struct Editor {
    current: Option<Revision>,
    undo: Vec<Revision>,
    redo: Vec<Revision>,
    path: Option<PathBuf>,
    siblings: Vec<PathBuf>,
    index: usize,
    next_id: u64,
    saved_id: u64,
}
#[derive(Serialize)]
pub struct View {
    pub name: String,
    pub path: Option<String>,
    pub width: u32,
    pub height: u32,
    pub preview: String,
    pub adjustments: Adjustments,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub siblings: Vec<String>,
    pub index: usize,
    pub bytes: u64,
}
pub fn dimensions(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err("Dimensions must be positive and no larger than 80 megapixels.".into());
    }
    Ok(())
}
pub fn decode(path: &Path) -> Result<DynamicImage, String> {
    let reader = ImageReader::open(path)
        .map_err(|e| format!("Cannot open {}: {e}", path.display()))?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("Cannot decode {}: {e}", path.display()))?;
    let (w, h) = decoder.dimensions();
    dimensions(w, h)?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    image.apply_orientation(orientation);
    Ok(image)
}
impl Editor {
    fn current(&self) -> Result<&Revision, String> {
        self.current
            .as_ref()
            .ok_or_else(|| "Open an image first.".into())
    }
    pub fn dirty(&self) -> bool {
        self.current.as_ref().is_some_and(|r| r.id != self.saved_id)
    }
    pub fn open(&mut self, path: PathBuf, discard: bool) -> Result<View, String> {
        if self.dirty() && !discard {
            return Err("Export or discard your changes before opening another image.".into());
        }
        let image = decode(&path)?;
        let path = path.canonicalize().map_err(|e| e.to_string())?;
        let (siblings, index) = navigation::get_image_siblings(&path);
        self.replace(image, Some(path));
        self.siblings = siblings;
        self.index = index;
        self.view()
    }
    fn replace(&mut self, image: DynamicImage, path: Option<PathBuf>) {
        self.next_id += 1;
        self.saved_id = if path.is_some() { self.next_id } else { 0 };
        self.current = Some(Revision {
            image: Arc::new(image),
            adjustments: Adjustments::default(),
            id: self.next_id,
        });
        self.path = path;
        self.undo.clear();
        self.redo.clear();
        self.siblings.clear();
        self.index = 0;
    }
    pub fn paste(&mut self, image: DynamicImage, discard: bool) -> Result<View, String> {
        if self.dirty() && !discard {
            return Err("Export or discard your changes first.".into());
        }
        dimensions(image.width(), image.height())?;
        self.replace(image, None);
        self.view()
    }
    pub fn rendered(&self) -> Result<DynamicImage, String> {
        let r = self.current()?;
        Ok(r.adjustments.apply(&r.image))
    }
    fn commit(&mut self, image: Arc<DynamicImage>, adjustments: Adjustments) -> Result<(), String> {
        let old = self.current()?.clone();
        self.undo.push(old);
        self.redo.clear();
        self.next_id += 1;
        self.current = Some(Revision {
            image,
            adjustments,
            id: self.next_id,
        });
        // Bound pixel history by bytes as well as count. Arc shares pixels for slider-only edits.
        while self.undo.len() > 1
            && (self.undo.len() > 20
                || self
                    .undo
                    .iter()
                    .map(|r| r.image.as_bytes().len())
                    .sum::<usize>()
                    > HISTORY_BYTES)
        {
            self.undo.remove(0);
        }
        Ok(())
    }
    pub fn adjust(&mut self, value: Adjustments) -> Result<View, String> {
        value.validate()?;
        let r = self.current()?;
        if r.adjustments != value {
            self.commit(r.image.clone(), value)?;
        }
        self.view()
    }
    pub fn edit(&mut self, action: &str, args: &[u32]) -> Result<View, String> {
        if action == "undo" {
            if let Some(previous) = self.undo.pop() {
                let old = self.current.replace(previous).unwrap();
                self.redo.push(old);
            }
        } else if action == "redo" {
            if let Some(next) = self.redo.pop() {
                let old = self.current.replace(next).unwrap();
                self.undo.push(old);
            }
        } else {
            // Bake the visible slider result before pixel operations, so undo restores exactly what was seen.
            let image = self.rendered()?;
            let image = match action {
                "rotate_right" => image.rotate90(),
                "rotate_left" => image.rotate270(),
                "rotate_180" => image.rotate180(),
                "flip_horizontal" => image.fliph(),
                "flip_vertical" => image.flipv(),
                "grayscale" => image.grayscale(),
                "invert" => {
                    let mut image = image;
                    image.invert();
                    image
                }
                "auto" => image_ops::auto_adjust(&image),
                "resize" if args.len() == 2 => {
                    dimensions(args[0], args[1])?;
                    image_ops::resize_image(&image, args[0], args[1])
                }
                "crop" if args.len() == 4 => {
                    if args[0] >= image.width()
                        || args[1] >= image.height()
                        || args[2] == 0
                        || args[3] == 0
                    {
                        return Err("Choose a crop area inside the image.".into());
                    }
                    image_ops::crop_image(&image, args[0], args[1], args[2], args[3])
                }
                _ => return Err("Unknown image operation or invalid dimensions.".into()),
            };
            self.commit(Arc::new(image), Adjustments::default())?;
        }
        self.view()
    }
    pub fn view(&self) -> Result<View, String> {
        let r = self.current()?;
        let preview = r.adjustments.apply(&r.image);
        let mut bytes = Cursor::new(Vec::new());
        preview
            .write_to(&mut bytes, ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(View {
            name: self
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Clipboard image".into()),
            path: self.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
            width: r.image.width(),
            height: r.image.height(),
            preview: format!(
                "data:image/png;base64,{}",
                STANDARD.encode(bytes.into_inner())
            ),
            adjustments: r.adjustments,
            dirty: self.dirty(),
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
            siblings: self
                .siblings
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            index: self.index,
            bytes: self
                .path
                .as_ref()
                .and_then(|p| p.metadata().ok())
                .map(|m| m.len())
                .unwrap_or(0),
        })
    }
    pub fn export(&mut self, path: &Path) -> Result<(), String> {
        save(&self.rendered()?, path, true)?;
        self.saved_id = self.current()?.id;
        Ok(())
    }
}

fn format(path: &Path) -> Result<ImageFormat, String> {
    let format = ImageFormat::from_path(path)
        .map_err(|_| "Choose a .png, .jpg, .webp, .bmp or .tiff filename.".to_string())?;
    if !matches!(
        format,
        ImageFormat::Png
            | ImageFormat::Jpeg
            | ImageFormat::WebP
            | ImageFormat::Bmp
            | ImageFormat::Tiff
    ) {
        return Err("Export supports PNG, JPEG, WebP, BMP and TIFF.".into());
    }
    Ok(format)
}
pub fn save(image: &DynamicImage, path: &Path, overwrite: bool) -> Result<(), String> {
    let format = format(path)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    // JPEG has no alpha channel. Composite transparency onto white rather than failing or turning black.
    if format == ImageFormat::Jpeg {
        let rgba = image.to_rgba8();
        let rgb = image::RgbImage::from_fn(image.width(), image.height(), |x, y| {
            let p = rgba.get_pixel(x, y);
            let a = u32::from(p[3]);
            image::Rgb(
                [0, 1, 2].map(|c| ((u32::from(p[c]) * a + 255 * (255 - a) + 127) / 255) as u8),
            )
        });
        DynamicImage::ImageRgb8(rgb)
            .write_to(temp.as_file_mut(), format)
            .map_err(|e| e.to_string())?;
    } else {
        image
            .write_to(temp.as_file_mut(), format)
            .map_err(|e| e.to_string())?;
    }
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    if overwrite {
        temp.persist(path).map_err(|e| e.to_string())?;
    } else {
        temp.persist_noclobber(path)
            .map_err(|e| format!("Output already exists or cannot be written: {e}"))?;
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct BatchOptions {
    pub files: Vec<PathBuf>,
    pub directory: PathBuf,
    pub format: String,
    pub resize: Option<[u32; 2]>,
    pub keep_aspect: bool,
    pub rotation: u16,
    pub grayscale: bool,
    pub invert: bool,
    pub auto: bool,
}
#[derive(Clone, Serialize)]
pub struct BatchResult {
    pub completed: usize,
    pub total: usize,
    pub written: usize,
    pub errors: Vec<String>,
}
pub fn batch(
    options: BatchOptions,
    mut progress: impl FnMut(&BatchResult),
) -> Result<BatchResult, String> {
    if options.files.is_empty() {
        return Err("Add at least one image.".into());
    }
    if !options.directory.is_dir() {
        return Err("Choose an existing output folder.".into());
    }
    if ![0, 90, 180, 270].contains(&options.rotation) {
        return Err("Invalid rotation.".into());
    }
    if let Some([w, h]) = options.resize {
        dimensions(w, h)?;
    }
    let ext = match options.format.as_str() {
        "png" => "png",
        "jpg" => "jpg",
        "webp" => "webp",
        "bmp" => "bmp",
        "tiff" => "tiff",
        _ => return Err("Unsupported batch format.".into()),
    };
    let mut seen = HashSet::new();
    let mut result = BatchResult {
        completed: 0,
        total: options.files.len(),
        written: 0,
        errors: vec![],
    };
    for input in &options.files {
        let outcome = (|| {
            let stem = input.file_stem().ok_or("Invalid input filename.")?;
            let mut name = stem.to_os_string();
            name.push(".");
            name.push(ext);
            let output = options.directory.join(name);
            // Reserve destinations before decoding; no batch file may overwrite any existing file.
            if !seen.insert(output.to_string_lossy().to_lowercase()) || output.exists() {
                return Err(format!(
                    "{}: output exists or conflicts with another input; skipped",
                    input.display()
                ));
            }
            let mut image = decode(input)?;
            if let Some([w, h]) = options.resize {
                image = if options.keep_aspect {
                    image.resize(w, h, image::imageops::FilterType::Lanczos3)
                } else {
                    image_ops::resize_image(&image, w, h)
                };
            }
            image = match options.rotation {
                90 => image.rotate90(),
                180 => image.rotate180(),
                270 => image.rotate270(),
                _ => image,
            };
            if options.grayscale {
                image = image.grayscale();
            }
            if options.invert {
                image.invert();
            }
            if options.auto {
                image = image_ops::auto_adjust(&image);
            }
            save(&image, &output, false)
        })();
        match outcome {
            Ok(()) => result.written += 1,
            Err(e) => result.errors.push(format!("{}: {e}", input.display())),
        }
        result.completed += 1;
        progress(&result);
    }
    Ok(result)
}
