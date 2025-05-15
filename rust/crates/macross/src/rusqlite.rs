use indoc::formatdoc;

/// Migrates database.
pub fn migrate(
    conn: &mut rusqlite::Connection,
    migrations: &[&'static str],
) -> Result<(), rusqlite::Error> {
    loop {
        let user_version: i64 = conn.query_row("PRAGMA user_version;", [], |x| x.get(0))?;
        if user_version == migrations.len() as i64 {
            break;
        }
        let migration = migrations[user_version as usize];
        conn.execute_batch(&formatdoc! {"
                {};
                PRAGMA user_version={};
            ",
            migration,
            user_version + 1
        })?;
    }
    Ok(())
}
