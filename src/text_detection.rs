use crate::types::{Data, Error, MessageAttachment::*};

use std::sync::{Mutex, MutexGuard};

use chrono::{DateTime, Duration, Utc};
use poise::serenity_prelude as serenity;
use poise::Event;
use rand::prelude::*;
use serenity::Message;

pub fn register_detectors(data: &mut Data) {
    data.register(
        "rust",
        r"rust",
        |_message, _ctx| {
            let i = random::<u8>() % 5;
            Text(match i {
                1 => "RUST MENTIONED :crab: :crab: :crab:",
                2 => "<@216767618923757568>",
                3 => "Rust is simply the best programming language. Nothing else can compare. I am naming my kids Rust and Ferris.",
                4 => concat!("Launch the Polaris,\n", "the end doesn't scare us\n", "When will this cease?\n", "The warheads will all rust in peace!"),
                _ => "Rust? Oh, you mean the game?"
            })
        }
    );
    data.register("tkinter", r"tkinter", |_message, _ctx| {
        TextPlusImage("TKINTER MENTIONED", "./assets/tkinter.png")
    });
    data.register("arch", r"arch", |_message, _ctx| Text("I use Arch btw"));
    data.register("goop", r"goop", |_message, _ctx| {
        let i = random::<bool>();
        Text(if i {
            "https://tenor.com/view/gunge-gunged-slime-slimed-dunk-gif-21115557"
        } else {
            "https://tenor.com/view/goop-goop-house-jello-gif-23114313"
        })
    });
    data.register("1984", r"1984", |_message, _ctx| {
        Text("https://tenor.com/view/1984-gif-19260546")
    });
}

pub async fn text_detection(
    ctx: &serenity::Context,
    _event: &Event<'_>,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
    message: &Message,
) -> Result<(), Error> {
    match data.check_should_respond(message) {
        Some(name) => {
            if cooldown_checker(
                data.last_response(&name),
                data.config.lock_cooldown(),
                message.timestamp.with_timezone(&Utc),
            ) {
                data.run_action(&name, message, ctx).await?;
            }
        }
        None => {}
    }

    Ok(())
}

/// Checks if the cooldown is met. If yes, it is, returns true and resets the cooldown. If not,
/// returns false and does nothing.
fn cooldown_checker(
    last_message: &Mutex<DateTime<Utc>>,
    cooldown: MutexGuard<Duration>,
    timestamp: DateTime<Utc>,
) -> bool {
    let mut last_message = last_message.lock().expect("Could not lock mutex");
    if *last_message + *cooldown > timestamp {
        return false;
    }

    *last_message = timestamp;

    true
}
