//! Порт: Telegram/SourceFiles/{storage,main,data,api,core,lang,countries} и корневые apiwrap/config/settings/logs/stdafx, history/history*, history_item* оригинала AyuGramDesktop.
// Unsafe is restricted to reviewed minizip/local-time FFI modules.
#![deny(unsafe_code)]
pub mod data;
#[path = "main/mod.rs"]
pub mod main;
pub mod storage;

pub mod ayu;
pub mod core;

pub mod statistics;

pub mod platform;
