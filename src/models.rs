//! Datenmodelle, die vom Server verarbeitet werden

pub use gbx::*;
use serde_derive::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum MountPoint {
    Local,
    Remote,
}

pub fn get_local_path() -> String {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("local")
        .to_str()
        .unwrap_or_default()
        .to_string()
}

pub fn get_remote_path() -> String {
    std::env::var("REMOTE_MOUNT_POINT").unwrap_or("/mnt/data/files".to_string())
}

pub fn get_base_path(mount_point: MountPoint) -> String {
    match mount_point {
        MountPoint::Local => get_local_path(),
        MountPoint::Remote => get_remote_path(),
    }
}

pub fn get_db_path(mount_point: MountPoint) -> String {
    Path::new(&get_base_path(mount_point))
        .join("benutzer.sqlite.db")
        .to_str()
        .unwrap_or_default()
        .to_string()
}

pub fn get_data_dir(mount_point: MountPoint) -> String {
    Path::new(&get_base_path(mount_point))
        .join("data")
        .to_str()
        .unwrap_or_default()
        .to_string()
}

pub fn get_index_dir() -> String {
    Path::new(&get_base_path(MountPoint::Local))
        .join("index")
        .to_str()
        .unwrap_or_default()
        .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrUsize {
    S(String),
    U(usize),
}

impl std::fmt::Display for StringOrUsize {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            StringOrUsize::S(s) => write!(f, "{s}"),
            StringOrUsize::U(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenutzerInfo {
    pub id: i32,
    pub rechte: String,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AbonnementInfo {
    pub amtsgericht: String,
    pub grundbuchbezirk: String,
    pub blatt: AbonnementInfoBlattNr,
    pub text: String,
    pub aktenzeichen: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AbonnementInfoBlattNr {
    Alle,
    Exakt(i32),
}

impl AbonnementInfoBlattNr {
    pub fn matches(&self, s: &str) -> bool {
        match self {
            AbonnementInfoBlattNr::Alle => true,
            AbonnementInfoBlattNr::Exakt(i) => i.to_string() == s.trim(),
        }
    }
}