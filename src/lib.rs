use heroku_rs::framework::{auth::Credentials, ApiEnvironment, HttpApiClient, HttpApiClientConfig};

use serenity::client::Client;
use serenity::framework::standard::DispatchError::{NotEnoughArguments, TooManyArguments};
use serenity::framework::standard::{HelpOptions, help_commands, Args, CommandGroup, CommandResult, macros::{help,group}, StandardFramework};
use serenity::model::gateway::Ready;
use serenity::prelude::{Context, EventHandler, TypeMapKey};
use serenity::model::prelude::{Message, UserId};
use std::sync::Arc;
use std::collections::HashSet;

mod commands;

use commands::{heroku::*, math::*, myid::*, ping::*};

mod authorizations;

pub mod config;

pub mod utilities;

use crate::config::Config;

use crate::authorizations::users::*;

#[group]
#[commands(
    ping,
    multiply,
    myid,
    get_app,
    get_apps,
    restart_app,
    scale_app,
    update_app_config,
    get_app_releases,
    rollback_app,
    block_ip,
    unblock_ip,
    deploy_app
)]
struct General;

struct Handler;

impl EventHandler for Handler {
    fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

struct HerokuClientKey;

impl TypeMapKey for HerokuClientKey {
    type Value = Arc<heroku_rs::framework::HttpApiClient>;
}

#[help]
#[individual_command_tip =
"Hello! こんにちは！Hola! Bonjour! Ciao! 您好!\n\
If you want more information about a specific command, just pass the command as argument."]
#[command_not_found_text = "Could not find: `{}`."]
fn my_help(
   context: &mut Context,
   msg: &Message,
   args: Args,
   help_options: &'static HelpOptions,
   groups: &[&'static CommandGroup],
   owners: HashSet<UserId>
) -> CommandResult {
   help_commands::with_embeds(context, msg, args, help_options, groups, owners)
}


// These commands do not require a user
// to be in the AUTHORIZED_USERS env variable
const NO_AUTH_COMMANDS: &[&str] = &["ping", "multiply", "myid"];

pub fn run(config: Config) {
    let mut client = Client::new(&config.discord_token, Handler).expect("Err creating client");

    let heroku_client_instance = initial_heroku_client(&config.heroku_api_key);

    {
        let mut data = client.data.write();
        data.insert::<HerokuClientKey>(Arc::new(heroku_client_instance));
        data.insert::<Config>(Arc::new(config.clone()));
    }

    client.with_framework(
        StandardFramework::new()
            .before(move |ctx, msg, cmd_name| {
                if !is_authorized(&msg.author.id.to_string(), &config) {
                    if NO_AUTH_COMMANDS.contains(&cmd_name) {
                        return true;
                    }

                    println!("User is not authorized to run this command");
                    msg.reply(
                        ctx,
                        format!("User {} is not authorized to run this command", &msg.author),
                    )
                    .ok();

                    return false;
                }
                println!("Running command {}", cmd_name);
                true
            })
            .on_dispatch_error(|context, msg, error| match error {
                NotEnoughArguments { min, given } => {
                    let s = format!("Need {} arguments, but only got {}.", min, given);

                    let _ = msg.channel_id.say(&context.http, &s);
                }
                TooManyArguments { max, given } => {
                    let s = format!("Max arguments allowed is {}, but got {}.", max, given);

                    let _ = msg.channel_id.say(&context.http, &s);
                }
                _ => {
                    println!("Unhandled dispatch error {:?}", error);
                }
            })
            .after(|ctx, msg, cmd_name, error| {
                if let Err(err) = error {
                    msg.reply(&ctx, format!("There was an error when running {}: {:?}", cmd_name, err)).ok();
                }
            })
            .configure(|c| c.prefix("~")) // set the bot's prefix to "~"
            .unrecognised_command(|ctx, msg, unknown_command_name| {
                msg.reply(&ctx, format!("Could not find a command named `{}`", unknown_command_name)).ok();
            })
            .help(&MY_HELP)
            .group(&GENERAL_GROUP),
    );

    if let Err(why) = client.start() {
        println!("Client error: {:?}", why);
    }
}

fn heroku_credentials(api_key: &str) -> heroku_rs::framework::auth::Credentials {
    Credentials::UserAuthToken {
        token: api_key.to_string(),
    }
}

fn initial_heroku_client(api_key: &str) -> heroku_rs::framework::HttpApiClient {
    HttpApiClient::new(
        heroku_credentials(api_key),
        HttpApiClientConfig::default(),
        ApiEnvironment::Production,
    )
    .unwrap()
}
