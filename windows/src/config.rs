use serde::{Deserialize, Serialize};
use std::{fmt, fs, io, path};

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct Config {
    pub version: semver::Version,
    pub log_level: log::Level,
    pub flux: FluxSettings,

    // An optional path to the location of this config
    #[serde(skip)]
    location: Option<path::PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Latest version of the config
            version: semver::Version::parse("0.1.0").unwrap(),
            log_level: log::Level::Warn,
            flux: Default::default(),
            location: None,
        }
    }
}

impl Config {
    pub fn load(optional_config_dir: Option<&path::Path>) -> Self {
        match optional_config_dir {
            None => Self::default(),

            Some(config_dir) => {
                let config_path = config_dir.join("settings.json");
                let config = Self::load_existing_config(config_path.as_path());
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

                config.unwrap_or_default().attach_location(&config_path)
            }
        }
    }

    // Attach the config's location
    fn attach_location(mut self, path: &path::Path) -> Self {
        self.location = Some(path.to_owned());

        self
    }

    fn load_existing_config(config_path: &path::Path) -> Result<Config, Problem> {
        let config_string =
            fs::read_to_string(config_path).map_err(|err| Problem::ReadSettings {
                path: config_path.to_owned(),
                err,
            })?;

        serde_json::from_str(&config_string).map_err(|err| Problem::DecodeSettings {
            path: config_path.to_owned(),
            err,
        })
    }

    pub fn save(&self) -> Result<(), Problem> {
        match &self.location {
            None => Err(Problem::NoSaveLocation),
            Some(config_path) => {
                if let Some(config_dir) = config_path.parent() {
                    fs::create_dir_all(config_dir).map_err(Problem::IO)?
                }
                let config = fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(config_path)
                    .map_err(Problem::IO)?;

                serde_json::to_writer_pretty(config, self).map_err(|err| Problem::Save {
                    path: config_path.clone(),
                    err,
                })
            }
        }
    }

    pub fn to_settings(&self, wallpaper: Option<path::PathBuf>) -> flux::settings::Settings {
        use flux::settings;

        let color_mode = match &self.flux.color_mode {
            ColorMode::Preset(preset) => settings::ColorMode::Preset(*preset),
            ColorMode::DesktopImage => wallpaper.map_or(
                settings::ColorMode::default(),
                settings::ColorMode::ImageFile,
            ),
        };
        flux::settings::Settings {
            color_mode,
            ..Default::default()
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct FluxSettings {
    pub color_mode: ColorMode,
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum ColorMode {
    Preset(flux::settings::ColorPreset),
    DesktopImage,
}

impl Default for ColorMode {
    fn default() -> Self {
        Self::Preset(Default::default())
    }
}

use flux::settings::ColorPreset;
impl ColorMode {
    pub const ALL: [ColorMode; 4] = [
        ColorMode::Preset(ColorPreset::Original),
        ColorMode::Preset(ColorPreset::Plasma),
        ColorMode::Preset(ColorPreset::Poolside),
        ColorMode::DesktopImage,
    ];
}

impl std::fmt::Display for ColorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ColorMode::Preset(preset) => {
                    use flux::settings::ColorPreset::*;
                    match preset {
                        Original => "Original",
                        Plasma => "Plasma",
                        Poolside => "Poolside",
                        Freedom => "Freedom",
                    }
                }
                ColorMode::DesktopImage => "Use desktop wallpaper",
            }
        )
    }
}

#[derive(Debug)]
pub enum Problem {
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
    NoSaveLocation,
    Save {
        path: path::PathBuf,
        err: serde_json::Error,
    },
    IO(io::Error),
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
            Problem::NoSaveLocation => write!(f, "No location available to save the settings"),
            Problem::Save { path, err } => {
                write!(
                    f,
                    "Failed to save the settings to {}: {}",
                    path.display(),
                    err
                )
            }
            Problem::IO(err) => {
                write!(f, "IO error: {}", err)
            }
        }
    }
}
