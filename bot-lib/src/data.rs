use crate::{
    config::{Config, ResponseKind},
    llm,
};
use bot_db::KingFisherDb;
use color_eyre::eyre::{Error, OptionExt, Result};
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::Message;
use rand::seq::SliceRandom;
use std::{path::Path, sync::Arc};
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    /// Config file watcher that refreshes the config if it changes
    ///
    /// Attached to the AppState to keep the watcher alive
    _watcher: notify::RecommendedWatcher,
    /// The path to the config file.
    /// This is to allow for saving / reloading the config.
    pub config_path: Box<Path>,
    pub llm_tx: crossbeam_channel::Sender<(String, tokio::sync::oneshot::Sender<String>)>,
    pub db: KingFisherDb,
}

impl AppState {
    pub fn new(config: Config, config_path: String) -> Result<AppState> {
        let config = Arc::new(RwLock::new(config));

        let llm_tx = llm::setup_llm()?;
        let db = KingFisherDb::new()?;

        use notify::{
            event::{AccessKind, AccessMode},
            Event, EventKind, RecursiveMode, Watcher,
        };

        let config_clone = Arc::clone(&config);
        let reload_config_path = config_path.clone();
        let config_path: Box<Path> = Path::new(&config_path).into();

        let mut watcher = notify::recommended_watcher(move |res| match res {
            Ok(Event {
                kind: EventKind::Access(AccessKind::Close(AccessMode::Write)),
                ..
            }) => {
                tracing::info!("config changed, reloading...");

                config_clone.blocking_write().reload(&*reload_config_path);
            }
            Err(e) => tracing::error!("watch error: {:?}", e),
            _ => {}
        })
        .expect("Failed to create file watcher");

        watcher
            .watch(&config_path, RecursiveMode::NonRecursive)
            .expect("Failed to watch config file");

        Ok(AppState {
            config,
            _watcher: watcher,
            config_path,
            llm_tx,
            db,
        })
    }

    /// If the message contents match any pattern, return the name of the response type.
    /// Otherwise, return None
    pub async fn find_response(
        &self,
        message: &str,
        message_link: &str,
    ) -> Option<Arc<ResponseKind>> {
        let config = self.config.read().await;

        config
            .responses
            .iter()
            .find_map(|response| response.find_valid_response(message, &config, message_link))
    }

    pub async fn respond(
        &self,
        message_response: &ResponseKind,
        reply_target: &Message,
        ctx: &serenity::Context,
    ) -> Result<()> {
        match message_response {
            ResponseKind::Text { content } => {
                reply_target.reply(ctx, content).await?;
            }
            ResponseKind::RandomText { content } => {
                let response = content
                    .choose(&mut rand::thread_rng())
                    .ok_or_eyre("The responses list is empty")?;

                reply_target.reply(ctx, response).await?;
            }
            ResponseKind::None => {}
        }

        Ok(())
    }
}

// User data, which is stored and accessible in all command invocations
pub type PoiseContext<'a> = poise::Context<'a, AppState, Error>;
