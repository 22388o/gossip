use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct About {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: String,
    pub repository: String,
    pub homepage: String,
    pub license: String,
    pub database_path: String,
}

#[tauri::command]
pub fn about() -> About {
    let data_dir = match dirs::data_dir() {
        Some(mut d) => {
            d.push("gossip");
            d.push("gossip.sqlite");
            format!("{}", d.display())
        },
        None =>
            "Cannot find a directory to store application data.".to_owned()
    };

    About {
        name: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: env!("CARGO_PKG_DESCRIPTION").to_string(),
        authors: env!("CARGO_PKG_AUTHORS").to_string(),
        repository: env!("CARGO_PKG_REPOSITORY").to_string(),
        homepage: env!("CARGO_PKG_HOMEPAGE").to_string(),
        license: env!("CARGO_PKG_LICENSE").to_string(),
        database_path: data_dir.to_string(),
    }
}
