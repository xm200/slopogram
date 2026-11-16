use crate::theme_mappings::MAPPINGS;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Cursor, Read},
    path::Path,
};
use zip::ZipArchive;

const MAX_ARCHIVE: u64 = 24 * 1024 * 1024;
const MAX_COLORS: u64 = 1024 * 1024;
const MAX_WALLPAPER: u64 = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 256;

pub struct DesktopTheme {
    pub name: String,
    pub palette: String,
    pub is_dark: bool,
    pub wallpaper: Option<Vec<u8>>,
    pub wallpaper_extension: Option<String>,
    pub wallpaper_tiled: bool,
}

fn read_limited<R: Read>(reader: R, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read theme archive".to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Theme component exceeds size limit".into());
    }
    Ok(bytes)
}

fn strip_comments(text: &str) -> Result<String, String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"//") {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if bytes[i..].starts_with(b"/*") {
            i += 2;
            let start = i;
            while i + 1 < bytes.len() && !bytes[i..].starts_with(b"*/") {
                i += 1;
            }
            if i + 1 >= bytes.len() {
                return Err("Unterminated theme comment".into());
            }
            if i > start {
                out.push(b' ');
            }
            i += 2;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "Theme colors are not UTF-8".into())
}
fn definitions(text: &str) -> Result<HashMap<String, String>, String> {
    let clean = strip_comments(text.trim_start_matches('\u{feff}'))?;
    let mut values = HashMap::new();
    for part in clean.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, value) = part
            .split_once(':')
            .ok_or("Invalid theme color definition")?;
        let key = key.trim();
        let value = value.trim();
        if key.is_empty()
            || !key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            || value.is_empty()
            || value.bytes().any(|c| c.is_ascii_whitespace())
        {
            return Err("Invalid theme color definition".into());
        }
        values.insert(key.to_owned(), value.to_owned());
        if values.len() > 10_000 {
            return Err("Too many theme colors".into());
        }
    }
    Ok(values)
}
fn resolve(key: &str, values: &HashMap<String, String>) -> Result<Option<[u8; 4]>, String> {
    if !values.contains_key(key) {
        return Ok(None);
    }
    let mut current = key;
    let mut seen = HashSet::new();
    loop {
        if !seen.insert(current.to_owned()) {
            return Err("Theme color alias cycle".into());
        }
        let value = values.get(current).ok_or("Theme color alias is missing")?;
        if let Some(hex) = value.strip_prefix('#') {
            if !(hex.len() == 6 || hex.len() == 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("Invalid theme color".into());
            }
            let mut result = [0, 0, 0, 255];
            for (index, chunk) in hex.as_bytes().as_chunks::<2>().0.iter().enumerate() {
                let pair = std::str::from_utf8(chunk).map_err(|_| "Invalid theme color")?;
                result[index] = u8::from_str_radix(pair, 16).map_err(|_| "Invalid theme color")?;
            }
            return Ok(Some(result));
        }
        if !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err("Invalid theme color alias".into());
        }
        current = value;
    }
}
fn render_palette(name: &str, colors: &str) -> Result<(String, bool), String> {
    let values = definitions(colors)?;
    let mut background = None;
    for key in ["windowBg", "dialogsBg", "historyComposeAreaBg"] {
        if let Some(color) = resolve(key, &values)? {
            background = Some(color);
            break;
        }
    }
    let background = background.unwrap_or([255, 255, 255, 255]);
    let dark = (u32::from(background[0]) * 299
        + u32::from(background[1]) * 587
        + u32::from(background[2]) * 114)
        < 128_000;
    let mut out = format!(
        "isDark = {}\ntinted = 1\nname = {}\nparent = {}\n",
        u8::from(dark),
        name,
        if dark { "nightAccent" } else { "dayClassic" }
    );
    let mut mapped = 0;
    for &(target, source) in MAPPINGS {
        if let Some(color) = resolve(source, &values)? {
            mapped += 1;
            out.push_str(&format!(
                "{target} = #{:02x}{:02x}{:02x}",
                color[0], color[1], color[2]
            ));
            if color[3] != 255 {
                out.push_str(&format!(":{:.6}", f64::from(color[3]) / 255.0));
            }
            out.push('\n');
        }
    }
    if mapped == 0 {
        return Err("Theme has no supported colors".into());
    }
    out.push_str("wallpaper = none\n");
    Ok((out, dark))
}

pub fn decode(path: &Path) -> Result<DesktopTheme, String> {
    let file = File::open(path).map_err(|_| "Cannot open theme file")?;
    if file
        .metadata()
        .map_err(|_| "Cannot inspect theme file")?
        .len()
        > MAX_ARCHIVE
    {
        return Err("Theme archive exceeds size limit".into());
    }
    let bytes = read_limited(file, MAX_ARCHIVE)?;
    let (colors, wallpaper) = if bytes.starts_with(b"PK") {
        let mut archive =
            ZipArchive::new(Cursor::new(bytes)).map_err(|_| "Invalid theme archive")?;
        if archive.len() > MAX_ENTRIES {
            return Err("Too many theme archive entries".into());
        }
        let mut indices = HashMap::new();
        for index in 0..archive.len() {
            let entry = archive
                .by_index(index)
                .map_err(|_| "Cannot read theme archive entry")?;
            let name = entry.name().to_ascii_lowercase();
            if matches!(
                name.as_str(),
                "colors.tdesktop-theme"
                    | "colors.tdesktop-palette"
                    | "background.jpg"
                    | "background.jpeg"
                    | "background.png"
                    | "tiled.jpg"
                    | "tiled.jpeg"
                    | "tiled.png"
            ) && indices.insert(name, index).is_some()
            {
                return Err("Duplicate theme archive entry".into());
            }
        }
        let colors_index = ["colors.tdesktop-theme", "colors.tdesktop-palette"]
            .iter()
            .find_map(|name| indices.get(*name).copied())
            .ok_or("Theme has no colors entry")?;
        let entry = archive
            .by_index(colors_index)
            .map_err(|_| "Cannot read theme colors")?;
        if entry.size() > MAX_COLORS {
            return Err("Invalid theme colors entry".into());
        }
        let colors = String::from_utf8(read_limited(entry, MAX_COLORS)?)
            .map_err(|_| "Theme colors are not UTF-8")?;
        let selection = [
            ("background.jpg", "jpg", false),
            ("background.jpeg", "jpg", false),
            ("background.png", "png", false),
            ("tiled.jpg", "jpg", true),
            ("tiled.jpeg", "jpg", true),
            ("tiled.png", "png", true),
        ]
        .iter()
        .find_map(|(name, extension, tiled)| {
            indices.get(*name).map(|index| (*index, *extension, *tiled))
        });
        let wallpaper = if let Some((index, extension, tiled)) = selection {
            let entry = archive
                .by_index(index)
                .map_err(|_| "Cannot read theme wallpaper")?;
            if entry.size() > MAX_WALLPAPER {
                return Err("Invalid theme wallpaper entry".into());
            }
            Some((
                read_limited(entry, MAX_WALLPAPER)?,
                extension.to_owned(),
                tiled,
            ))
        } else {
            None
        };
        (colors, wallpaper)
    } else {
        if bytes.len() as u64 > MAX_COLORS {
            return Err("Theme colors exceed size limit".into());
        }
        (
            String::from_utf8(bytes).map_err(|_| "Theme colors are not UTF-8")?,
            None,
        )
    };
    let name = path
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("Imported Theme")
        .chars()
        .filter(|c| !c.is_control() && *c != '=')
        .take(100)
        .collect::<String>();
    let name = if name.trim().is_empty() {
        "Imported Theme".to_string()
    } else {
        name
    };
    let (palette, is_dark) = render_palette(&name, &colors)?;
    let (wallpaper, wallpaper_extension, wallpaper_tiled) = match wallpaper {
        Some((bytes, extension, tiled)) => (Some(bytes), Some(extension), tiled),
        None => (None, None, false),
    };
    Ok(DesktopTheme {
        name,
        palette,
        is_dark,
        wallpaper,
        wallpaper_extension,
        wallpaper_tiled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::{ZipWriter, write::FileOptions};

    fn archive(entries: &[(&str, &[u8])]) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        {
            let mut zip = ZipWriter::new(&mut file);
            for (name, data) in entries {
                zip.start_file(
                    *name,
                    FileOptions::default().compression_method(zip::CompressionMethod::Deflated),
                )
                .unwrap();
                zip.write_all(data).unwrap();
            }
            zip.finish().unwrap();
        }
        file
    }
    const DARK: &[u8] = b"/* comment */\nwindowBg: #102030; windowBgActive: accentColor; accentColor: #abcdef80; windowFg: #ffffff;";
    #[test]
    fn maps_alias_alpha_and_background_priority() {
        let file = archive(&[
            ("COLORS.TDESKTOP-PALETTE", b"windowBg: #ffffff;"),
            ("COLORS.TDESKTOP-THEME", DARK),
            ("tiled.png", b"tile"),
            ("background.jpg", b"background"),
        ]);
        let theme = decode(file.path()).unwrap();
        assert!(theme.is_dark);
        assert!(theme.palette.contains("basicAccent = #abcdef:0.501961"));
        assert_eq!(theme.wallpaper.as_deref(), Some(&b"background"[..]));
        assert_eq!(theme.wallpaper_extension.as_deref(), Some("jpg"));
        assert!(!theme.wallpaper_tiled);
    }
    #[test]
    fn supports_plain_and_tiled_and_missing_wallpaper() {
        let mut plain = tempfile::NamedTempFile::new().unwrap();
        plain
            .write_all(b"windowBg: #ffffff; windowFg: #101010;")
            .unwrap();
        let theme = decode(plain.path()).unwrap();
        assert!(!theme.is_dark);
        assert!(theme.wallpaper.is_none());
        assert!(theme.palette.contains("parent = dayClassic"));
        let zip = archive(&[("colors.tdesktop-palette", DARK), ("tiled.png", b"tile")]);
        let theme = decode(zip.path()).unwrap();
        assert!(theme.wallpaper_tiled);
        assert_eq!(theme.wallpaper_extension.as_deref(), Some("png"));
    }
    #[test]
    fn sparse_palettes_infer_background_or_use_light_fallback() {
        let (palette, dark) = render_palette("x", "dialogsBg: #111111;").unwrap();
        assert!(dark && palette.contains("listBackground = #111111"));
        let (palette, dark) = render_palette("x", "windowFg: #222222;").unwrap();
        assert!(!dark && palette.contains("text = #222222"));
        assert!(render_palette("x", "unknown: #ffffff;").is_err());
        assert!(render_palette("x", "").is_err());
    }
    #[test]
    fn rejects_bad_aliases_duplicates_and_oversized_entries() {
        assert!(render_palette("x", "windowBg: windowBg;").is_err());
        assert!(render_palette("x", "windowBg: #1122zz;").is_err());
        assert!(
            decode(
                archive(&[
                    ("colors.tdesktop-theme", DARK),
                    ("COLORS.TDESKTOP-THEME", DARK)
                ])
                .path()
            )
            .is_err()
        );
        assert!(
            decode(
                archive(&[(
                    "colors.tdesktop-theme",
                    &vec![b'x'; MAX_COLORS as usize + 1]
                )])
                .path()
            )
            .is_err()
        );
        assert!(strip_comments("windowBg: #fff; /*").is_err());
    }
    #[test]
    #[ignore = "requires explicitly supplied local theme fixtures"]
    fn provided_theme_archives() {
        let directory =
            std::env::var_os("SLOPOGRAM_THEME_FIXTURES").expect("set fixture directory");
        let mut count = 0;
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|v| v.to_str()) != Some("tdesktop-theme") {
                continue;
            }
            let theme = decode(&path).unwrap();
            assert!(theme.is_dark);
            assert_eq!(
                theme
                    .palette
                    .lines()
                    .filter(|line| line.contains(" = "))
                    .count(),
                125
            );
            let mut archive = ZipArchive::new(File::open(&path).unwrap()).unwrap();
            let mut expected = Vec::new();
            archive
                .by_name("background.jpg")
                .unwrap()
                .read_to_end(&mut expected)
                .unwrap();
            assert_eq!(theme.wallpaper.as_deref(), Some(expected.as_slice()));
            count += 1;
        }
        assert_eq!(count, 6);
    }
}
