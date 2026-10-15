//! Shared message feed: the boss types in the panel, Claude posts through `gb.say`.
//! Every entry is also appended to collab/devfeed.log so it survives the session.

use bevy::prelude::*;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct FeedEntry {
    pub who: String,
    pub text: String,
    pub tick: u64,
    pub stamp: String,
}

#[derive(Resource, Default)]
pub struct DevFeed {
    pub entries: Vec<FeedEntry>,
}

impl DevFeed {
    pub fn post(&mut self, who: &str, text: &str, tick: u64) {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) % 86_400;
        let stamp = format!("{:02}:{:02}:{:02} UTC", secs / 3600, secs / 60 % 60, secs % 60);
        let path = format!("{}/collab/devfeed.log", crate::ROOT);
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "[{stamp}] {who} @tick {tick}: {text}");
        }
        self.entries.push(FeedEntry { who: who.into(), text: text.into(), tick, stamp });
    }
}
