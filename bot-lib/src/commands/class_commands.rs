use crate::{commands::get_member, courses::get_course, data::PoiseContext};
use color_eyre::eyre::{OptionExt, Result, WrapErr};
use itertools::Itertools;
use poise::serenity_prelude::{
    ChannelType, CreateChannel, EditRole, PermissionOverwrite, PermissionOverwriteType,
    Permissions, Role, RoleId,
};
use regex::Regex;
use std::{collections::HashMap, fmt::Write, sync::LazyLock};

static CLASS_ROLE_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\w+ \d+$").unwrap());

fn get_class_roles(roles: HashMap<RoleId, Role>) -> impl Iterator<Item = Role> {
    roles
        .into_values()
        .filter(|role| CLASS_ROLE_REGEX.is_match(&role.name))
        .sorted()
}

/// List all classes you can join
#[poise::command(slash_command, ephemeral = true)]
pub async fn list_classes(ctx: PoiseContext<'_>) -> Result<()> {
    let guild_id = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let roles = guild_id.roles(ctx).await?;

    let mut message_text =
        String::from("### Classes:\nJoin any of them with `/join_class <role name>`\n\n");

    for role in get_class_roles(roles) {
        message_text.push_str(&format!("`{}` ", role.name));
    }

    ctx.say(message_text).await?;

    Ok(())
}

/// List the classes you're in via roles
#[poise::command(slash_command, ephemeral = true)]
pub async fn my_classes(ctx: PoiseContext<'_>) -> Result<()> {
    let user = ctx.author();
    let guild_id = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let roles = guild_id.roles(ctx).await?;

    let Some(user_roles) = guild_id.member(ctx, user.id).await?.roles(ctx) else {
        ctx.say("You don't have any roles?").await?;

        return Ok(());
    };

    let user_roles_formatted = get_class_roles(roles)
        .filter(|role| user_roles.contains(role))
        .map(|role| role.name)
        .collect_vec();

    let message_text = if user_roles_formatted.is_empty() {
        String::from("You don't have any class roles.")
    } else {
        let mut text = String::from("Your classes:\n");
        for role_str in user_roles_formatted {
            writeln!(&mut text, "- `{role_str}`")?;
        }

        text
    };

    ctx.say(message_text).await?;

    Ok(())
}

enum GetRoleResult {
    Found(RoleId),
    MultipleFound(Vec<Role>),
    NotFound,
}

async fn get_role(ctx: PoiseContext<'_>, identifier: &str) -> Result<GetRoleResult> {
    let guild_id = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let roles = guild_id.roles(ctx).await?;

    let identifier = identifier
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect::<String>();

    let college_id = identifier
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect::<String>();

    let college_id = if college_id.is_empty() {
        String::from("CS")
    } else {
        college_id
    };

    let course_id = identifier
        .chars()
        .skip_while(|c| c.is_ascii_alphabetic())
        .collect::<String>();

    let identifier = format!("{} {}", college_id, course_id);

    let joinable_roles = get_class_roles(roles)
        .filter(|Role { name, .. }| name.contains(&identifier))
        .collect_vec();

    if joinable_roles.is_empty() {
        return Ok(GetRoleResult::NotFound);
    }

    if joinable_roles.len() > 1 {
        return Ok(GetRoleResult::MultipleFound(joinable_roles));
    }

    Ok(GetRoleResult::Found(joinable_roles.first().unwrap().id))
}

const MOD_ROLE_ID: RoleId = RoleId::new(1192863993883279532);

#[poise::command(
    slash_command,
    required_permissions = "MANAGE_CHANNELS",
    description_localized("en-US", "Creates a class category")
)]
pub async fn create_class_category(
    ctx: PoiseContext<'_>,
    #[description = "The class number, eg. for CS2420 put in \"2420\""] number: String,
) -> Result<()> {
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let channels = guild.channels(ctx).await?;

    let number_string = number.to_string();
    for (_, channel) in channels {
        if channel.name.contains(&number_string) {
            ctx.say("Category/channels already seem to exist!").await?;
            return Ok(());
        }
    }

    let role_name = format!("CS {}", number_string);

    let (category_name, channel_description) = get_course(&format!("CS{}", number))
        .map(|course| {
            let mut category_name = format!("{role_name} - {}", course.long_name);
            category_name.truncate(100);
            let mut channel_description = course.description;
            channel_description.truncate(1024);

            (Some(category_name), Some(channel_description))
        })
        .unwrap_or((None, None));

    let role = guild
        .create_role(ctx, EditRole::new().name(&role_name))
        .await
        .wrap_err("Couldn't create role")?;

    let category = guild
        .create_channel(
            ctx,
            CreateChannel::new(category_name.unwrap_or(role_name))
                .kind(ChannelType::Category)
                .permissions(vec![
                    PermissionOverwrite {
                        allow: Permissions::VIEW_CHANNEL,
                        deny: Permissions::empty(),
                        kind: PermissionOverwriteType::Role(role.id),
                    },
                    PermissionOverwrite {
                        allow: Permissions::VIEW_CHANNEL,
                        deny: Permissions::empty(),
                        kind: PermissionOverwriteType::Role(MOD_ROLE_ID),
                    },
                    PermissionOverwrite {
                        allow: Permissions::empty(),
                        deny: Permissions::VIEW_CHANNEL,
                        kind: PermissionOverwriteType::Role(guild.everyone_role()),
                    },
                ]),
        )
        .await
        .wrap_err("Couldn't create category")?;

    guild
        .create_channel(
            ctx,
            CreateChannel::new(format!("{}-resources", number_string))
                .kind(ChannelType::Text)
                .category(category.id),
        )
        .await
        .wrap_err("Couldn't create resources channel")?;

    guild
        .create_channel(
            ctx,
            CreateChannel::new(format!("{}-general", number_string))
                .kind(ChannelType::Text)
                .category(category.id)
                .topic(channel_description.unwrap_or_default()),
        )
        .await
        .wrap_err("Couldn't create general channel")?;

    ctx.say("Success!").await?;
    Ok(())
}

#[poise::command(
    slash_command,
    required_permissions = "MANAGE_CHANNELS",
    description_localized("en-US", "Deletes a class category")
)]
pub async fn delete_class_category(
    _ctx: PoiseContext<'_>,
    #[description = "The class identifier, eg. for CS2420 put in \"CS2420\" or \"2420\""]
    _identifier: String,
) -> Result<()> {
    // let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    // let channels = guild.channels(ctx).await?;

    // let category_regex = format!("^CS {}", number);
    // let pattern = Regex::new(&category_regex)?;

    // let gotten_channels = channels
    //     .values()
    //     .find(|channel| pattern.is_match(&channel.name));

    // let Some(category_channel) = gotten_channels else {
    //     ctx.say("Could not find category channel!").await?;
    //     return Ok(());
    // };

    // let children_guild_channels = channels
    //     .values()
    //     .filter(|guild_channel| matches!(guild_channel.parent_id, Some(parent) if parent.eq(&category_channel.id)));

    // let role_id = get_role(ctx, &number).await?;

    // category_channel.delete(ctx).await?;
    // for guild_channel in children_guild_channels {
    //     guild_channel.delete(ctx).await?;
    // }
    // guild.delete_role(ctx, role_id).await?;

    // ctx.say("Success!").await?;
    Ok(())
}

// pub async fn reset_class_category_backend(ctx: PoiseContext<'_>, number: u32) -> Result<()> {
//     let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
//     let members = guild.members(ctx, None, None).await?;

//     let general_channel_name = format!("{}-general", number);
//     let gotten_channels = get_channels(ctx, guild, Regex::new(&general_channel_name)?).await?;
//     let general_channel = gotten_channels
//         .first()
//         .ok_or_eyre("Could not find general channel!")?;

//     let role_id = get_role(ctx, number).await?;

//     let category_id = general_channel
//         .parent_id
//         .ok_or_eyre("Couldn't get category ID!")?;

//     general_channel.delete(ctx).await?;

//     guild
//         .create_channel(
//             ctx,
//             CreateChannel::new(general_channel_name)
//                 .kind(ChannelType::Text)
//                 .category(category_id),
//         )
//         .await
//         .wrap_err("Couldn't create general channel")?;

//     let members_with_role = members
//         .iter()
//         .filter(|member| member.roles.contains(&role_id));

//     for member in members_with_role {
//         member.remove_role(ctx, role_id).await?;
//     }

//     Ok(())
// }

#[poise::command(
    slash_command,
    required_permissions = "MANAGE_CHANNELS",
    description_localized(
        "en-US",
        "Resets a class category (clears the general channel, removes the role from everyone)"
    )
)]
pub async fn reset_class_category(
    ctx: PoiseContext<'_>,
    #[description = "The class number, eg. for CS2420 put in \"2420\""] _number: u32,
) -> Result<()> {
    // reset_class_category_backend(ctx, number).await?;
    // ctx.say("Success!").await?;
    ctx.say("This command is currently not implemented").await?;
    Ok(())
}

#[poise::command(
    slash_command,
    required_permissions = "MANAGE_CHANNELS",
    description_localized("en-US", "Resets all class categories")
)]
pub async fn reset_class_categories(ctx: PoiseContext<'_>) -> Result<()> {
    // let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    // let removed_categories = get_channels(ctx, guild, Regex::new(r"\d{4}-general").unwrap())
    //     .await?
    //     .into_iter()
    //     .map(|channel| {
    //         channel
    //             .name
    //             .get(0..4)
    //             .unwrap_or("Intentional parse error")
    //             .parse::<u32>()
    //             .context("Parse error")
    //     });

    // for category in removed_categories {
    //     reset_class_category_backend(ctx, category?).await?;
    // }

    // ctx.say("Success!").await?;
    ctx.say("This command is currently not implemented").await?;

    Ok(())
}

/// Join a class. Enter the CS identifier, eg. for CS2420 put in "CS2420" or "2420"
#[poise::command(slash_command, rename = "join_class", ephemeral = true)]
pub async fn add_class_role(
    ctx: PoiseContext<'_>,
    #[description = "The class identifier, eg. for CS2420 put in \"CS2420\" or \"2420\""]
    identifier: String,
) -> Result<()> {
    let author = get_member(ctx).await?;

    match get_role(ctx, &identifier).await? {
        GetRoleResult::Found(role_id) => {
            author
                .add_role(ctx, role_id)
                .await
                .wrap_err("Couldn't add role")?;

            ctx.say("Joined class!").await?;
        }
        GetRoleResult::MultipleFound(roles) => {
            let mut message_text =
                format!("Multiple classes found with search \"{}\"\n", identifier);
            for role in roles {
                message_text.push_str(&format!("`{}` ", role.name));
            }
            ctx.say(message_text).await?;
        }
        GetRoleResult::NotFound => {
            ctx.say(format!(
                "Could not find class with identifier \"{}\"",
                identifier
            ))
            .await?;
        }
    }

    Ok(())
}

/// Leave a class. Enter the CS identifier, eg. for CS2420 put in "CS2420" or "2420"
#[poise::command(slash_command, rename = "leave_class", ephemeral = true)]
pub async fn remove_class_role(
    ctx: PoiseContext<'_>,
    #[description = "The class identifier, eg. for CS2420 put in \"CS2420\" or \"2420\""]
    identifier: String,
) -> Result<()> {
    let author = get_member(ctx).await?;

    match get_role(ctx, &identifier).await? {
        GetRoleResult::Found(role_id) => {
            author
                .remove_role(ctx, role_id)
                .await
                .wrap_err("Couldn't remove role")?;

            ctx.say("Left class!").await?;
        }
        GetRoleResult::MultipleFound(roles) => {
            let mut message_text =
                format!("Multiple classes found with search \"{}\"\n", identifier);
            for role in roles {
                message_text.push_str(&format!("`{}` ", role.name));
            }
            ctx.say(message_text).await?;
        }
        GetRoleResult::NotFound => {
            ctx.say(format!(
                "Could not find class with identifier \"{}\"",
                identifier
            ))
            .await?;
        }
    }

    Ok(())
}
