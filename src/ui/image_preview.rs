use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use image::{GenericImageView, Pixel};
use ratatui::prelude::*;

use super::{ACCENT_SOFT, EDITOR_MUTED};

const IMAGE_CHAR: &str = "▀";
const MAX_IMAGE_HEIGHT: u16 = 18;

#[derive(Clone)]
struct CachedImage {
    modified: Option<SystemTime>,
    width: u16,
    lines: Vec<Line<'static>>,
}

static IMAGE_CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedImage>>> = OnceLock::new();

pub(super) fn render_image_reference(
    line: &str,
    base_dir: Option<&Path>,
    max_width: u16,
) -> Option<Vec<Line<'static>>> {
    let reference = parse_image_reference(line)?;
    let path = resolve_image_path(&reference, base_dir)?;
    let metadata = std::fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }

    let width = max_width.min(80).max(1);
    let modified = metadata.modified().ok();
    let cache = IMAGE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock() {
        if let Some(cached) = cache.get(&path) {
            if cached.modified == modified && cached.width == width {
                return Some(cached.lines.clone());
            }
        }
    }

    let image = image::open(&path).ok()?;
    let (source_width, source_height) = image.dimensions();
    if source_width == 0 || source_height == 0 {
        return None;
    }

    let rows = ((source_height as f32 / source_width as f32) * width as f32 / 2.0)
        .ceil()
        .max(1.0)
        .min(MAX_IMAGE_HEIGHT as f32) as u16;
    let pixel_height = rows.saturating_mul(2).max(2);
    let resized = image.resize_exact(
        width as u32,
        pixel_height as u32,
        image::imageops::FilterType::Triangle,
    );

    let mut lines = Vec::with_capacity(rows as usize + 1);
    lines.push(Line::from(vec![Span::styled(
        format!("image: {}", path.display()),
        Style::default().fg(EDITOR_MUTED),
    )]));

    for y in (0..pixel_height).step_by(2) {
        let mut spans = Vec::with_capacity(width as usize);
        for x in 0..width {
            let upper = resized.get_pixel(x as u32, y as u32).to_rgb();
            let lower = resized
                .get_pixel(x as u32, (y + 1).min(pixel_height - 1) as u32)
                .to_rgb();
            spans.push(Span::styled(
                IMAGE_CHAR,
                Style::default()
                    .fg(Color::Rgb(upper[0], upper[1], upper[2]))
                    .bg(Color::Rgb(lower[0], lower[1], lower[2])),
            ));
        }
        lines.push(Line::from(spans));
    }

    if let Ok(mut cache) = cache.lock() {
        cache.insert(
            path,
            CachedImage {
                modified,
                width,
                lines: lines.clone(),
            },
        );
    }

    Some(lines)
}

pub(super) fn image_fallback_line(line: &str) -> Option<Line<'static>> {
    let reference = parse_image_reference(line)?;
    Some(Line::from(vec![
        Span::styled("image ", Style::default().fg(ACCENT_SOFT)),
        Span::styled(reference, Style::default().fg(EDITOR_MUTED)),
    ]))
}

fn parse_image_reference(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("![[") {
        let end = rest.find("]]")?;
        let target = strip_obsidian_suffixes(&rest[..end]).trim();
        if !target.is_empty() {
            return Some(target.to_string());
        }
    }

    if !trimmed.starts_with("![") {
        return None;
    }
    let alt_end = trimmed.find("](")?;
    let rest = &trimmed[alt_end + 2..];
    let target_end = rest.find(')')?;
    let target = rest[..target_end]
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string();
    if target.is_empty() {
        None
    } else {
        Some(target)
    }
}

fn strip_obsidian_suffixes(target: &str) -> &str {
    let target = target
        .split_once('|')
        .map(|(target, _)| target)
        .unwrap_or(target);
    target
        .split_once('#')
        .map(|(target, _)| target)
        .unwrap_or(target)
}

fn resolve_image_path(reference: &str, base_dir: Option<&Path>) -> Option<PathBuf> {
    if reference.starts_with("http://")
        || reference.starts_with("https://")
        || reference.starts_with("data:")
    {
        return None;
    }

    let path = PathBuf::from(reference);
    let candidate = if path.is_absolute() {
        path
    } else if let Some(base_dir) = base_dir {
        base_dir.join(path)
    } else {
        std::env::current_dir().ok()?.join(path)
    };

    candidate.canonicalize().ok()
}
