use image::DynamicImage;

/// Non-destructively adjust the saturation of an image.
/// `saturation` slider is between -1.0 (grayscale) and 1.0 (double saturation).
pub fn adjust_saturation(img: &DynamicImage, saturation: f32) -> DynamicImage {
    if saturation.abs() < 0.001 {
        return img.clone();
    }

    let mut rgba_img = img.to_rgba8();
    let factor = 1.0 + saturation;

    for pixel in rgba_img.pixels_mut() {
        let r = pixel[0] as f32;
        let g = pixel[1] as f32;
        let b = pixel[2] as f32;

        let gray = 0.299 * r + 0.587 * g + 0.114 * b;

        pixel[0] = (gray + (r - gray) * factor).clamp(0.0, 255.0) as u8;
        pixel[1] = (gray + (g - gray) * factor).clamp(0.0, 255.0) as u8;
        pixel[2] = (gray + (b - gray) * factor).clamp(0.0, 255.0) as u8;
    }

    DynamicImage::ImageRgba8(rgba_img)
}

/// Adjust the gamma correction of an image.
/// `gamma` is between 0.1 and 3.0, where 1.0 is no change.
pub fn adjust_gamma(img: &DynamicImage, gamma: f32) -> DynamicImage {
    if (gamma - 1.0).abs() < 0.001 || gamma <= 0.0 {
        return img.clone();
    }

    let mut rgba_img = img.to_rgba8();
    let inv_gamma = 1.0 / gamma;

    // Use a lookup table (LUT) for performance
    let mut lut = [0u8; 256];
    for (i, val) in lut.iter_mut().enumerate() {
        let normalized = i as f32 / 255.0;
        let adjusted = normalized.powf(inv_gamma) * 255.0;
        *val = adjusted.clamp(0.0, 255.0) as u8;
    }

    for pixel in rgba_img.pixels_mut() {
        pixel[0] = lut[pixel[0] as usize];
        pixel[1] = lut[pixel[1] as usize];
        pixel[2] = lut[pixel[2] as usize];
    }

    DynamicImage::ImageRgba8(rgba_img)
}

/// Adjust color tint of an image by adding Red, Green, Blue channel offsets.
/// `r_offset`, `g_offset`, `b_offset` are sliders from -1.0 to 1.0.
pub fn adjust_tint(
    img: &DynamicImage,
    r_offset: f32,
    g_offset: f32,
    b_offset: f32,
) -> DynamicImage {
    if r_offset.abs() < 0.001 && g_offset.abs() < 0.001 && b_offset.abs() < 0.001 {
        return img.clone();
    }

    let mut rgba_img = img.to_rgba8();
    let r_add = r_offset * 255.0;
    let g_add = g_offset * 255.0;
    let b_add = b_offset * 255.0;

    for pixel in rgba_img.pixels_mut() {
        pixel[0] = (pixel[0] as f32 + r_add).clamp(0.0, 255.0) as u8;
        pixel[1] = (pixel[1] as f32 + g_add).clamp(0.0, 255.0) as u8;
        pixel[2] = (pixel[2] as f32 + b_add).clamp(0.0, 255.0) as u8;
    }

    DynamicImage::ImageRgba8(rgba_img)
}

/// Automatically adjust image contrast and brightness (histogram stretching/auto-levels).
///
/// A naive stretch based on the literal per-channel min/max is a no-op on almost any
/// real photo: noise, JPEG artifacts, or a single specular highlight/shadow pixel means
/// a channel has usually already touched 0 and 255, so min==0 and max==255 already and
/// there's nothing to stretch. Real auto-levels implementations (e.g. Photoshop's
/// "Auto Levels") instead clip a small percentile of outlier pixels at each end of the
/// histogram before stretching, which is what actually produces a visible effect.
pub fn auto_adjust(img: &DynamicImage) -> DynamicImage {
    // Fraction of pixels to clip at each end of the histogram, per channel.
    const CLIP_FRACTION: f64 = 0.005; // 0.5%, matching common auto-levels implementations

    let mut rgba_img = img.to_rgba8();

    // Build a 256-bucket histogram per channel, over non-transparent pixels only.
    let mut hist_r = [0u32; 256];
    let mut hist_g = [0u32; 256];
    let mut hist_b = [0u32; 256];
    let mut total: u32 = 0;

    for pixel in rgba_img.pixels() {
        if pixel[3] > 0 {
            hist_r[pixel[0] as usize] += 1;
            hist_g[pixel[1] as usize] += 1;
            hist_b[pixel[2] as usize] += 1;
            total += 1;
        }
    }

    if total == 0 {
        return DynamicImage::ImageRgba8(rgba_img);
    }

    let clip_count = ((total as f64) * CLIP_FRACTION).round() as u32;

    // Find the low/high bounds such that `clip_count` pixels lie outside them on each side.
    let find_bounds = |hist: &[u32; 256]| -> (u8, u8) {
        let mut low = 0u16;
        let mut acc = 0u32;
        while low < 255 {
            acc += hist[low as usize];
            if acc > clip_count {
                break;
            }
            low += 1;
        }

        let mut high = 255i16;
        let mut acc = 0u32;
        while high > 0 {
            acc += hist[high as usize];
            if acc > clip_count {
                break;
            }
            high -= 1;
        }

        if high < low as i16 {
            (0, 255) // Degenerate histogram (e.g. near-solid color); leave unchanged
        } else {
            (low as u8, high as u8)
        }
    };

    let (min_r, max_r) = find_bounds(&hist_r);
    let (min_g, max_g) = find_bounds(&hist_g);
    let (min_b, max_b) = find_bounds(&hist_b);

    let stretch = |val: u8, min: u8, max: u8| -> u8 {
        if max <= min {
            val
        } else {
            (((val as f32 - min as f32) / (max as f32 - min as f32)) * 255.0).clamp(0.0, 255.0)
                as u8
        }
    };

    for pixel in rgba_img.pixels_mut() {
        pixel[0] = stretch(pixel[0], min_r, max_r);
        pixel[1] = stretch(pixel[1], min_g, max_g);
        pixel[2] = stretch(pixel[2], min_b, max_b);
    }

    DynamicImage::ImageRgba8(rgba_img)
}

/// Crop an image to a rectangular area (in image pixel space).
pub fn crop_image(img: &DynamicImage, x: u32, y: u32, width: u32, height: u32) -> DynamicImage {
    if x >= img.width() || y >= img.height() {
        return img.clone();
    }
    let w = width.min(img.width() - x);
    let h = height.min(img.height() - y);
    if w == 0 || h == 0 {
        return img.clone();
    }
    img.crop_imm(x, y, w, h)
}

/// Resize an image to exact dimensions.
pub fn resize_image(img: &DynamicImage, width: u32, height: u32) -> DynamicImage {
    if width == 0 || height == 0 {
        return img.clone();
    }
    img.resize_exact(width, height, image::imageops::FilterType::Lanczos3)
}

/// Apply interactive adjustments (brightness, contrast, saturation, gamma, tint) to a DynamicImage.
pub fn apply_adjustments(
    img: &DynamicImage,
    brightness: f32,
    contrast: f32,
    saturation: f32,
    gamma: f32,
    tint: [f32; 3],
) -> DynamicImage {
    let [r_tint, g_tint, b_tint] = tint;
    let mut adjusted = img.clone();

    // 1. Brightness: slider -1.0 to 1.0 -> maps to -255 to 255
    if brightness.abs() > 0.001 {
        let val = (brightness * 255.0) as i32;
        adjusted = adjusted.brighten(val);
    }

    // 2. Contrast: slider -1.0 to 1.0 -> maps to -100.0 to 100.0
    if contrast.abs() > 0.001 {
        let val = contrast * 100.0;
        adjusted = adjusted.adjust_contrast(val);
    }

    // 3. Saturation: slider -1.0 to 1.0
    if saturation.abs() > 0.001 {
        adjusted = adjust_saturation(&adjusted, saturation);
    }

    // 4. Gamma: slider 0.1 to 3.0 (default 1.0)
    if (gamma - 1.0).abs() > 0.001 {
        adjusted = adjust_gamma(&adjusted, gamma);
    }

    // 5. Tint: sliders -1.0 to 1.0
    if r_tint.abs() > 0.001 || g_tint.abs() > 0.001 || b_tint.abs() > 0.001 {
        adjusted = adjust_tint(&adjusted, r_tint, g_tint, b_tint);
    }

    adjusted
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    #[test]
    fn test_gamma_correction() {
        let buffer = ImageBuffer::from_fn(1, 1, |_, _| Rgba([128, 128, 128, 255]));
        let img = DynamicImage::ImageRgba8(buffer);

        // Gamma 2.0 (image should be brighter since inv_gamma is 0.5)
        let adjusted = adjust_gamma(&img, 2.0);
        let p = adjusted.to_rgba8().get_pixel(0, 0).0;
        // (128/255)^0.5 * 255 = 0.707 * 255 = ~180
        assert!(p[0] > 175 && p[0] < 185);
    }

    #[test]
    fn test_crop_image() {
        let buffer = ImageBuffer::from_fn(4, 4, |x, y| Rgba([x as u8, y as u8, 0, 255]));
        let img = DynamicImage::ImageRgba8(buffer);
        let cropped = crop_image(&img, 1, 1, 2, 2);
        assert_eq!(cropped.width(), 2);
        assert_eq!(cropped.height(), 2);

        let p = cropped.to_rgba8().get_pixel(0, 0).0;
        assert_eq!(p[0], 1);
        assert_eq!(p[1], 1);
    }

    #[test]
    fn test_auto_adjust_stretches_low_contrast_image_with_outlier_pixels() {
        // Simulate a real low-contrast photo: most pixels sit in a narrow mid-gray band
        // (say 100-140), but a couple of stray pixels already touch 0 and 255 (sensor
        // noise / a specular highlight), which is extremely common in real photos.
        // A naive min/max stretch would see min=0, max=255 already and do nothing.
        let mut buffer = ImageBuffer::from_fn(10, 10, |x, _y| {
            let v = 100 + (x as u8 * 4); // spans 100..136
            Rgba([v, v, v, 255])
        });
        buffer.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        buffer.put_pixel(9, 9, Rgba([255, 255, 255, 255]));
        let img = DynamicImage::ImageRgba8(buffer);

        let adjusted = auto_adjust(&img);
        let rgba = adjusted.to_rgba8();

        // Column 1 (original value 104, near the low end of the 100-136 band) and
        // column 8 (original value 132, near the high end) should now be pulled much
        // closer to black and white respectively, rather than staying compressed
        // around 104/132. A naive min/max stretch (the old, buggy behavior) would
        // leave these essentially unchanged because the outlier pixels already
        // touched 0 and 255.
        let low_end = rgba.get_pixel(1, 3).0[0];
        let high_end = rgba.get_pixel(8, 3).0[0];
        assert!(
            low_end < 50,
            "expected low-end pixel to be stretched toward black, got {}",
            low_end
        );
        assert!(
            high_end > 200,
            "expected high-end pixel to be stretched toward white, got {}",
            high_end
        );
    }

    #[test]
    fn test_auto_adjust_leaves_full_range_image_reasonable() {
        // A genuinely full-range image (every value 0..=255 present) should not panic
        // or invert; bounds should degrade gracefully.
        let buffer = ImageBuffer::from_fn(256, 1, |x, _y| Rgba([x as u8, x as u8, x as u8, 255]));
        let img = DynamicImage::ImageRgba8(buffer);
        let adjusted = auto_adjust(&img);
        assert_eq!(adjusted.width(), 256);
        assert_eq!(adjusted.height(), 1);
    }
}
