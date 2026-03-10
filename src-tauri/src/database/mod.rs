mod audio;
mod conversions;
mod cuts;
mod migrations;
mod models;
mod previews;
mod project_content;
mod projects;
mod schema;
mod sources;
mod timeline;

use rusqlite::{Connection, Result as SqlResult};
use std::path::PathBuf;

pub use audio::*;
pub use conversions::*;
pub use cuts::*;
pub use models::*;
pub use previews::*;
pub use project_content::*;
pub use projects::*;
pub use sources::*;
pub use timeline::*;

pub(crate) fn app_support_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("cascii_studio")
}

pub(crate) fn database_path() -> PathBuf {
    app_support_dir().join("projects.db")
}

pub(crate) fn init_database() -> SqlResult<Connection> {
    let db_path = database_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let conn = Connection::open(db_path)?;
    schema::init_schema(&conn)?;
    Ok(conn)
}
