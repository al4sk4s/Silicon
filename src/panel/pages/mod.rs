//! Uma página por arquivo; tudo re-exportado para o roteador e para os outros módulos.
mod alerts;
mod chats;
mod contacts;
mod dashboard;
mod debug;
mod downloader;
mod pair;
mod rules;
mod settings;
pub(crate) use alerts::*;
pub(crate) use chats::*;
pub(crate) use contacts::*;
pub(crate) use dashboard::*;
pub(crate) use debug::*;
pub(crate) use downloader::*;
pub(crate) use pair::*;
pub(crate) use rules::*;
pub(crate) use settings::*;
