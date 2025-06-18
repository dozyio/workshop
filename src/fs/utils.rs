use crate::{
    fs::Error as FsError,
    languages::{programming, spoken},
    models::workshop,
    Error,
};
use semver::Version;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::process::Command;
use tracing::{debug, info};

const APPLICATION_PARTS: [&str; 3] = ["io", "libp2p", "workshop"];

pub mod application {
    use super::*;

    /// Try to get the path to the python executable
    pub async fn find_python_executable<S: AsRef<str>>(min_version: S) -> Result<String, Error> {
        // parse the python version from the --version output
        fn parse_version(output: &str) -> Option<Version> {
            let version_str = output.rsplit_once(' ')?.1.trim();
            Version::parse(version_str).ok()
        }

        let min_version =
            Version::parse(min_version.as_ref()).map_err(|_| Error::NoPythonExecutable)?;

        // Common Python executable names
        let mut candidates = vec!["python3", "python", "py"];

        // Platform-specific candidates
        #[cfg(target_os = "windows")]
        {
            // Windows: Check for Python in common installation paths and registry
            candidates.extend(vec![
                "C:\\Python39\\python.exe",
                "C:\\Python38\\python.exe",
                "C:\\Program Files\\Python39\\python.exe",
                "C:\\Program Files\\Python38\\python.exe",
                "C:\\Users\\%USERNAME%\\AppData\\Local\\Programs\\Python\\Python39\\python.exe",
                "C:\\Users\\%USERNAME%\\AppData\\Local\\Programs\\Python\\Python38\\python.exe",
            ]);
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: Check Homebrew, system Python, and pyenv paths
            candidates.extend(vec![
                "/usr/local/bin/python3",
                "/opt/homebrew/bin/python3",
                "/usr/bin/python3",
                "/opt/local/bin/python3",
                "~/.pyenv/shims/python3",
            ]);
        }

        #[cfg(target_os = "linux")]
        {
            // Linux: Check common distro paths and pyenv
            candidates.extend(vec![
                "/usr/bin/python3",
                "/usr/local/bin/python3",
                "/bin/python3",
                "~/.pyenv/shims/python3",
            ]);
        }

        // Try each candidate
        for candidate in candidates.iter() {
            // On Windows, replace %USERNAME% with actual username
            #[cfg(target_os = "windows")]
            let candidate =
                candidate.replace("%USERNAME%", &std::env::var("USERNAME").unwrap_or_default());

            // Expand tilde (~) for home directory on Unix-like systems
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            let candidate = shellexpand::tilde(candidate).to_string();

            // Check if the executable exists and is runnable
            debug!("Checking Python candidate: {}", candidate);
            let output = Command::new(&candidate).arg("--version").output().await;

            if let Ok(output) = output {
                if output.status.success() {
                    // Verify it's a Python executable by checking version output
                    let version_output = String::from_utf8_lossy(&output.stdout);
                    if let Some(version) = parse_version(&version_output) {
                        if version >= min_version {
                            info!(
                                "v Found Python executable: {} (version: {})",
                                candidate, version
                            );
                            return Ok(candidate.to_string());
                        }
                    } else {
                        debug!(
                            "Candidate '{}' did not return a valid Python version",
                            candidate
                        );
                    }
                }
            } else {
                debug!(
                    "Failed to execute candidate '{}': {}",
                    candidate,
                    output.unwrap_err()
                );
            }
        }

        // Try querying the system for Python (Windows-specific: py launcher)
        #[cfg(target_os = "windows")]
        {
            let output = Command::new("py").arg("-0").output().await;
            if let Ok(output) = output {
                if output.status.success() {
                    let py_output = String::from_utf8_lossy(&output.stdout);
                    // Parse the output of `py -0` to find the highest Python version
                    if let Some(line) = py_output.lines().find(|line| line.contains("-3")) {
                        if let Some(version) = line.split_whitespace().next() {
                            return Ok(format!("py -{}", version.trim_start_matches('-')));
                        }
                    }
                }
            }
        }

        Err(Error::NoPythonExecutable)
    }

    /// Try to get the path to the docker compose executable
    pub async fn find_docker_compose_executable<S: AsRef<str>>(
        min_version: S,
    ) -> Result<String, Error> {
        let min_version =
            Version::parse(min_version.as_ref()).map_err(|_| Error::NoDockerComposeExecutable)?;

        // First, try to find docker executable and test if it has compose subcommand
        if let Ok(docker_compose_cmd) = try_docker_compose_plugin(&min_version).await {
            return Ok(docker_compose_cmd);
        }

        // If docker compose plugin doesn't work, try standalone docker-compose
        if let Ok(docker_compose_cmd) = try_docker_compose_standalone(&min_version).await {
            return Ok(docker_compose_cmd);
        }

        Err(Error::NoDockerComposeExecutable)
    }

    /// Try to find docker executable and test if it has compose subcommand
    async fn try_docker_compose_plugin(min_version: &Version) -> Result<String, Error> {
        // parse the python version from the --version output
        fn parse_version(output: &str) -> Option<Version> {
            let version_str = output.rsplit_once('v')?.1.trim();
            Version::parse(version_str).ok()
        }

        // Common docker executable names
        let mut docker_candidates = vec!["docker"];

        // Platform-specific docker candidates
        #[cfg(target_os = "windows")]
        {
            docker_candidates.extend(vec![
                "docker.exe",
                "C:\\Program Files\\Docker\\Docker\\resources\\bin\\docker.exe",
            ]);
        }

        #[cfg(target_os = "macos")]
        {
            docker_candidates.extend(vec![
                "/usr/local/bin/docker",
                "/opt/homebrew/bin/docker",
                "/Applications/Docker.app/Contents/Resources/bin/docker",
            ]);
        }

        #[cfg(target_os = "linux")]
        {
            docker_candidates.extend(vec![
                "/usr/bin/docker",
                "/usr/local/bin/docker",
                "/snap/bin/docker",
            ]);
        }

        for docker_cmd in docker_candidates.iter() {
            debug!("Checking docker executable: {}", docker_cmd);

            // Test if docker compose version works
            let output = Command::new(docker_cmd)
                .args(["compose", "version"])
                .output()
                .await;

            if let Ok(output) = output {
                if output.status.success() {
                    let version_output = String::from_utf8_lossy(&output.stdout);
                    debug!("Docker compose version output: {}", version_output);

                    // Parse version from "Docker Compose version v2.36.2"
                    if let Some(version) = parse_version(&version_output) {
                        if version >= *min_version {
                            info!(
                                "v Found Docker with compose plugin: {} (version: {})",
                                docker_cmd, version
                            );
                            return Ok(docker_cmd.to_string());
                        } else {
                            debug!(
                                "Docker compose version {} is below minimum {}",
                                version, min_version
                            );
                        }
                    } else {
                        debug!("Could not parse Docker Compose version from output");
                    }
                }
            } else {
                debug!(
                    "Failed to execute docker command '{}': {}",
                    docker_cmd,
                    output.unwrap_err()
                );
            }
        }

        Err(Error::NoDockerComposeExecutable)
    }

    /// Try to find standalone docker-compose executable
    async fn try_docker_compose_standalone(min_version: &Version) -> Result<String, Error> {
        // parse the python version from the --version output
        fn parse_version(output: &str) -> Option<Version> {
            let version_str = output.rsplit_once(' ')?.1.trim();
            Version::parse(version_str).ok()
        }

        // Common docker-compose executable names
        let mut docker_compose_candidates = vec!["docker-compose"];

        // Platform-specific docker-compose candidates
        #[cfg(target_os = "windows")]
        {
            docker_compose_candidates.extend(vec![
                "docker-compose.exe",
                "C:\\Program Files\\Docker\\Docker\\resources\\bin\\docker-compose.exe",
                "C:\\ProgramData\\DockerDesktop\\version-bin\\docker-compose.exe",
            ]);
        }

        #[cfg(target_os = "macos")]
        {
            docker_compose_candidates.extend(vec![
                "/usr/local/bin/docker-compose",
                "/opt/homebrew/bin/docker-compose",
                "/Applications/Docker.app/Contents/Resources/bin/docker-compose",
            ]);
        }

        #[cfg(target_os = "linux")]
        {
            docker_compose_candidates.extend(vec![
                "/usr/bin/docker-compose",
                "/usr/local/bin/docker-compose",
                "/snap/bin/docker-compose",
            ]);
        }

        for docker_compose_cmd in docker_compose_candidates.iter() {
            debug!("Checking docker-compose executable: {}", docker_compose_cmd);

            // Test if docker-compose --version works
            let output = Command::new(docker_compose_cmd)
                .arg("--version")
                .output()
                .await;

            if let Ok(output) = output {
                if output.status.success() {
                    let version_output = String::from_utf8_lossy(&output.stdout);
                    debug!("Docker-compose version output: {}", version_output);

                    // Parse version from "docker-compose version 1.29.2"
                    if let Some(version) = parse_version(&version_output) {
                        if version >= *min_version {
                            info!(
                                "Found docker-compose standalone: {} (version: {})",
                                docker_compose_cmd, version
                            );
                            return Ok(docker_compose_cmd.to_string());
                        } else {
                            debug!(
                                "Docker-compose version {} is below minimum {}",
                                version, min_version
                            );
                        }
                    } else {
                        debug!("Could not parse docker-compose version from output");
                    }
                }
            }
        }

        Err(Error::NoDockerComposeExecutable)
    }

    /// Get the application data directory. This works on Windows, macOS, and Linux.
    pub fn data_dir() -> Result<PathBuf, Error> {
        let data_dir = if let Ok(workshops_dir) = std::env::var("WORKSHOPS_DIR") {
            PathBuf::from(workshops_dir)
        } else {
            directories::ProjectDirs::from(
                APPLICATION_PARTS[0],
                APPLICATION_PARTS[1],
                APPLICATION_PARTS[2],
            )
            .map(|dirs| dirs.data_dir().to_path_buf())
            .ok_or(FsError::ApplicationDirsNotFound)?
        };

        // create the data directory if it doesn't exist
        std::fs::create_dir_all(&data_dir)?;

        Ok(data_dir)
    }

    /// Get the application config directory. This works on Windows, macOS, and Linux.
    pub fn config_dir() -> Result<PathBuf, Error> {
        let config_dir = directories::ProjectDirs::from(
            APPLICATION_PARTS[0],
            APPLICATION_PARTS[1],
            APPLICATION_PARTS[2],
        )
        .map(|dirs| dirs.config_dir().to_path_buf())
        .ok_or(FsError::ApplicationDirsNotFound)?;

        // create the config directory if it doesn't exist
        std::fs::create_dir_all(&config_dir)?;

        Ok(config_dir)
    }

    /// Get all of the workshops data objects for all workshops in the application data directory
    pub fn all_workshops() -> Result<HashMap<String, workshop::WorkshopData>, Error> {
        let mut workshops_data = workshops::load_workshop_data(data_dir()?)?;
        if let Some(workshops_dir) = workshops::data_dir() {
            // If the workshops directory exists, load the workshop data from there
            workshops_data.extend(workshops::load_workshop_data(workshops_dir)?);
        }
        Ok(workshops_data)
    }

    /// Get all of the workshops in the application data directory, that support the given spoken
    /// and programming languages
    pub fn all_workshops_filtered(
        spoken_language: Option<spoken::Code>,
        programming_language: Option<programming::Code>,
    ) -> Result<HashMap<String, workshop::WorkshopData>, Error> {
        let workshops = all_workshops()?;
        Ok(workshops
            .into_iter()
            .filter(|(_, workshop_data)| {
                workshop_data.is_selected(spoken_language, programming_language)
            })
            .collect())
    }

    /// Get all of the spoken languages supported by all workshops in the application data
    /// directory
    pub fn all_spoken_languages() -> Result<Vec<spoken::Code>, Error> {
        let mut spoken_languages: Vec<spoken::Code> = all_workshops()?
            .values()
            .flat_map(|workshop| workshop.get_all_spoken_languages())
            .collect::<Vec<_>>();
        spoken_languages.sort();
        spoken_languages.dedup();
        Ok(spoken_languages)
    }

    /// Get all of the programming languages supported by all workshops in the application data
    /// directory
    pub fn all_programming_languages() -> Result<Vec<programming::Code>, Error> {
        let mut programming_languages: Vec<programming::Code> = all_workshops()?
            .values()
            .flat_map(|workshop| workshop.get_all_programming_languages())
            .collect::<Vec<_>>();
        programming_languages.sort();
        programming_languages.dedup();
        Ok(programming_languages)
    }

    /// Get all of the spoken to programming language mappings for all workshops in the application
    /// data directory
    pub fn get_all_languages() -> Result<HashMap<spoken::Code, Vec<programming::Code>>, Error> {
        let mut languages: HashMap<spoken::Code, Vec<programming::Code>> = HashMap::new();
        for workshop in all_workshops()?.values() {
            let workshop_languages = workshop.get_all_languages();
            for (spoken_lang, programming_langs) in workshop_languages {
                languages
                    .entry(*spoken_lang)
                    .and_modify(|langs| {
                        langs.extend(programming_langs.iter());
                        langs.sort();
                        langs.dedup();
                    })
                    .or_insert(programming_langs.clone());
            }
        }
        Ok(languages)
    }
}

pub mod workshops {
    use super::*;

    // recursively copy the folder from the source path to the target path
    fn copy_tree<P: AsRef<Path>>(source: P, target: P) -> Result<(), Error> {
        let source = source.as_ref();
        let target = target.as_ref();

        if !source.exists() || !source.is_dir() {
            return Err(FsError::WorkshopDataDirNotFound.into());
        }

        // create the target directory if it doesn't exist
        std::fs::create_dir_all(target)?;

        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            let entry_path = entry.path();
            let target_path = target.join(entry.file_name());

            if entry_path.is_dir() {
                copy_tree(entry_path, target_path)?;
            } else {
                std::fs::copy(entry_path, target_path)?;
            }
        }
        Ok(())
    }

    /// Initialize the present working directory (pwd) by creating a `.workshops` directory, if
    /// missing, and then recursively copying the selected workshop from the application data
    /// directory to the `.workshops` directory. Then return the path to the `.workshops`
    /// directory.
    pub fn init_data_dir<S: AsRef<str>>(workshop: S) -> Result<PathBuf, Error> {
        // get the pwd
        let pwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let workshops_dir = pwd.join(".workshops");

        // Create the workshops directory if it doesn't exist
        std::fs::create_dir_all(&workshops_dir)?;

        // Copy the selected workshop to the workshops directory
        let data_dir = application::data_dir()?;
        let workshop_path = data_dir.join(workshop.as_ref());
        if workshop_path.exists() && workshop_path.is_dir() {
            let target_path = workshops_dir.join(workshop.as_ref());
            debug!(
                "Copying workshop data from {} to {}",
                workshop_path.display(),
                target_path.display()
            );
            copy_tree(workshop_path, target_path)?;
        } else {
            return Err(FsError::WorkshopDataDirNotFound.into());
        }

        Ok(workshops_dir)
    }

    /// Get the path to the `.workshops` directory by starting in the pwd and searching for the
    /// `.workshops` directory. Recursively search the parent directories until either the
    /// `.workshops` directory is found or the root directory is reached.
    pub fn data_dir() -> Option<PathBuf> {
        let mut current_dir = std::env::current_dir().ok()?;
        loop {
            let workshops_dir = current_dir.join(".workshops");
            if workshops_dir.exists() && workshops_dir.is_dir() {
                return Some(workshops_dir);
            }
            if !current_dir.pop() {
                break; // reached the root directory
            }
        }
        None
    }

    /// Get the given workshop in the `.workshops` directory, if it exists.
    pub fn load<S: AsRef<str>>(workshop: S) -> Option<workshop::WorkshopData> {
        let workshops_dir = data_dir()?;
        let workshop_path = workshops_dir.join(workshop.as_ref());
        if workshop_path.exists() && workshop_path.is_dir() {
            return workshop::Loader::new(workshop.as_ref())
                .path(&workshops_dir)
                .try_load()
                .ok();
        }
        None
    }

    /// Get all workshop data objects for workshops in the given folder
    pub fn load_workshop_data<T: AsRef<Path>>(
        data_dir: T,
    ) -> Result<HashMap<String, workshop::WorkshopData>, Error> {
        let data_dir = data_dir.as_ref();
        if !data_dir.exists() || !data_dir.is_dir() {
            return Err(FsError::WorkshopDataDirNotFound.into());
        }

        let mut workshops = HashMap::new();
        for entry in std::fs::read_dir(data_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                let workshop_name = entry.file_name().to_string_lossy().to_string();
                info!("... {workshop_name}");
                let workshop_data = workshop::Loader::new(&workshop_name)
                    .path(data_dir)
                    .try_load()?;
                workshops.insert(workshop_name, workshop_data);
            }
        }
        Ok(workshops)
    }
}
