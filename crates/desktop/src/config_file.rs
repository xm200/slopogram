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

    #[cfg(target_os = "linux")]
    let user_config =
        directories::BaseDirs::new().map(|dirs| dirs.config_dir().join("slopogram/.env"));
    #[cfg(not(target_os = "linux"))]
    let user_config: Option<PathBuf> = None;

    for path in candidates(&cwd, exe_dir, &repository, user_config.as_deref()) {
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

fn candidates(
    cwd: &Path,
    exe_dir: &Path,
    repository: &Path,
    user_config: Option<&Path>,
) -> Vec<PathBuf> {
    let mut paths = vec![cwd.join(".env")];
    paths.extend(user_config.map(Path::to_path_buf));
    paths.push(exe_dir.join(".env"));
    paths.push(repository.join(".env"));
    paths
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
                Path::new("/repo"),
                Some(Path::new("/config/slopogram/.env"))
            ),
            [
                Path::new("/working/.env"),
                Path::new("/config/slopogram/.env"),
                Path::new("/bundle/.env"),
                Path::new("/repo/.env"),
            ]
        );
    }
}
