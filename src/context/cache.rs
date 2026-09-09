use crate::context::{Context, ContextError, ContextName};
use crate::registry::GetSchemaRegistryClient;
use crate::registry::{ListSubjectsOptions, SchemaReference, SchemaVersion, Subject, SubjectName};
use std::fmt::Display;
use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DownloadAllSchemaFilesOpts<P: DownloadProbe> {
    // Force update of all schemas
    pub ignore_cache: bool,
    pub probe: Option<P>,
}

impl<P> Default for DownloadAllSchemaFilesOpts<P>
where
    P: DownloadProbe,
{
    fn default() -> Self {
        Self {
            ignore_cache: false,
            probe: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WalkSchemaSubjectsOpts {
    pub ignore_errors: bool,
}

pub trait DownloadProbe {
    fn total(&self, total: u64);
    fn downloading(&self, name: &ContextName, subject: &SubjectName, version: &SchemaVersion);
    fn inc(&self, progress: u64);
    fn finished(&self);
}

impl Context {
    /// Returns the path to the cache directory for this context.
    pub fn cache_dir(&self) -> Result<PathBuf, ContextError> {
        let dir = match &self.cache_dir_override {
            Some(base) => {
                let mut path = base.clone();
                path.push(self.name.deref());
                path
            }
            None => dirs::cache_dir()
                .map(|mut path| {
                    path.push("io.kannika.same");
                    path.push(self.name.deref());
                    path
                })
                .ok_or(ContextError::CacheDirCreationFailed)?,
        };

        mkdir_p(&dir)
    }

    /// Downloads all schemas from the schema registry and stores them in the cache directory.
    pub async fn download_all_schema_files<P: DownloadProbe>(
        &self,
        opts: DownloadAllSchemaFilesOpts<P>,
    ) -> Result<(), ContextError> {
        let cache_dir = self.cache_dir()?;
        let client = self.get_client()?;

        let subjects = client
            .subject()
            .list(ListSubjectsOptions::default())
            .await
            .map_err(ContextError::SchemaRegistryError)?;

        if let Some(probe) = opts.probe.as_ref() {
            probe.total(subjects.len() as u64);
        }

        for subject in subjects {
            tracing::debug!("Downloading all schemas for subject {}", subject);

            let subject_cache_dir = mkdir_p(cache_dir.join(subject.deref()))?;

            let versions = client
                .subject()
                .versions(&subject)
                .await
                .map_err(ContextError::SchemaRegistryError)?;

            tracing::debug!("Found {} versions for subject {}", versions.len(), subject);

            for version in versions {
                tracing::debug!("Downloading subject {} (version {})", subject, version);

                if let Some(probe) = opts.probe.as_ref() {
                    probe.downloading(&self.name, &subject, &version);
                }

                // Check if we already have this schema cached
                let schema_file = subject_cache_dir.join(version.to_string());

                if !opts.ignore_cache && schema_file.exists() {
                    tracing::debug!("Schema already cached at {}", schema_file.display());
                    continue;
                }

                let schema = client.subject().version(&subject, version).await?;

                if let Some(schema) = schema {
                    let schema_file = subject_cache_dir.join(version.to_string());

                    tracing::debug!(
                        "Writing subject {} (version {}) with id {} to {}",
                        schema.subject,
                        schema.version,
                        schema.id,
                        schema_file.display()
                    );

                    let file = std::fs::File::create(schema_file).map_err(ContextError::IoError)?;

                    serde_yml::to_writer(file, &schema)
                        .map_err(ContextError::SerializationError)?;
                } else {
                    tracing::debug!(
                        "No schema found for subject {} version {}",
                        subject,
                        version
                    )
                }
            }

            if let Some(probe) = opts.probe.as_ref() {
                probe.inc(1);
            }
        }

        if let Some(probe) = opts.probe.as_ref() {
            probe.finished();
        }

        Ok(())
    }

    /// Walks all schema subjects in the cache directory and calls the given function for each subject.
    pub async fn walk_schema_subjects<T, E: Display>(
        &self,
        mut f: impl FnMut(Subject) -> Result<T, E>,
        opts: WalkSchemaSubjectsOpts,
    ) -> Result<(), ContextError> {
        let cache_dir = self.cache_dir()?;

        for entry in fs::read_dir(cache_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_file() {
                        let file = fs::File::open(&path)?;

                        match serde_yml::from_reader(&file) {
                            Ok(subject) => {
                                f(subject).map_err(|e| ContextError::WalkError(e.to_string()))?;
                            }
                            Err(e) => {
                                if opts.ignore_errors {
                                    tracing::warn!("Error reading subject file {:?}: {}", file, e);
                                } else {
                                    return Err(ContextError::DeserializationError(e));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get_subject(
        &self,
        reference: &SchemaReference,
    ) -> Result<Option<Subject>, ContextError> {
        let cache_dir = self.cache_dir()?;

        let subject_cache_dir = cache_dir.join(reference.subject.deref());

        let schema_file = subject_cache_dir.join(reference.version.to_string());

        if schema_file.exists() {
            let file = fs::File::open(schema_file)?;

            let subject: Subject =
                serde_yml::from_reader(file).map_err(ContextError::DeserializationError)?;

            Ok(Some(subject))
        } else {
            Ok(None)
        }
    }
}

fn mkdir_p<P: AsRef<Path>>(path: P) -> Result<PathBuf, ContextError> {
    let path = path.as_ref();

    fs::create_dir_all(path).map_err(ContextError::IoError)?;

    Ok(path.to_path_buf())
}

pub struct EmptyDownloadProbe {}

impl DownloadProbe for EmptyDownloadProbe {
    fn total(&self, _total: u64) {}
    fn downloading(&self, _name: &ContextName, _subject: &SubjectName, _version: &SchemaVersion) {}
    fn inc(&self, _progress: u64) {}
    fn finished(&self) {}
}
