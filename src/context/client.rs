use crate::context::prelude::*;
use crate::registry::{SchemaRegistryClient, SchemaRegistryClientError};

impl crate::registry::GetSchemaRegistryClient for Context {
    fn get_client(&self) -> Result<SchemaRegistryClient, SchemaRegistryClientError> {
        match &self.registry.auth {
            Authentication::None => SchemaRegistryClient::new(self.registry.url.as_str()),
            Authentication::Keychain(basic) => {
                let entry = keyring::Entry::new(
                    basic.basic_auth_entry_name.as_str(),
                    basic.username.as_str(),
                )
                .map_err(SchemaRegistryClientError::KeyringError)?;

                let password = entry
                    .get_password()
                    .map_err(SchemaRegistryClientError::KeyringError)?;

                SchemaRegistryClient::new_with_basic_auth(
                    self.registry.url.as_str(),
                    basic.username.as_str(),
                    password.as_str(),
                )
            }
            Authentication::BasicAuth { username, password } => {
                SchemaRegistryClient::new_with_basic_auth(
                    self.registry.url.as_str(),
                    username.as_str(),
                    password.as_str(),
                )
            }
        }
    }
}
