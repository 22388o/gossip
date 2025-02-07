use crate::{Error, GLOBALS};
use nostr_proto::Url;
use serde::{Deserialize, Serialize};
use tauri::async_runtime::spawn_blocking;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbRelay {
    pub url: String,
    pub success_count: u64,
    pub failure_count: u64,
    pub rank: Option<u64>,
}

impl DbRelay {
    pub fn new(url: String) -> DbRelay {
        DbRelay {
            url: url,
            success_count: 0,
            failure_count: 0,
            rank: Some(3),
        }
    }

    #[allow(dead_code)]
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbRelay>, Error> {
        let sql = "SELECT url, success_count, failure_count, rank FROM relay".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbRelay>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok(DbRelay {
                    url: row.get(0)?,
                    success_count: row.get(1)?,
                    failure_count: row.get(2)?,
                    rank: row.get(3)?,
                })
            })?;

            let mut output: Vec<DbRelay> = Vec::new();
            for row in rows {
                output.push(row?);
            }
            Ok(output)
        })
        .await?;

        output
    }

    #[allow(dead_code)]
    pub async fn fetch_one(url: &Url) -> Result<Option<DbRelay>, Error> {
        let relays = DbRelay::fetch(
            Some(&format!("url='{}'",url))
        ).await?;

        if relays.len() == 0 {
            Ok(None)
        } else {
            Ok(Some(relays[0].clone()))
        }
    }

    #[allow(dead_code)]
    pub async fn insert(relay: DbRelay) -> Result<(), Error> {
        let sql =
            "INSERT OR IGNORE INTO relay (url, success_count, failure_count, rank) \
             VALUES (?1, ?2, ?3, ?4)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            stmt.execute((
                &relay.url,
                &relay.success_count,
                &relay.failure_count,
                &relay.rank
            ))?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn update(relay: DbRelay) -> Result<(), Error> {
        let sql =
            "UPDATE relay SET success_count=?, failure_count=?, rank=? WHERE url=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            stmt.execute((
                &relay.success_count,
                &relay.failure_count,
                &relay.rank,
                &relay.url,
            ))?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete(criteria: &str) -> Result<(), Error> {
        let sql = format!("DELETE FROM relay WHERE {}", criteria);

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            db.execute(&sql, [])?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn populate_new_relays() -> Result<(), Error> {
        let sql = "INSERT OR IGNORE INTO relay (url, rank) SELECT DISTINCT relay, 3 FROM person_relay";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            db.execute(&sql, [])?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }
}
