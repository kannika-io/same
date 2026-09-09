use clap::Args;
use keyring::Entry;
use same::context::{Authentication, ContextRepository, KeychainConfig, LocalContextRepository};
use std::env;
use std::str::FromStr;

#[derive(Args, Debug)]
pub struct AddCommand {}

enum AuthInput {
    Basic(BasicAuthInput),
    None,
}

pub struct BasicAuthInput {
    pub username: String,
    pub password: String,
}

impl AddCommand {
    pub async fn run(&self) -> anyhow::Result<()> {
        // https://github.com/console-rs/dialoguer/issues/251

        let url_input = dialoguer::Input::<String>::new()
            .with_prompt("Enter the url for the schema registry")
            .validate_with(|input: &String| -> Result<(), &str> {
                if input.starts_with("http://") || input.starts_with("https://") {
                    Ok(())
                } else {
                    Err("The url must start with http:// or https://")
                }
            });

        let url = interact(url_input)?;

        let auth_selection = dialoguer::Select::new()
            .with_prompt("Select the authentication method")
            .items(["Basic Auth", "None"])
            .default(0)
            .interact()?;

        let auth = match auth_selection {
            // Basic Auth
            0 => {
                let username_input =
                    dialoguer::Input::<String>::new().with_prompt("Enter the username");

                let username = interact(username_input)?;

                let password = dialoguer::Password::new()
                    .with_prompt("Enter the password")
                    .interact()?;

                AuthInput::Basic(BasicAuthInput { username, password })
            }
            // None
            1 => AuthInput::None,
            _ => unreachable!(),
        };

        // Ask user for name
        let name_input =
            dialoguer::Input::<String>::new().with_prompt("Enter a name for the context");

        let name = interact(name_input)?;

        // Store credentials in keyring
        let auth = match auth {
            AuthInput::Basic(basic_auth) => {
                let entry_name = store_in_keychain(&name, &basic_auth)?;

                Authentication::Keychain(KeychainConfig {
                    username: basic_auth.username,
                    basic_auth_entry_name: entry_name,
                })
            }
            AuthInput::None => Authentication::None,
        };

        // Store context
        let context = same::context::Context::new(
            same::context::ContextName(name.to_owned()),
            same::context::SchemaRegistryConfig { url, auth },
        );

        let repo = LocalContextRepository::get();
        repo.set_context(context)?;

        Ok(())
    }
}

///
/// Interacts with the user to get input
///
/// If the user is in a tmux session, any character is allowed.
/// If not, then only alphanumeric characters are allowed.
///
/// https://github.com/console-rs/dialoguer/issues/251
fn interact<T>(input: dialoguer::Input<T>) -> dialoguer::Result<T>
where
    T: Clone + ToString + FromStr,
    <T as FromStr>::Err: ToString,
{
    match env::var("TMUX") {
        Ok(value) if !value.is_empty() => input.interact(),
        _ => input.interact_text(),
    }
}

// TODO move to context module
fn store_in_keychain(name: &String, basic_auth: &BasicAuthInput) -> anyhow::Result<String> {
    let entry_name = format!("kannika-same-{}", &name);
    let entry = Entry::new(entry_name.as_str(), basic_auth.username.as_str())?;
    entry.set_password(&basic_auth.password)?;
    Ok(entry_name)
}
