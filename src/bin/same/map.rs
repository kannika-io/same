use std::collections::BTreeMap;
use std::io::Write;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use std::{fmt, io};

use clap::Args;
use dialoguer::console::Emoji;
use indicatif::{ProgressState, ProgressStyle};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use same::context::{
    Authentication, Context, ContextError, ContextName, ContextRepository,
    DownloadAllSchemaFilesOpts, DownloadProbe, LocalContextRepository, SchemaRegistryConfig,
};
use same::mapping::conflict::ConflictResolutionStrategy;
use same::mapping::fingerprint::FingerprintOpts;
use same::mapping::json::{DEFAULT_IGNORED_KEYWORDS, JsonCanonicalOpts};
use same::mapping::{map_schemas, MapSchemasOpts};
use same::registry::{SchemaId, SchemaReference, SchemaVersion, SubjectName};

use crate::map::MapError::ContextNotFound;

#[derive(Args, Debug)]
pub struct MapCommand {
    /// The name of the context to map from
    #[arg(long)]
    from: String,
    /// The name of the context to map to
    #[arg(long)]
    to: String,
    /// Output file. Optional; if not specified, output is written to stdout
    #[arg(long, short = 'o')]
    output: Option<String>,

    /// Ignore the local cache and download all schemas again
    #[arg(long, short = 'U')]
    force_update: bool,

    /// Work offline, do not download schemas
    #[arg(long)]
    offline: bool,

    /// Ignore errors when indexing schemas.
    /// If this flag is set, the command will continue to index schemas even if some schemas fail to map
    #[arg(long)]
    ignore_indexing_errors: bool,

    #[arg(long)]
    // File containing a list of registries to use for mapping
    registries: Option<String>,

    #[arg(long)]
    on_conflict: ConflictResolutionStrategy,

    /// JSON Schema keywords to ignore when fingerprinting.
    /// Passing the flag replaces the default list; repeat it or separate keywords with commas
    #[arg(
        long,
        value_name = "KEYWORD",
        value_delimiter = ',',
        default_values_t = DEFAULT_IGNORED_KEYWORDS.iter().map(|k| k.to_string())
    )]
    json_ignore_keyword: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Registries {
    #[serde(default)]
    registries: Vec<RegistryConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegistryConfig {
    name: String,
    url: String,
    username: Option<String>,
    password: Option<String>,
}

impl TryFrom<RegistryConfig> for Context {
    type Error = MapError;

    fn try_from(config: RegistryConfig) -> Result<Self, Self::Error> {
        let auth = if let (Some(username), Some(password)) = (config.username, config.password) {
            Authentication::BasicAuth { username, password }
        } else {
            Authentication::None
        };

        Ok(Context::new(
            config.name.parse().unwrap(),
            SchemaRegistryConfig {
                url: config.url,
                auth,
            },
        ))
    }
}

impl Registries {
    fn get(&self, name: &str) -> Result<RegistryConfig, MapError> {
        self.registries
            .iter()
            .find(|r| r.name == name)
            .cloned()
            .ok_or_else(|| MapError::ContextNotFound(name.into()))
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MappingOutput {
    mapping: BTreeMap<SchemaId, SchemaId>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    missed: Vec<MissedSchema>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MissedSchema {
    id: SchemaId,
    subject: SubjectName,
    version: SchemaVersion,
    schema: String,
    fingerprint: Option<String>,
    references: Vec<SchemaReference>,
}

#[derive(Debug, thiserror::Error)]
pub enum MapError {
    #[error("Context not found: {0}")]
    ContextNotFound(ContextName),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Context error: {0}")]
    ContextError(#[from] ContextError),

    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_yml::Error),
}

impl MapCommand {
    pub async fn run(&self) -> anyhow::Result<()> {
        let (from_ctx, to_ctx) = self.get_contexts()?;

        if from_ctx == to_ctx {
            return Err(anyhow::anyhow!("Cannot map a registry to itself"));
        }

        if self.offline {
            step(
                1,
                Emoji("🚚 ", ""),
                "Working offline, using locally cached schemas...",
            );
        } else {
            step(1, Emoji("🚚 ", ""), "Downloading schemas...");
            self.download_schemas(&from_ctx, &to_ctx).await?;
        }

        step(2, Emoji("🔎 ", ""), "Mapping schemas...");
        let mapping = map_schemas(
            from_ctx.clone(),
            to_ctx.clone(),
            MapSchemasOpts {
                ignore_indexing_errors: self.ignore_indexing_errors,
                on_conflict: self.on_conflict,
                fingerprint: FingerprintOpts {
                    json: JsonCanonicalOpts {
                        ignored_keywords: self.json_ignore_keyword.iter().cloned().collect(),
                    },
                },
            },
        )
        .await?;

        step(3, Emoji("🖨️ ", ""), "Printing mapping...");
        serde_yml::to_writer(
            self.output(),
            &MappingOutput {
                mapping: mapping.matched().to_owned(),
                missed: mapping
                    .missed()
                    .iter()
                    .map(|schema| MissedSchema {
                        id: schema.id.clone(),
                        subject: schema.subject.clone(),
                        version: schema.version.clone(),
                        schema: schema.schema.clone(),
                        fingerprint: schema.fingerprint.get_value_opt(),
                        references: schema.references.clone(),
                    })
                    .collect(),
            },
        )?;

        if mapping.missed().is_empty() {
            writeln!(io::stderr(), "All schemas mapped successfully!")?;
        } else {
            writeln!(io::stderr(), "Some schemas failed to map.")?;
        }

        step(4, Emoji("💫", ""), "Done");

        Ok(())
    }

    fn get_contexts(&self) -> Result<(Arc<Context>, Arc<Context>), MapError> {
        match self.registries {
            Some(ref path) => {
                let file = std::fs::File::open(path)?;
                let registries: Registries = serde_yml::from_reader(file)?;
                let from = registries.get(&self.from)?;
                let to = registries.get(&self.to)?;
                Ok((Arc::new(from.try_into()?), Arc::new(to.try_into()?)))
            }
            None => {
                let repo = LocalContextRepository::get();
                let from: ContextName = self.from.clone().into();
                let to: ContextName = self.to.clone().into();
                let from_ctx = Arc::new(
                    repo.find_context(&from)?
                        .ok_or_else(|| ContextNotFound(from))?,
                );
                let to_ctx = Arc::new(
                    repo.find_context(&self.to.clone().into())?
                        .ok_or_else(|| ContextNotFound(to))?,
                );
                Ok((from_ctx, to_ctx))
            }
        }
    }

    pub async fn download_schemas(
        &self,
        from_ctx: &Arc<Context>,
        to_ctx: &Arc<Context>,
    ) -> Result<(), ContextError> {
        let progress = Arc::new(indicatif::MultiProgress::new());
        let (download_source_task, download_target_task) = tokio::join!(
            self.spawn_download_task(from_ctx.clone(), progress.clone()),
            self.spawn_download_task(to_ctx.clone(), progress.clone()),
        );

        flatten(download_source_task).await?;
        flatten(download_target_task).await?;
        Ok(())
    }

    async fn spawn_download_task(
        &self,
        ctx: Arc<Context>,
        multi_progress_bar: Arc<indicatif::MultiProgress>,
    ) -> DownloadTask {
        let ignore_cache = self.force_update;
        tokio::spawn(async move {
            let progress_bar = DownloadProgressBar::from_multi(multi_progress_bar.clone());
            let opts = DownloadAllSchemaFilesOpts {
                ignore_cache,
                probe: Some(progress_bar),
            };
            ctx.download_all_schema_files(opts).await?;
            Ok(())
        })
    }
}

impl MapCommand {
    fn output(&self) -> Box<dyn io::Write> {
        match self.output {
            Some(ref path) => Box::new(std::fs::File::create(path).unwrap()),
            None => Box::new(std::io::stdout()),
        }
    }
}

pub struct DownloadProgressBar {
    progress_bar: indicatif::ProgressBar,
}

impl DownloadProgressBar {
    pub fn from_multi(multi_progress_bar: Arc<indicatif::MultiProgress>) -> Self {
        let progress_bar = multi_progress_bar.add(indicatif::ProgressBar::new_spinner());
        progress_bar.enable_steady_tick(Duration::from_millis(100));
        progress_bar.tick();
        progress_bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] {msg} [{wide_bar:.cyan/blue}] ({eta})",
            )
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn fmt::Write| {
                write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
            })
            .progress_chars("#>-"),
        );
        Self { progress_bar }
    }
}

impl DownloadProbe for DownloadProgressBar {
    fn total(&self, total: u64) {
        self.progress_bar.set_length(total);
    }

    fn downloading(&self, name: &ContextName, subject: &SubjectName, version: &SchemaVersion) {
        fn substr(s: &str, start: usize, end: usize) -> String {
            match s.char_indices().nth(start) {
                Some((start_idx, _)) => match s.char_indices().nth(end) {
                    Some((end_idx, _)) => s[start_idx..end_idx].to_string(),
                    None => s[start_idx..].to_string(),
                },
                None => String::new(),
            }
        }

        let message = format!(
            "{:<8} / {:<12} / {:<3}",
            if name.len() > 8 {
                format!("{:.5}...", substr(name.deref(), 0, 5))
            } else {
                format!("{:.8}", name)
            },
            if subject.len() > 12 {
                format!("{:.9}...", substr(subject.deref(), 0, 9))
            } else {
                format!("{:.12}", subject)
            },
            format!("{:.5}", version)
        );

        self.progress_bar.set_message(message);
    }

    fn inc(&self, progress: u64) {
        self.progress_bar.inc(progress);
    }

    fn finished(&self) {
        self.progress_bar.finish_and_clear();
    }
}

fn step(number: usize, emoji: Emoji, message: &str) {
    writeln!(io::stderr(), "[{}/4] {} {}", number, emoji, message,).unwrap();
}

type DownloadTask = JoinHandle<DownloadTaskResult>;
type DownloadTaskResult = Result<(), ContextError>;

async fn flatten(handle: DownloadTask) -> DownloadTaskResult {
    match handle.await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => Err(err),
        Err(err) => panic!("Failed to download schemas: {:?}", err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU32;

    #[test]
    fn mapping_output_yaml_without_missed() {
        let mut mapping = BTreeMap::new();
        mapping.insert(SchemaId::from(1u32), SchemaId::from(10u32));
        mapping.insert(SchemaId::from(2u32), SchemaId::from(20u32));

        let output = MappingOutput {
            mapping,
            missed: vec![],
        };

        let yaml = serde_yml::to_string(&output).unwrap();
        assert_eq!(yaml, "mapping:\n  1: 10\n  2: 20\n");
    }

    #[test]
    fn mapping_output_yaml_with_missed() {
        let output = MappingOutput {
            mapping: BTreeMap::new(),
            missed: vec![MissedSchema {
                id: SchemaId::from(42u32),
                subject: "my-subject".parse().unwrap(),
                version: SchemaVersion::Version(NonZeroU32::new(3).unwrap()),
                schema: r#"{"type":"string"}"#.to_string(),
                fingerprint: Some("abc123".to_string()),
                references: vec![SchemaReference {
                    name: "ref1".to_string(),
                    subject: "ref-subject".to_string(),
                    version: SchemaVersion::Version(NonZeroU32::new(1).unwrap()),
                }],
            }],
        };

        let yaml = serde_yml::to_string(&output).unwrap();
        assert_eq!(
            yaml,
            "\
mapping: {}
missed:
- id: 42
  subject: my-subject
  version: 3
  schema: '{\"type\":\"string\"}'
  fingerprint: abc123
  references:
  - name: ref1
    subject: ref-subject
    version: 1
"
        );
    }

    #[test]
    fn mapping_output_skips_empty_missed() {
        let output = MappingOutput {
            mapping: BTreeMap::new(),
            missed: vec![],
        };

        let yaml = serde_yml::to_string(&output).unwrap();
        assert!(!yaml.contains("missed"), "empty missed should be omitted from YAML");
    }
}
