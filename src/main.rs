mod vk_api;

use std::{collections::HashSet, fs::File};
use std::io::prelude::*;
use serde::Deserialize;
use vk_api::SessionInfo;
use anyhow::{Context, Result};

trait BoolExt {
    fn not(self) -> bool;
}

impl BoolExt for bool {
    #[inline]
    fn not(self) -> bool { !self }
}

#[derive(Deserialize)]
struct Config {
    access_token: String,
    id_list: Vec<u32>,
}

fn pause() {
    use std::io;
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();
    write!(stdout, "Press any key to continue...").unwrap();
    stdout.flush().unwrap();
    let _ = stdin.read(&mut [0u8]).unwrap();
}

async fn main_hook() -> Result<()> {
    let mut file = File::open("config.toml").context("Could not open file.")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).context("Could not read contents of config.toml.")?;
    let config: Config = toml::from_str(contents.as_str()).context("Failed to parse config data.")?;
    let access_token = config.access_token;
    let id_list: HashSet<String> = config.id_list.into_iter().map(|u| u.to_string()).collect();
    let s_info = SessionInfo::new(access_token, "5.124");
    let mut long_poll_server_iter = s_info.get_long_poll_server(false, 0, 2).await?.into_async_iter(&s_info);
    while let Some(updates) = long_poll_server_iter.next().await {
        let messages = updates.into_iter()
        .filter(|v| v.len() > 6 && v[0] == 4 && v[3].as_u64().iter().any(|&x| x < 2_000_000_000).not())
        .filter_map(|update| {
            match update[6].as_object()
            .and_then(|obj| obj.get("from"))
            .and_then(|obj| obj.as_str()) {
                Some(user_id) if id_list.contains(user_id) => update[1].as_u64().map(|x| x.to_string()),
                _ => None,
            }
        })
        .collect::<Vec<_>>()
        .join(",");
        if messages.is_empty().not() {
            match s_info.delete_messages(&messages, false, 0, false).await
            {
                Ok(_) => println!("{}", messages),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    main_hook().await.map_err(|e| {
        eprintln!("Error: {}", e);
        if let Some(src) = e.source() {
            eprintln!("Caused by: {}", src);
        }
        pause();
    })
    .unwrap_or_default()
}
