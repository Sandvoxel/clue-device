use std::fs::read;
use std::path::PathBuf;
use image::{ColorType, ImageBuffer, Rgba};
use rusttype::{Font, point, Point, Scale};

#[cfg(target_os = "windows")]
fn default_font_path() -> PathBuf {
    PathBuf::from(r"C:\Windows\Fonts\arial.ttf")
}

#[cfg(target_os = "macos")]
fn default_font_path() -> PathBuf {
    PathBuf::from("/System/Library/Fonts/Supplemental/Arial.ttf")
}

#[cfg(target_os = "linux")]
fn default_font_path() -> PathBuf {
    PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
}

pub fn generate_image_with_text(text: &str, output_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Create an image buffer
    let mut image = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(1920, 1080);

    // Set the background color
    let background_color = Rgba([255, 255, 255, 255]);
    for pixel in image.pixels_mut() {
        *pixel = background_color;
    }

    // Draw text on the image
    let font = Font::try_from_vec(read(default_font_path())?).expect("Failed to load font.");

    let font_size = 100.0;

    let scale = Scale {
        x: font_size,
        y: font_size,
    };

    // Calculate the maximum width for each line based on the image width and the margin
    let max_line_width = image.width() as i32 - 200;

    // Split the text into words
    let words: Vec<&str> = text.split_whitespace().collect();

    // Create lines of words that don't exceed the maximum width
    let mut lines = vec![];
    let mut line = String::new();
    for word in words {
        let test_line = if line.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", line, word)
        };
        let test_line_width = text_width(&font,&test_line, font_size);
        if test_line_width <= max_line_width {
            line = test_line;
        } else {
            lines.push(line);
            line = word.to_string();
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }

    // Calculate the total height of the text
    let v_metrics = font.v_metrics(scale);
    let line_height = (v_metrics.ascent - v_metrics.descent).ceil() as i32;
    let total_height = line_height * lines.len() as i32;

    // Calculate the starting y-coordinate based on the total height of the text
    let start_y = (image.height() as i32 - total_height) / 2;

    // Draw each line of text, centering it horizontally
    for (i, line) in lines.iter().enumerate() {
        let glyphs: Vec<rusttype::PositionedGlyph> = font.layout(line, scale, point(0.0, font_size)).collect();
        let line_width: i32 = glyphs.last().map(|g| g.pixel_bounding_box().map(|b| b.max.x).unwrap_or(0)).unwrap_or(0);
        let start_x = (image.width() as i32 - line_width) / 2;
        let line_y = start_y + i as i32 * line_height;

        for glyph in &glyphs {
            if let Some(bb) = glyph.pixel_bounding_box() {
                glyph.draw(|x, y, v| {
                    let px = x as i32 + bb.min.x + start_x;
                    let py = y as i32 + bb.min.y + line_y;

                    if px >= 0 && px < image.width() as i32 && py >= 0 && py < image.height() as i32 {
                        let background_color = image.get_pixel(px as u32, py as u32);
                        let alpha = (v * 255.0) / 255.0;
                        let color = Rgba([
                            (background_color[0] as f32 * (1.0 - alpha) + 0.0 * alpha) as u8,
                            (background_color[1] as f32 * (1.0 - alpha) + 0.0 * alpha) as u8,
                            (background_color[2] as f32 * (1.0 - alpha) + 0.0 * alpha) as u8,
                            255,
                        ]);
                        image.put_pixel(px as u32, py as u32, color);
                    }
                });
            }
        }
    }

    // Save the image
    image::save_buffer(output_path, &image.clone().into_raw(), image.width(), image.height(), ColorType::Rgba8)?;

    Ok(())
}

fn text_width(font: &Font, text: &str, size: f32) -> i32 {
    let scale = Scale { x: size, y: size };
    let glyphs: Vec<_> = font.layout(text, scale, Point { x: 0.0, y: 0.0 }).collect();
    let width = glyphs
        .iter()
        .map(|g| g.unpositioned().h_metrics().advance_width)
        .sum::<f32>();

    width.ceil() as i32
}
