use crate::theme_mappings::MAPPINGS;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Cursor, Read, Write},
    path::Path,
};
use zip::ZipArchive;

const MAX_ARCHIVE: u64 = 24 * 1024 * 1024;
const MAX_COLORS: u64 = 1024 * 1024;
const MAX_WALLPAPER: u64 = 16 * 1024 * 1024;
const MAX_VIDEO: u64 = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 256;

pub struct DesktopTheme {
    pub name: String,
    pub palette: String,
    pub is_dark: bool,
    pub wallpaper: Option<Vec<u8>>,
    pub wallpaper_extension: Option<String>,
    pub wallpaper_tiled: bool,
    pub video: Option<Vec<u8>>,
    pub video_warning: Option<String>,
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

pub fn desktop_colors(palette: &str) -> Result<Vec<(String, String)>, String> {
    if palette.len() as u64 > MAX_COLORS {
        return Err("Theme colors exceed size limit".into());
    }
    let mut native = HashMap::new();
    for line in palette.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if MAPPINGS.iter().any(|(target, _)| *target == key)
                && native.insert(key, value.trim()).is_some()
            {
                return Err("Duplicate palette color".into());
            }
        }
    }
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for &(target, source) in MAPPINGS {
        if let Some(value) = native.get(target) {
            let (hex, alpha) = value
                .split_once(':')
                .map_or((value.trim(), None), |(h, a)| (h.trim(), Some(a.trim())));
            if hex.len() != 7
                || !hex.starts_with('#')
                || !hex[1..].bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err("Invalid palette color".into());
            }
            let alpha = match alpha {
                Some(a) => {
                    let a: f64 = a.parse().map_err(|_| "Invalid palette alpha")?;
                    if !a.is_finite() || !(0.0..=1.0).contains(&a) {
                        return Err("Invalid palette alpha".into());
                    }
                    (a * 255.0).round() as u8
                }
                None => 255,
            };
            if seen.insert(source) {
                result.push((
                    source.to_string(),
                    format!("{}{:02x}", hex.to_ascii_lowercase(), alpha),
                ));
            }
        }
    }
    if result.is_empty() {
        return Err("Theme has no supported colors".into());
    }
    Ok(result)
}

pub fn encode(
    colors: Vec<(String, String)>,
    wallpaper: Option<Vec<u8>>,
    extension: Option<String>,
    tiled: bool,
    video: Option<Vec<u8>>,
) -> Result<Vec<u8>, String> {
    let mut values = HashMap::new();
    for (key, value) in colors {
        if key.is_empty()
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || value.is_empty()
            || values.insert(key, value).is_some()
        {
            return Err("Invalid or duplicate theme color".into());
        }
    }
    if values.is_empty() || values.len() > 10_000 {
        return Err("Invalid theme colors".into());
    }
    let mut keys: Vec<_> = values.keys().collect();
    keys.sort();
    let mut text = String::new();
    for key in &keys {
        let [r, g, b, alpha] = resolve(key, &values)?.ok_or("Missing theme color")?;
        text.push_str(&format!("{key}: #{r:02x}{g:02x}{b:02x}{alpha:02x};\n"));
    }
    render_palette("Export", &text)?;
    if text.len() as u64 > MAX_COLORS {
        return Err("Theme colors exceed size limit".into());
    }
    if wallpaper.is_some() != extension.is_some()
        || (tiled && wallpaper.is_none())
        || (video.is_some() && (wallpaper.is_none() || tiled))
    {
        return Err("Inconsistent theme wallpaper".into());
    }
    if let Some(ref bytes) = wallpaper {
        if bytes.len() as u64 > MAX_WALLPAPER {
            return Err("Wallpaper exceeds size limit".into());
        }
        let valid = match extension.as_deref() {
            Some("png") => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            Some("jpg") => bytes.starts_with(b"\xff\xd8\xff"),
            _ => false,
        };
        if !valid {
            return Err("Invalid wallpaper format".into());
        }
    }
    if video
        .as_ref()
        .is_some_and(|v| v.len() as u64 > MAX_VIDEO || v.len() < 12 || &v[4..8] != b"ftyp")
    {
        return Err("Invalid theme video".into());
    }
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("colors.tdesktop-theme", options)
        .map_err(|_| "Cannot write theme")?;
    zip.write_all(text.as_bytes())
        .map_err(|_| "Cannot write theme")?;
    if let Some(bytes) = wallpaper {
        zip.start_file(
            format!(
                "{}.{}",
                if tiled { "tiled" } else { "background" },
                extension.unwrap()
            ),
            options,
        )
        .map_err(|_| "Cannot write theme")?;
        zip.write_all(&bytes).map_err(|_| "Cannot write theme")?;
    }
    if let Some(bytes) = video {
        zip.start_file("slopogram/background.mp4", options)
            .map_err(|_| "Cannot write theme")?;
        zip.write_all(&bytes).map_err(|_| "Cannot write theme")?;
    }
    let bytes = zip.finish().map_err(|_| "Cannot write theme")?.into_inner();
    if bytes.len() as u64 > MAX_ARCHIVE {
        return Err("Theme archive exceeds size limit".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod export_tests {
    use super::*;
    #[test]
    fn palette_priority_and_alpha() {
        let colors = desktop_colors("basicAccent = #123456:0.5\naccent = #ffffff\n").unwrap();
        assert_eq!(colors, vec![("windowBgActive".into(), "#12345680".into())]);
    }
    #[test]
    fn rejects_invalid_colors_and_aliases() {
        for colors in [
            vec![("unknown", "#ffffff")],
            vec![("bad key", "#ffffff")],
            vec![("windowBg", "#zzz")],
            vec![("windowBg", "missing")],
            vec![("windowBg", "other"), ("other", "windowBg")],
        ] {
            assert!(
                encode(
                    colors
                        .into_iter()
                        .map(|(k, v)| (k.into(), v.into()))
                        .collect(),
                    None,
                    None,
                    false,
                    None
                )
                .is_err()
            );
        }
        assert!(desktop_colors("background = #123456 : 0.5").is_ok());
        assert!(desktop_colors("background = #123456 : NaN").is_err());
    }
    #[test]
    fn tiled_and_bounds() {
        let colors = vec![("windowBg".into(), "#12345680".into())];
        let png = b"\x89PNG\r\n\x1a\nfixture".to_vec();
        let bytes = encode(
            colors.clone(),
            Some(png.clone()),
            Some("png".into()),
            true,
            None,
        )
        .unwrap();
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), bytes).unwrap();
        let theme = decode(file.path()).unwrap();
        assert!(theme.wallpaper_tiled);
        assert_eq!(theme.wallpaper, Some(png.clone()));
        assert!(
            encode(
                colors.clone(),
                Some(png),
                Some("png".into()),
                true,
                Some(b"0000ftypxxxx".to_vec())
            )
            .is_err()
        );
        assert!(
            encode(
                colors.clone(),
                Some(vec![0; MAX_WALLPAPER as usize + 1]),
                Some("png".into()),
                false,
                None
            )
            .is_err()
        );
        assert!(
            encode(
                colors.clone(),
                Some(b"\xff\xd8\xff".to_vec()),
                Some("jpg".into()),
                false,
                Some(vec![0; MAX_VIDEO as usize + 1])
            )
            .is_err()
        );
        let mut image = vec![0; 13 * 1024 * 1024];
        image[..3].copy_from_slice(b"\xff\xd8\xff");
        let mut video = vec![0; 12 * 1024 * 1024];
        video[4..8].copy_from_slice(b"ftyp");
        assert!(encode(colors, Some(image), Some("jpg".into()), false, Some(video)).is_err());
        assert!(desktop_colors(&"x".repeat(MAX_COLORS as usize + 1)).is_err());
    }
    #[test]
    fn aliases_are_exported_as_literals() {
        let bytes = encode(
            vec![
                ("windowBg".into(), "zzz".into()),
                ("zzz".into(), "#12345680".into()),
            ],
            None,
            None,
            false,
            None,
        )
        .unwrap();
        let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut text = String::new();
        zip.by_name("colors.tdesktop-theme")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert!(text.contains("windowBg: #12345680;"));
    }
    #[test]
    fn archive_roundtrip_and_validation() {
        let colors = vec![
            ("windowBg".into(), "#12345680".into()),
            ("windowFg".into(), "windowBg".into()),
        ];
        let png = b"\x89PNG\r\n\x1a\nfixture".to_vec();
        let video = b"0000ftypfixture".to_vec();
        let bytes = encode(
            colors.clone(),
            Some(png.clone()),
            Some("png".into()),
            false,
            Some(video.clone()),
        )
        .unwrap();
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), bytes).unwrap();
        let decoded = decode(file.path()).unwrap();
        assert!(decoded.palette.contains("background = #123456:0.501961"));
        assert_eq!(decoded.wallpaper, Some(png));
        assert_eq!(decoded.video, Some(video));
        assert!(encode(colors.clone(), None, None, false, Some(vec![])).is_err());
        assert!(encode(colors, Some(vec![1]), Some("png".into()), false, None).is_err());
        assert!(
            encode(
                vec![
                    ("x".into(), "#ffffff".into()),
                    ("x".into(), "#000000".into())
                ],
                None,
                None,
                false,
                None
            )
            .is_err()
        );
    }
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
    let (colors, wallpaper, video, video_warning) = if bytes.starts_with(b"PK") {
        let mut archive =
            ZipArchive::new(Cursor::new(bytes)).map_err(|_| "Invalid theme archive")?;
        if archive.len() > MAX_ENTRIES {
            return Err("Too many theme archive entries".into());
        }
        let mut indices = HashMap::new();
        let mut video_index = None;
        let mut video_duplicate = false;
        for index in 0..archive.len() {
            let entry = archive
                .by_index_raw(index)
                .map_err(|_| "Cannot read theme archive entry")?;
            let name = entry.name().to_ascii_lowercase();
            if name == "slopogram/background.mp4" && video_index.replace(index).is_some() {
                video_duplicate = true;
            }
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
        let (video, video_warning) = if video_duplicate {
            (None, Some("Duplicate animated wallpaper entry".to_string()))
        } else if let Some(index) = video_index {
            match archive.by_index(index) {
                Ok(entry) if entry.size() <= MAX_VIDEO => match read_limited(entry, MAX_VIDEO) {
                    Ok(bytes) if !bytes.is_empty() => (Some(bytes), None),
                    _ => (None, Some("Cannot read animated wallpaper".to_string())),
                },
                _ => (
                    None,
                    Some("Animated wallpaper exceeds size limit or cannot be read".to_string()),
                ),
            }
        } else {
            (None, None)
        };
        (colors, wallpaper, video, video_warning)
    } else {
        if bytes.len() as u64 > MAX_COLORS {
            return Err("Theme colors exceed size limit".into());
        }
        (
            String::from_utf8(bytes).map_err(|_| "Theme colors are not UTF-8")?,
            None,
            None,
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
        video,
        video_warning,
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
    fn optional_video_is_bounded_and_does_not_discard_static_theme() {
        let valid = archive(&[
            ("colors.tdesktop-theme", DARK),
            ("slopogram/background.mp4", b"video"),
        ]);
        assert_eq!(
            decode(valid.path()).unwrap().video.as_deref(),
            Some(&b"video"[..])
        );
        let oversized = vec![0; MAX_VIDEO as usize + 1];
        let invalid = archive(&[
            ("colors.tdesktop-theme", DARK),
            ("background.jpg", b"poster"),
            ("slopogram/background.mp4", &oversized),
        ]);
        let theme = decode(invalid.path()).unwrap();
        assert_eq!(theme.wallpaper.as_deref(), Some(&b"poster"[..]));
        assert!(theme.video.is_none());
        assert!(theme.video_warning.is_some());
        let duplicate = archive(&[
            ("colors.tdesktop-theme", DARK),
            ("slopogram/background.mp4", b"a"),
            ("SLOPOGRAM/BACKGROUND.MP4", b"b"),
        ]);
        assert!(decode(duplicate.path()).unwrap().video_warning.is_some());
    }
    #[test]
    fn unsupported_video_compression_preserves_static_theme() {
        let file = archive(&[
            ("colors.tdesktop-theme", DARK),
            ("background.jpg", b"poster"),
            ("slopogram/background.mp4", b"video"),
        ]);
        let mut bytes = std::fs::read(file.path()).unwrap();
        // Patch the central directory compression method for the video entry.
        let name = b"slopogram/background.mp4";
        let name_at = bytes
            .windows(name.len())
            .rposition(|window| window == name)
            .unwrap();
        let header = name_at - 46;
        assert_eq!(&bytes[header..header + 4], b"PK\x01\x02");
        bytes[header + 10..header + 12].copy_from_slice(&98u16.to_le_bytes());
        std::fs::write(file.path(), bytes).unwrap();
        let theme = decode(file.path()).unwrap();
        assert_eq!(theme.wallpaper.as_deref(), Some(&b"poster"[..]));
        assert!(theme.video.is_none());
        assert!(theme.video_warning.is_some());
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
