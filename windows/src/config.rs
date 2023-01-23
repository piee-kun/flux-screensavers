use serde::{Deserialize, Serialize};
use std::{fmt, fs, io, path};

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct Config {
    pub version: semver::Version,
    pub log_level: log::Level,
    pub flux: FluxSettings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Latest version of the config
            version: semver::Version::parse("0.1").unwrap(),
            log_level: log::Level::Warn,
            flux: Default::default(),
        }
    }
}

impl Config {
    pub fn load(optional_config_dir: Option<&path::Path>) -> Self {
        match optional_config_dir {
            None => Self::default(),

            Some(config_dir) => {
                let config = Self::load_existing_config(config_dir);

                if let Err(err) = &config {
                    match err {
                        Problem::ReadSettings { err, path }
                            if err.kind() == io::ErrorKind::NotFound =>
                        {
                            log::info!(
                                "No settings file found at {}. Using defaults.",
                                path.display()
                            )
                        }
                        _ => log::error!("{}", err),
                    }
                }

                config.unwrap_or_default()
            }
        }
    }

    fn load_existing_config(config_dir: &path::Path) -> Result<Config, Problem> {
        let config_path = config_dir.join("settings.json");
        let config_string =
            fs::read_to_string(&config_path).map_err(|err| Problem::ReadSettings {
                path: config_path.clone(),
                err,
            })?;
        serde_json::from_str(&config_string).map_err(|err| Problem::DecodeSettings {
            path: config_path.clone(),
            err,
        })
    }
}

#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct FluxSettings {
    pub color_mode: ColorMode,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ColorMode {
    Preset(flux::settings::ColorPreset),
    DesktopImage,
}

impl Default for ColorMode {
    fn default() -> Self {
        Self::Preset(Default::default())
    }
}

enum Problem {
    GetProjectDir,
    CreateProjectDir {
        path: path::PathBuf,
        err: io::Error,
    },
    ReadSettings {
        path: path::PathBuf,
        err: io::Error,
    },
    DecodeSettings {
        path: path::PathBuf,
        err: serde_json::Error,
    },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::GetProjectDir => write!(
                f,
                "Failed to find a suitable project directory to store settings"
            ),
            Problem::CreateProjectDir { path, err } => write!(
                f,
                "Failed to create the project directory at {}: {}",
                path.display(),
                err
            ),
            Problem::ReadSettings { path, err } => {
                write!(
                    f,
                    "Failed to read the settings file at {}: {}",
                    path.display(),
                    err
                )
            }
            Problem::DecodeSettings { path, err } => {
                write!(
                    f,
                    "Failed to decode settings file at {}: {}",
                    path.display(),
                    err
                )
            }
        }
    }
}
