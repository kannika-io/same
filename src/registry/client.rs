use crate::registry::{
    ApiError, RegisterSchema, RegisteredSchema, Schema, SchemaId, SchemaType, SchemaVersion,
    Subject, SubjectName,
};
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue};
use reqwest::{Client, Response, StatusCode, Url};
use reqwest_middleware::ClientWithMiddleware;
use reqwest_tracing::TracingMiddleware;
use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Debug)]
pub struct SchemaRegistryClient {
    url: Url,
    client: ClientWithMiddleware,
}

#[derive(Debug, thiserror::Error)]
pub enum SchemaRegistryClientError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// An error when building an URL
    #[error(transparent)]
    UrlError(#[from] url::ParseError),

    #[error("Keyring error: {0}")]
    KeyringError(#[from] keyring::Error),

    /// An error with reqwest
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    /// An error with reqwest middleware
    #[error(transparent)]
    ReqwestMiddlewareError(#[from] reqwest_middleware::Error),

    /// An API error
    #[error(transparent)]
    ApiError(#[from] ApiError),

    /// A schema registry error
    #[error("Schema registry error {0}")]
    SchemaRegistryError(String),
}

impl SchemaRegistryClient {
    fn parse_url(url: &str) -> Result<Url, SchemaRegistryClientError> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return SchemaRegistryClient::parse_url(format!("https://{}", url).as_str());
        }
        Url::parse(url).map_err(|_| SchemaRegistryClientError::InvalidUrl(url.to_string()))
    }

    pub fn new(url: &str) -> Result<Self, SchemaRegistryClientError> {
        let url = Self::parse_url(url)?;

        Self::build_default(url)
    }

    pub fn new_with_basic_auth(
        url: &str,
        username: &str,
        password: &str,
    ) -> Result<Self, SchemaRegistryClientError> {
        let mut url = Self::parse_url(url)?;

        url.set_username(username)
            .map_err(|_| SchemaRegistryClientError::InvalidUrl(url.to_string()))?;

        url.set_password(Some(password))
            .map_err(|_| SchemaRegistryClientError::InvalidUrl(url.to_string()))?;

        Self::build_default(url)
    }

    pub fn build_default(url: Url) -> Result<Self, SchemaRegistryClientError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.schemaregistry.v1+json"),
        );
        let reqwest_client = Client::builder()
            .default_headers(headers)
            .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
            .build()?;
        let client = reqwest_middleware::ClientBuilder::new(reqwest_client)
            .with(TracingMiddleware::default())
            .build();

        Ok(Self { url, client })
    }

    /// Subject client
    #[must_use]
    pub fn subject(&'_ self) -> SubjectClient<'_> {
        SubjectClient { client: self }
    }

    /// Schema client
    #[must_use]
    pub fn schema(&self) -> SchemaClient<'_> {
        SchemaClient { client: self }
    }

    async fn get<T>(&self, url: Url) -> Result<T, SchemaRegistryClientError>
    where
        T: DeserializeOwned,
    {
        let response = self.client.get(url).send().await?;
        handle_response(response).await
    }

    async fn post<R, B>(&self, url: Url, body: &R) -> Result<B, SchemaRegistryClientError>
    where
        R: Serialize,
        B: DeserializeOwned,
    {
        let response = self.client.post(url).json(body).send().await?;
        handle_response(response).await
    }

    async fn get_optional<T>(&self, url: Url) -> Result<Option<T>, SchemaRegistryClientError>
    where
        T: DeserializeOwned,
    {
        let response = self.client.get(url).send().await?;
        handle_optional_response(response).await
    }

    async fn delete_optional<B>(&self, url: Url) -> Result<Option<B>, SchemaRegistryClientError>
    where
        B: DeserializeOwned,
    {
        let response = self.client.delete(url).send().await?;
        handle_optional_response(response).await
    }
}

pub trait GetSchemaRegistryClient {
    fn get_client(&self) -> Result<SchemaRegistryClient, SchemaRegistryClientError>;
}

async fn handle_response<T>(response: Response) -> Result<T, SchemaRegistryClientError>
where
    T: DeserializeOwned,
{
    if response.status().is_success() {
        let result = response.json().await?;
        Ok(result)
    } else {
        let err = handle_error(response).await;
        Err(err)
    }
}

async fn handle_optional_response<T>(
    response: Response,
) -> Result<Option<T>, SchemaRegistryClientError>
where
    T: DeserializeOwned,
{
    let status = response.status();
    if status.is_success() {
        let result = response.json().await?;
        Ok(Some(result))
    } else if status == StatusCode::NOT_FOUND || status == StatusCode::NO_CONTENT {
        Ok(None)
    } else {
        let err = handle_error(response).await;
        Err(err)
    }
}

async fn handle_error(response: Response) -> SchemaRegistryClientError {
    let body = response.text().await.unwrap_or_default();
    if let Ok(error) = serde_json::from_str::<ApiError>(&body) {
        SchemaRegistryClientError::ApiError(error)
    } else {
        SchemaRegistryClientError::SchemaRegistryError(body)
    }
}

pub struct SchemaClient<'client> {
    pub(super) client: &'client SchemaRegistryClient,
}

impl SchemaClient<'_> {
    #[tracing::instrument(skip(self))]
    pub async fn get(
        &self,
        id: SchemaId,
        subject: Option<&SubjectName>,
    ) -> Result<Option<Schema>, SchemaRegistryClientError> {
        let path = format!("schemas/ids/{id}");
        let mut url = self.client.url.join(&path)?;
        if let Some(subject) = subject {
            let query = format!("subject={subject}");
            url.set_query(Some(&query));
        }
        self.client.get_optional(url).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_schema(
        &self,
        id: SchemaId,
        subject: Option<SubjectName>,
    ) -> Result<Option<String>, SchemaRegistryClientError> {
        let path = format!("schemas/ids/{id}/schema");
        let mut url = self.client.url.join(&path)?;
        if let Some(subject) = subject {
            let query = format!("subject={subject}");
            url.set_query(Some(&query));
        }
        self.client.get_optional(url).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn types(&self) -> Result<Vec<SchemaType>, SchemaRegistryClientError> {
        let url = self.client.url.join("schemas/types")?;
        self.client.get(url).await
    }
}

/// The subject client
#[derive(Debug)]
pub struct SubjectClient<'client> {
    pub(super) client: &'client SchemaRegistryClient,
}

/// Options for listing subjects
#[derive(Debug, Default)]
pub struct ListSubjectsOptions {
    pub subject_prefix: Option<String>,
    pub deleted: Option<bool>,
}

/// Options for creating a new subject version
#[derive(Debug, Default)]
pub struct NewVersionOptions {
    pub normalize: Option<bool>,
}

/// Options for deleting a subject version
#[derive(Debug, Default)]
pub struct DeleteVersionOptions {
    pub permanent: Option<bool>,
}

impl SubjectClient<'_> {
    #[tracing::instrument(skip(self))]
    pub async fn list(
        &self,
        opts: ListSubjectsOptions,
    ) -> Result<Vec<SubjectName>, SchemaRegistryClientError> {
        let mut url = self.client.url.join("subjects")?;
        if let Some(subject_prefix) = opts.subject_prefix {
            url.query_pairs_mut()
                .append_pair("subjectPrefix", subject_prefix.as_str());
        }
        if let Some(deleted) = opts.deleted {
            url.query_pairs_mut()
                .append_pair("deleted", deleted.to_string().as_str());
        }
        self.client.get(url).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn versions(
        &self,
        name: &SubjectName,
    ) -> Result<Vec<SchemaVersion>, SchemaRegistryClientError> {
        let path = format!("subjects/{name}/versions");
        let url = self.client.url.join(&path)?;
        self.client.get(url).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn version(
        &self,
        name: &SubjectName,
        version: SchemaVersion,
    ) -> Result<Option<Subject>, SchemaRegistryClientError> {
        let path = format!("subjects/{name}/versions/{version}");
        let url = self.client.url.join(&path)?;
        self.client.get_optional(url).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn new_version(
        &self,
        name: &SubjectName,
        schema: &RegisterSchema,
        options: NewVersionOptions,
    ) -> Result<RegisteredSchema, SchemaRegistryClientError> {
        let path = format!("subjects/{name}/versions");
        let mut url = self.client.url.join(&path)?;
        if let Some(normalize) = options.normalize {
            let query = format!("normalize={normalize}");
            url.set_query(Some(&query));
        }
        self.client.post(url, schema).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn delete_version(
        &self,
        name: &SubjectName,
        version: SchemaVersion,
        options: DeleteVersionOptions,
    ) -> Result<Option<SchemaVersion>, SchemaRegistryClientError> {
        let path = format!("subjects/{name}/versions/{version}");
        let mut url = self.client.url.join(&path)?;
        if let Some(permanent) = options.permanent {
            let query = format!("permanent={permanent}");
            url.set_query(Some(&query));
        }

        self.client.delete_optional(url).await
    }
}
