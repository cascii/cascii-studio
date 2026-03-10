use super::{init_database, AudioExtraction};
use chrono::{DateTime, Utc};
use rusqlite::{params, Result as SqlResult};

pub fn add_audio_extraction(audio: &AudioExtraction) -> SqlResult<()> {
    let conn = init_database()?;
    conn.execute(
        "INSERT INTO audio (id, folder_name, folder_path, source_file_id, project_id, creation_date, total_size, audio_track_beginning, audio_track_end, custom_name)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            audio.id,
            audio.folder_name,
            audio.folder_path,
            audio.source_file_id,
            audio.project_id,
            audio.creation_date.to_rfc3339(),
            audio.total_size,
            audio.audio_track_beginning,
            audio.audio_track_end,
            audio.custom_name,
        ],
    )?;
    Ok(())
}

pub fn get_project_audio(project_id: &str) -> SqlResult<Vec<AudioExtraction>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, folder_name, folder_path, source_file_id, project_id, creation_date, total_size, audio_track_beginning, audio_track_end, custom_name
         FROM audio
         WHERE project_id = ?1
         ORDER BY creation_date DESC",
    )?;

    let audio_list = stmt
        .query_map([project_id], |row| {
            let date_str: String = row.get(5)?;
            Ok(AudioExtraction {
                id: row.get(0)?,
                folder_name: row.get(1)?,
                folder_path: row.get(2)?,
                source_file_id: row.get(3)?,
                project_id: row.get(4)?,
                creation_date: DateTime::parse_from_rfc3339(&date_str)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                total_size: row.get(6)?,
                audio_track_beginning: row.get(7)?,
                audio_track_end: row.get(8)?,
                custom_name: row.get(9)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;
    Ok(audio_list)
}

pub fn delete_audio(audio_id: &str) -> SqlResult<()> {
    println!("🗑️ DB: Deleting audio: {}", audio_id);
    let conn = init_database()?;
    let result = conn.execute("DELETE FROM audio WHERE id = ?1", [audio_id]);

    match &result {
        Ok(rows_affected) => println!("🗑️ DB: Delete successful, {} rows affected", rows_affected),
        Err(e) => println!("🗑️ DB: Delete failed: {}", e),
    }

    result?;
    Ok(())
}

pub fn delete_audio_by_folder_path(folder_path: &str) -> SqlResult<()> {
    println!("🗑️ DB: Deleting audio by folder path: {}", folder_path);
    let conn = init_database()?;
    let result = conn.execute("DELETE FROM audio WHERE folder_path = ?1", [folder_path]);

    match &result {
        Ok(rows_affected) => println!("🗑️ DB: Delete successful, {} rows affected", rows_affected),
        Err(e) => println!("🗑️ DB: Delete failed: {}", e),
    }

    result?;
    Ok(())
}

pub fn update_audio_custom_name(audio_id: &str, custom_name: Option<String>) -> SqlResult<()> {
    println!(
        "📝 DB: Updating audio custom_name for {} to {:?}",
        audio_id, custom_name
    );
    let conn = init_database()?;
    let result = conn.execute(
        "UPDATE audio SET custom_name = ?1 WHERE id = ?2",
        params![custom_name, audio_id],
    );

    match &result {
        Ok(rows_affected) => println!("📝 DB: Update successful, {} rows affected", rows_affected),
        Err(e) => println!("📝 DB: Update failed: {}", e),
    }

    result?;
    Ok(())
}
