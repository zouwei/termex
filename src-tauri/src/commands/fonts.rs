use serde::Serialize;
use std::path::PathBuf;

/// Metadata for a user-uploaded custom font.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFont {
    pub name: String,
    pub file_name: String,
    pub path: String,
    pub size: u64,
}

/// Allowed font file extensions.
const FONT_EXTENSIONS: &[&str] = &[".ttf", ".otf", ".woff", ".woff2"];

/// Get the ~/.termex/fonts/ directory, creating it if necessary.
fn get_fonts_dir() -> Result<PathBuf, String> {
    let app_data_dir = get_app_data_dir()?;
    let fonts_dir = app_data_dir.join("fonts");

    std::fs::create_dir_all(&fonts_dir)
        .map_err(|e| format!("Failed to create fonts directory: {}", e))?;

    Ok(fonts_dir)
}

/// Get the ~/.termex/ directory, using platform-specific paths.
fn get_app_data_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir().ok_or("Failed to get home directory")?;
        Ok(home.join(".termex"))
    }

    #[cfg(target_os = "windows")]
    {
        let app_data = std::env::var("APPDATA")
            .map_err(|_| "Failed to get APPDATA environment variable")?;
        Ok(PathBuf::from(app_data).join("termex"))
    }

    #[cfg(target_os = "linux")]
    {
        let home = dirs::home_dir().ok_or("Failed to get home directory")?;
        Ok(home.join(".termex"))
    }
}

/// Derive a display name from the font filename.
///
/// "MesloLGS-NF-Regular.ttf" → "MesloLGS NF Regular"
fn derive_font_name(file_name: &str) -> String {
    let name = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name);

    name.replace(['-', '_'], " ")
}

/// Sanitize a filename to prevent path traversal.
fn sanitize_filename(name: &str) -> Result<String, String> {
    let basename = std::path::Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?
        .to_string();

    if basename.contains("..") {
        return Err("Invalid filename".to_string());
    }

    let lower = basename.to_lowercase();
    if !FONT_EXTENSIONS.iter().any(|ext| lower.ends_with(ext)) {
        return Err(format!(
            "Unsupported font format. Allowed: {}",
            FONT_EXTENSIONS.join(", ")
        ));
    }

    Ok(basename)
}

/// Lists all custom fonts in ~/.termex/fonts/.
#[tauri::command]
pub async fn fonts_list_custom() -> Result<Vec<CustomFont>, String> {
    let fonts_dir = get_fonts_dir()?;

    if !fonts_dir.exists() {
        return Ok(Vec::new());
    }

    let mut fonts = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&fonts_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    let path = entry.path();
                    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                        let lower = filename.to_lowercase();
                        if FONT_EXTENSIONS.iter().any(|ext| lower.ends_with(ext)) {
                            fonts.push(CustomFont {
                                name: derive_font_name(filename),
                                file_name: filename.to_string(),
                                path: path.to_string_lossy().to_string(),
                                size: metadata.len(),
                            });
                        }
                    }
                }
            }
        }
    }

    fonts.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(fonts)
}

/// Saves an uploaded font file to ~/.termex/fonts/.
#[tauri::command]
pub async fn fonts_upload(file_name: String, data: Vec<u8>) -> Result<CustomFont, String> {
    let safe_name = sanitize_filename(&file_name)?;
    let fonts_dir = get_fonts_dir()?;
    let dest = fonts_dir.join(&safe_name);

    std::fs::write(&dest, &data)
        .map_err(|e| format!("Failed to write font file: {}", e))?;

    let size = data.len() as u64;

    Ok(CustomFont {
        name: derive_font_name(&safe_name),
        file_name: safe_name,
        path: dest.to_string_lossy().to_string(),
        size,
    })
}

/// Deletes a custom font from ~/.termex/fonts/.
#[tauri::command]
pub async fn fonts_delete(file_name: String) -> Result<(), String> {
    let safe_name = sanitize_filename(&file_name)?;
    let fonts_dir = get_fonts_dir()?;
    let font_path = fonts_dir.join(&safe_name);

    if !font_path.exists() {
        return Err(format!("Font not found: {}", safe_name));
    }

    std::fs::remove_file(&font_path)
        .map_err(|e| format!("Failed to delete font: {}", e))
}

/// Reads a custom font file as bytes (for frontend FontFace API loading).
#[tauri::command]
pub async fn fonts_read(file_name: String) -> Result<Vec<u8>, String> {
    let safe_name = sanitize_filename(&file_name)?;
    let fonts_dir = get_fonts_dir()?;
    let font_path = fonts_dir.join(&safe_name);

    if !font_path.exists() {
        return Err(format!("Font not found: {}", safe_name));
    }

    std::fs::read(&font_path)
        .map_err(|e| format!("Failed to read font file: {}", e))
}
