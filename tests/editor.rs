use image::{DynamicImage, Rgba, RgbaImage};
use rusty_viewer::{
    editor::{self, Adjustments, BatchOptions, Editor},
    image_ops, navigation,
};
use std::{fs, path::Path};
fn fixture(path: &Path) {
    DynamicImage::ImageRgba8(RgbaImage::from_fn(8, 6, |x, y| {
        Rgba([
            (x * 25) as u8,
            (y * 30) as u8,
            110,
            if x == 0 { 0 } else { 180 },
        ])
    }))
    .save(path)
    .unwrap();
}
#[test]
fn crop_outside_image_never_panics_or_underflows() {
    let image = DynamicImage::new_rgba8(4, 4);
    for (x, y) in [(4, 0), (0, 4), (u32::MAX, u32::MAX)] {
        let out = image_ops::crop_image(&image, x, y, 2, 2);
        assert_eq!(out.as_bytes(), image.as_bytes());
    }
    let cropped = image_ops::crop_image(&image, 3, 3, u32::MAX, u32::MAX);
    assert_eq!((cropped.width(), cropped.height()), (1, 1));
}
#[test]
fn undo_restores_sliders_and_pixels_and_redo_restores_edit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.png");
    fixture(&path);
    let mut editor = Editor::default();
    editor.open(path, false).unwrap();
    let adjusted = Adjustments {
        brightness: 0.2,
        gamma: 1.3,
        ..Default::default()
    };
    editor.adjust(adjusted).unwrap();
    let before = editor.rendered().unwrap();
    editor.edit("rotate_right", &[]).unwrap();
    let undo = editor.edit("undo", &[]).unwrap();
    assert_eq!(undo.adjustments, adjusted);
    assert_eq!(before.as_bytes(), editor.rendered().unwrap().as_bytes());
    let redo = editor.edit("redo", &[]).unwrap();
    assert_eq!((redo.width, redo.height), (6, 8));
    assert_eq!(redo.adjustments, Adjustments::default());
}
#[test]
fn failure_preserves_document_and_edits_require_discard() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.png");
    fixture(&path);
    let mut editor = Editor::default();
    editor.open(path.clone(), false).unwrap();
    editor.edit("invert", &[]).unwrap();
    assert!(editor.open(path, false).is_err());
    assert!(editor.open(dir.path().join("missing.png"), true).is_err());
    assert!(editor.dirty());
    assert_eq!(editor.view().unwrap().name, "test.png");
    assert!(editor.edit("resize", &[0, 20]).is_err());
    assert!(editor.edit("resize", &[u32::MAX, u32::MAX]).is_err());
    assert!(editor.edit("crop", &[99, 0, 1, 1]).is_err());
}
#[test]
fn jpeg_exports_rgba_with_white_transparency_and_no_source_changes() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("test.png");
    fixture(&input);
    let before = fs::read(&input).unwrap();
    let mut editor = Editor::default();
    editor.open(input.clone(), false).unwrap();
    editor.edit("rotate_right", &[]).unwrap();
    let output = dir.path().join("test.jpg");
    editor.export(&output).unwrap();
    assert!(!editor.dirty());
    let image = image::open(output).unwrap();
    assert_eq!((image.width(), image.height()), (6, 8));
    let white = image.to_rgb8().get_pixel(3, 0).0;
    assert!(white.iter().all(|c| *c > 220), "{white:?}");
    assert_eq!(fs::read(input).unwrap(), before);
    editor.edit("undo", &[]).unwrap();
    assert!(editor.dirty());
    editor.edit("redo", &[]).unwrap();
    assert!(!editor.dirty());
}
fn batch_options(files: Vec<std::path::PathBuf>, directory: std::path::PathBuf) -> BatchOptions {
    BatchOptions {
        files,
        directory,
        format: "png".into(),
        resize: Some([4, 4]),
        keep_aspect: true,
        rotation: 0,
        grayscale: false,
        invert: false,
        auto: false,
    }
}
#[test]
fn batch_skips_existing_and_duplicate_names_continues_after_bad_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out");
    fs::create_dir(&output).unwrap();
    let a = dir.path().join("same.png");
    fixture(&a);
    let b = dir.path().join("same.bmp");
    fixture(&b);
    let bad = dir.path().join("bad.png");
    fs::write(&bad, b"invalid").unwrap();
    let last = dir.path().join("photo.edited.png");
    fixture(&last);
    fs::write(output.join("same.png"), b"keep me").unwrap();
    let mut progress = vec![];
    let result = editor::batch(batch_options(vec![a, b, bad, last], output.clone()), |r| {
        progress.push(r.completed)
    })
    .unwrap();
    assert_eq!(result.written, 1);
    assert_eq!(result.errors.len(), 3);
    assert_eq!(progress, vec![1, 2, 3, 4]);
    assert_eq!(fs::read(output.join("same.png")).unwrap(), b"keep me");
    let converted = image::open(output.join("photo.edited.png")).unwrap();
    assert_eq!((converted.width(), converted.height()), (4, 3));
}
#[test]
fn failed_export_does_not_clear_dirty_or_replace_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.png");
    fixture(&input);
    let mut editor = Editor::default();
    editor.open(input, false).unwrap();
    editor.edit("invert", &[]).unwrap();
    let target = dir.path().join("out.gif");
    fs::write(&target, b"keep").unwrap();
    assert!(editor.export(&target).is_err());
    assert!(editor.dirty());
    assert_eq!(fs::read(target).unwrap(), b"keep");
}
#[test]
fn unchanged_adjustments_do_not_create_edits_and_invalid_values_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.png");
    fixture(&input);
    let mut editor = Editor::default();
    editor.open(input, false).unwrap();
    let view = editor.adjust(Adjustments::default()).unwrap();
    assert!(!view.dirty);
    assert!(!view.can_undo);
    assert!(
        editor
            .adjust(Adjustments {
                gamma: f32::NAN,
                ..Default::default()
            })
            .is_err()
    );
    assert!(
        editor
            .adjust(Adjustments {
                red: 5.0,
                ..Default::default()
            })
            .is_err()
    );
}
#[test]
fn extensionless_image_stays_in_folder_navigation() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.png");
    fixture(&input);
    let other = dir.path().join("no-extension");
    fs::rename(&input, &other).unwrap();
    assert!(editor::decode(&other).is_ok());
    let (files, index) = navigation::get_image_siblings(&other);
    assert_eq!(files[index], other.canonicalize().unwrap());
}
#[test]
fn save_without_overwrite_is_atomic_and_preserves_destination() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.png");
    fixture(&input);
    let before = fs::read(&input).unwrap();
    assert!(editor::save(&DynamicImage::new_rgba8(2, 2), &input, false).is_err());
    assert_eq!(fs::read(input).unwrap(), before);
}
