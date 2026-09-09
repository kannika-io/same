use std::io;
use std::path::{Path, PathBuf};

use crate::context::{Context, ContextError, ContextName};

pub const CURRENT_CONFIG_VERSION: u32 = 0;
pub const CFG_FILE: &str = "config";

/// ContextRepository is a repository for storing and retrieving contexts
pub trait ContextRepository {
    /// Find a context by name
    /// Returns None if the context does not exist
    fn find_context(&self, name: &ContextName) -> Result<Option<Context>, ContextError>;

    /// Set a context
    /// If the context already exists, it will be updated
    fn set_context(&self, context: Context) -> Result<(), ContextError>;
}

/// A local repository that stores contexts in a config file
///
/// On Linux, this is stored in `$XDG_DATA_HOME/same/config`
/// On Windows, this is stored in `%APPDATA%/same/config`
/// On MacOS, this is stored in `$HOME/Library/Application Support/same/config`
///
pub struct LocalContextRepository {
    cfg_file: PathBuf,
}

impl LocalContextRepository {
    pub fn get() -> Self {
        let data_dir = dirs::data_dir().expect("Could not find data directory");
        let root = data_dir.join("io.kannika.same");
        std::fs::create_dir_all(&root).expect("Could not create config directory");
        let cfg_file = root.join(CFG_FILE);
        Self { cfg_file }
    }

    /// Set the config file. Only used for testing purposes
    #[allow(dead_code)]
    fn set_cfg_file<P: AsRef<Path>>(&mut self, p: P) {
        self.cfg_file = p.as_ref().to_path_buf();
    }
}

impl ContextRepository for LocalContextRepository {
    fn find_context(&self, name: &ContextName) -> Result<Option<Context>, ContextError> {
        // load yaml from config file
        let cfg = Config::from_file(&self.cfg_file)?;

        // find context by name
        cfg.find_context(name)
    }

    fn set_context(&self, context: Context) -> Result<(), ContextError> {
        // TODO lock file
        // load yaml from config file
        let mut cfg = Config::from_file(&self.cfg_file)?;

        // set context
        cfg.set_context(context);

        // write yaml to config file
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.cfg_file)
            .map_err(ContextError::IoError)?;

        serde_yml::to_writer(&mut file, &cfg).map_err(ContextError::SerializationError)?;

        Ok(())
    }
}

/// Config represents the user's config file.
/// The file is stored in the user's data directory.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Config {
    /// The version of the config file
    version: u32,

    /// The list of registry contexts
    registries: Vec<Context>,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    fn new() -> Self {
        Self {
            version: CURRENT_CONFIG_VERSION,
            registries: Vec::new(),
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Config, ContextError> {
        // open file, create it if it doesn't exist
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(ContextError::IoError)?;

        // if file is empty, write default config
        let file_length = file.metadata().map_err(ContextError::IoError)?.len();

        if file_length == 0 {
            let default_config = Config::new();
            serde_yml::to_writer(&mut file, &default_config)
                .map_err(ContextError::SerializationError)?;
            return Ok(default_config);
        }

        Self::from_reader(&mut file)
    }

    pub fn from_reader<R>(rdr: &mut R) -> Result<Config, ContextError>
    where
        R: io::Read,
    {
        let value: Config =
            serde_yml::from_reader(rdr).map_err(ContextError::DeserializationError)?;
        Ok(value)
    }

    pub fn find_context(&self, name: &ContextName) -> Result<Option<Context>, ContextError> {
        let context = self.registries.iter().find(|c| &c.name == name);
        Ok(context.cloned())
    }

    pub fn set_context(&mut self, context: Context) {
        let index = self.registries.iter().position(|c| c.name == context.name);
        match index {
            Some(i) => self.registries[i] = context,
            None => self.registries.push(context),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::context::{Authentication, KeychainConfig, SchemaRegistryConfig};

    use super::*;

    fn mk_temp_repo() -> (LocalContextRepository, tempfile::TempDir) {
        let mut repo = LocalContextRepository::get();
        let tempdir = tempfile::tempdir().unwrap();
        repo.set_cfg_file(tempdir.path().join(CFG_FILE));
        (repo, tempdir)
    }

    #[test]
    fn find_context_returns_none_when_context_not_found() {
        let (repo, _tempdir) = mk_temp_repo();
        let result = repo.find_context(&"nonexistent".into());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn find_context_returns_context_when_found() {
        let (repo, _tempdir) = mk_temp_repo();
        let context = Context::new("data-land".into(), data_land_registry());
        repo.set_context(context.clone()).unwrap();
        let result = repo.find_context(&"data-land".into());
        assert_eq!(result.unwrap(), Some(context));
    }

    #[test]
    fn set_context_adds_new_context() {
        let (repo, _tempdir) = mk_temp_repo();
        let context = Context::new("data-land".into(), data_land_registry());
        repo.set_context(context.clone()).unwrap();
        let result = repo.find_context(&"data-land".into());
        assert_eq!(result.unwrap(), Some(context));
    }

    #[test]
    fn set_context_updates_existing_context() {
        let (repo, _tempdir) = mk_temp_repo();

        repo.set_context(Context::new("data-land".into(), data_land_registry()))
            .unwrap();

        repo.set_context(Context::new(
            "data-land".into(),
            chocolate_factory_registry(),
        ))
        .unwrap();

        let result = repo.find_context(&"data-land".into());
        assert_eq!(
            result.unwrap(),
            Some(Context::new(
                "data-land".into(),
                chocolate_factory_registry()
            ))
        );
    }

    #[test]
    fn set_context_does_not_update_other_contexts() {
        let (repo, _tempdir) = mk_temp_repo();
        let context = Context::new("data-land".into(), data_land_registry());
        repo.set_context(context.clone()).unwrap();
        let other_context = Context::new("chocolate-factory".into(), chocolate_factory_registry());
        repo.set_context(other_context.clone()).unwrap();
        let result = repo.find_context(&"data-land".into());
        assert_eq!(result.unwrap(), Some(context));
    }

    fn data_land_registry() -> SchemaRegistryConfig {
        SchemaRegistryConfig {
            url: "http://dataland:8081".into(),
            auth: Authentication::Keychain(KeychainConfig {
                username: "alice".to_owned(),
                basic_auth_entry_name: "kannika-same-dataland".into(),
            }),
        }
    }

    fn chocolate_factory_registry() -> SchemaRegistryConfig {
        SchemaRegistryConfig {
            url: "https://chocolate-factory:8081".into(),
            auth: Authentication::None,
        }
    }
}
