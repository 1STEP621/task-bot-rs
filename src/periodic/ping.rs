use anyhow::{Context as _, Error};
use chrono::{DateTime, Duration, Local, NaiveTime, TimeDelta};
use itertools::Itertools;
use poise::serenity_prelude::*;

use crate::{data, PoiseContext, Task};

fn search_tasks(from: DateTime<Local>, to: DateTime<Local>) -> Result<Vec<Task>, Error> {
    let data = data::load()?;

    let tasks = data.tasks.lock().unwrap().clone();

    Ok(tasks
        .iter()
        .filter(|task| from < task.datetime && task.datetime <= to)
        .sorted_by_key(|task| task.datetime)
        .cloned()
        .collect())
}

fn embed(tasks: Vec<Task>) -> CreateEmbed {
    let fields = tasks.iter().map(|task| task.to_field()).collect::<Vec<_>>();

    if !fields.is_empty() {
        CreateEmbed::default()
            .title("タスク通知")
            .description("明日のタスクをお知らせします！")
            .fields(fields)
            .color(Color::RED)
    } else {
        CreateEmbed::default()
            .title("タスク通知")
            .description("明日のタスクはありません:tada:")
            .color(Color::DARK_GREEN)
    }
}

pub async fn ping(ctx: &Context) -> Result<(), Error> {
    let data = data::load()?;

    let ping_channel = (*data.ping_channel.lock().unwrap()).context("Ping channel not set")?;
    let ping_role = (*data.ping_role.lock().unwrap()).context("Ping role not set")?;

    let from = (Local::now() + Duration::days(1))
        .with_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
        .unwrap();
    let to = (Local::now() + Duration::days(2))
        .with_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
        .unwrap();

    println!("Searching tasks: from {} to {}", from, to);

    let tasks = search_tasks(from, to)?;
    ping_channel
        .send_message(
            ctx,
            CreateMessage::default()
                .content(if !tasks.is_empty() {
                    format!("{}", ping_role.mention())
                } else {
                    "".into()
                })
                .embed(embed(tasks)),
        )
        .await?;

    Ok(())
}

pub async fn update(ctx: &PoiseContext<'_>) -> Result<(), Error> {
    let data = data::load()?;

    let ping_channel = (*data.ping_channel.lock().unwrap()).context("Ping channel not set")?;
    let ping_role = (*data.ping_role.lock().unwrap()).context("Ping role not set")?;

    let prev_messages = ping_channel
        .messages(ctx, GetMessages::default())
        .await?
        .into_iter()
        .sorted_by_key(|m| m.id.created_at())
        .rev()
        .filter(|m| {
            m.author.id == ctx.framework().bot_id
                && Local::now().date_naive() - TimeDelta::days(1) <= m.id.created_at().date_naive()
                && m.referenced_message.is_none()
        });

    for prev_message in prev_messages {
        let from = (prev_message.id.created_at().with_timezone(&Local) + Duration::days(1))
            .with_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
            .unwrap();
        let to = (prev_message.id.created_at().with_timezone(&Local) + Duration::days(2))
            .with_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
            .unwrap();

        let prev_embed = prev_message.embeds[0].clone();

        if CreateEmbed::from(prev_embed) != embed(search_tasks(from, to)?) {
            ping_channel
                .send_message(
                    ctx,
                    CreateMessage::default()
                        .reference_message(&prev_message)
                        .content(format!(
                            "{}\n更新があります！ご注意ください！",
                            ping_role.mention()
                        ))
                        .embed(embed(search_tasks(from, to)?)),
                )
                .await?;
            println!("{}: Message updated", prev_message.id.created_at());
        } else {
            println!("{}: No changes; Updating not needed", prev_message.id.created_at());
        }
    }

    Ok(())
}
