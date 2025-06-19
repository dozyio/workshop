use crate::{
    fs,
    languages::{programming, spoken},
    Config, Error,
};
use serde::{Deserialize, Serialize};
use tracing::{info, info_span};

/// This stores the currently active context for the application. It includes the spoken language,
/// programming language, selected workshop, and selected lesson. It serialzies to the status.yaml
/// file inside of the .workshops directory inside of your working directory. it is innitialized
/// from the Config object when first created.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Status {
    python_executable: Option<String>,
    docker_compose_executable: Option<String>,
    git_executable: Option<String>,
    spoken_language: Option<spoken::Code>,
    programming_language: Option<programming::Code>,
    workshop: Option<String>,
    lesson: Option<String>,
    #[serde(skip)]
    config: Config,
}

impl Status {
    /// load/create status
    pub fn load() -> Result<Self, Error> {
        let span = info_span!("Config");
        let _enter = span.enter();

        let config = Config::load()?;
        if let Some(path) = fs::workshops::data_dir().map(|d| d.join("status.yaml")) {
            if path.exists() {
                // try to load it from the file
                let mut status: Status = serde_yaml::from_reader(std::fs::File::open(&path)?)?;
                status.config = config;
                return Ok(status);
            }
        }

        // otherwise, create the status
        Ok(Status {
            python_executable: config.python_executable(),
            docker_compose_executable: config.docker_compose_executable(),
            git_executable: config.git_executable(),
            spoken_language: config.spoken_language(),
            programming_language: config.programming_language(),
            workshop: None,
            lesson: None,
            config,
        })
    }

    /// save the status to the given path
    pub fn save(&self) -> Result<(), Error> {
        // if there is a workshops data directory, save the status there
        if let Some(path) = fs::workshops::data_dir().map(|d| d.join("status.yaml")) {
            std::fs::create_dir_all(path.parent().unwrap())?;
            info!("Status saved to: {}", path.display());
            serde_yaml::to_writer(std::fs::File::create(path)?, &self)?;
        }
        // save the config as well
        self.config.save()?;
        Ok(())
    }

    /// Get the minimum required Python version
    pub fn python_minimum_version(&self) -> &str {
        self.config.python_minimum_version()
    }

    /// Get the preferred Python executable
    pub fn python_executable(&self) -> Option<&str> {
        self.python_executable.as_deref()
    }

    /// Get the minimum required Docker Compose version
    pub fn docker_compose_minimum_version(&self) -> &str {
        self.config.docker_compose_minimum_version()
    }

    /// Get the preferred Docker Compose executable
    pub fn docker_compose_executable(&self) -> Option<&str> {
        self.docker_compose_executable.as_deref()
    }

    /// Get the preferred Git executable
    pub fn git_executable(&self) -> Option<&str> {
        self.git_executable.as_deref()
    }

    /// Get the minimum required Git version
    pub fn git_minimum_version(&self) -> &str {
        self.config.git_minimum_version()
    }

    /// Get the preferred spoken language
    pub fn spoken_language(&self) -> Option<spoken::Code> {
        self.spoken_language
    }

    /// Get the preferred programming language
    pub fn programming_language(&self) -> Option<programming::Code> {
        self.programming_language
    }

    /// Get the selected workshop
    pub fn workshop(&self) -> Option<&str> {
        self.workshop.as_deref()
    }

    /// Get the selected lesson
    pub fn lesson(&self) -> Option<&str> {
        self.lesson.as_deref()
    }

    /// Set the preferred Python executable with optional default
    pub fn set_python_executable(&mut self, python_executable: &str, default: bool) {
        self.python_executable = Some(python_executable.to_string());
        if default {
            self.config.set_python_executable(python_executable);
        }
    }

    /// Set the preferred Docker Compose executable with optional default
    pub fn set_docker_compose_executable(
        &mut self,
        docker_compose_executable: &str,
        default: bool,
    ) {
        self.docker_compose_executable = Some(docker_compose_executable.to_string());
        if default {
            self.config
                .set_docker_compose_executable(docker_compose_executable);
        }
    }

    /// Set the preferred Git executable with optional default
    pub fn set_git_executable(&mut self, git_executable: &str, default: bool) {
        self.git_executable = Some(git_executable.to_string());
        if default {
            self.config.set_git_executable(git_executable);
        }
    }

    /// Set the spoken language with optional default
    pub fn set_spoken_language(&mut self, spoken_language: Option<spoken::Code>, default: bool) {
        self.spoken_language = spoken_language;
        if default {
            self.config.set_spoken_language(spoken_language);
        }
    }

    /// Set the programming language with optional default
    pub fn set_programming_language(
        &mut self,
        programming_language: Option<programming::Code>,
        default: bool,
    ) {
        self.programming_language = programming_language;
        if default {
            self.config.set_programming_language(programming_language);
        }
    }

    /// Set the selected workshop
    pub fn set_workshop(&mut self, workshop: Option<String>) {
        self.workshop = workshop;
    }

    /// Set the selected lesson
    pub fn set_lesson(&mut self, lesson: Option<String>) {
        self.lesson = lesson;
    }
}
