use std::path::{Path, PathBuf};

/// Load Slopogram configuration from an optional dotenv file.
///
/// Existing process environment variables take precedence over values in files.
pub fn load() -> Result<(), String> {
    if let Some(explicit) = std::env::var_os("SLOPOGRAM_CONFIG_FILE") {
        let path = PathBuf::from(explicit);
        return dotenvy::from_path(&path).map(|_| ()).map_err(|_| {
            format!(
                "Could not load the configuration file at {}.",
                path.display()
            )
        });
    }

    let cwd = std::env::current_dir()
        .map_err(|_| "Could not determine the current directory.".to_owned())?;
    let executable = std::env::current_exe()
        .map_err(|_| "Could not determine the executable path.".to_owned())?;
    let exe_dir = executable.parent().unwrap_or_else(|| Path::new("."));
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    for path in candidates(&cwd, exe_dir, &repository) {
        if path.exists() {
            return dotenvy::from_path(&path).map(|_| ()).map_err(|_| {
                format!(
                    "Could not load the configuration file at {}.",
                    path.display()
                )
            });
        }
    }
    Ok(())
}

fn candidates(cwd: &Path, exe_dir: &Path, repository: &Path) -> Vec<PathBuf> {
    vec![
        cwd.join(".env"),
        exe_dir.join(".env"),
        repository.join(".env"),
    ]
}

#[cfg(test)]
mod tests {
    use super::candidates;
    use std::path::Path;

    #[test]
    fn configuration_candidates_follow_precedence_order() {
        assert_eq!(
            candidates(
                Path::new("/working"),
                Path::new("/bundle"),
                Path::new("/repo")
            ),
            [
                Path::new("/working/.env"),
                Path::new("/bundle/.env"),
                Path::new("/repo/.env"),
            ]
        );
    }
}
