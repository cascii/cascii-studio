use super::{
    cut_display_name, delete_project_content_rows_for_item, init_database,
    rename_project_content_rows_for_item, ProjectContentType, VideoCut,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Result as SqlResult};

pub fn add_video_cut(cut: &VideoCut) -> SqlResult<()> {
    let conn = init_database()?;
    conn.execute(
        "INSERT INTO cuts (id, project_id, source_file_id, file_path, date_added, size, custom_name, start_time, end_time, duration)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            cut.id,
            cut.project_id,
            cut.source_file_id,
            cut.file_path,
            cut.date_added.to_rfc3339(),
            cut.size,
            cut.custom_name,
            cut.start_time,
            cut.end_time,
            cut.duration,
        ],
    )?;
    Ok(())
}

pub fn get_project_cuts(project_id: &str) -> SqlResult<Vec<VideoCut>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, project_id, source_file_id, file_path, date_added, size, custom_name, start_time, end_time, duration
         FROM cuts
         WHERE project_id = ?1
         ORDER BY date_added DESC",
    )?;

    let cuts = stmt
        .query_map([project_id], |row| {
            let date_str: String = row.get(4)?;
            Ok(VideoCut {
                id: row.get(0)?,
                project_id: row.get(1)?,
                source_file_id: row.get(2)?,
                file_path: row.get(3)?,
                date_added: DateTime::parse_from_rfc3339(&date_str)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                size: row.get(5)?,
                custom_name: row.get(6)?,
                start_time: row.get(7)?,
                end_time: row.get(8)?,
                duration: row.get(9)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;
    Ok(cuts)
}

pub fn get_cut(cut_id: &str) -> SqlResult<Option<VideoCut>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, project_id, source_file_id, file_path, date_added, size, custom_name, start_time, end_time, duration
         FROM cuts
         WHERE id = ?1
         LIMIT 1",
    )?;
    let mut rows = stmt.query([cut_id])?;
    if let Some(row) = rows.next()? {
        let date_str: String = row.get(4)?;
        Ok(Some(VideoCut {
            id: row.get(0)?,
            project_id: row.get(1)?,
            source_file_id: row.get(2)?,
            file_path: row.get(3)?,
            date_added: DateTime::parse_from_rfc3339(&date_str)
                .unwrap_or_else(|_| Utc::now().into())
                .with_timezone(&Utc),
            size: row.get(5)?,
            custom_name: row.get(6)?,
            start_time: row.get(7)?,
            end_time: row.get(8)?,
            duration: row.get(9)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn delete_cut(cut_id: &str) -> SqlResult<()> {
    println!("🗑️ DB: Deleting cut: {}", cut_id);
    let conn = init_database()?;
    delete_project_content_rows_for_item(&conn, cut_id, &[ProjectContentType::Cut])?;
    let result = conn.execute("DELETE FROM cuts WHERE id = ?1", [cut_id]);

    match &result {
        Ok(rows_affected) => println!("🗑️ DB: Delete successful, {} rows affected", rows_affected),
        Err(e) => println!("🗑️ DB: Delete failed: {}", e),
    }

    result?;
    Ok(())
}

pub fn update_cut_custom_name(cut_id: &str, custom_name: Option<String>) -> SqlResult<()> {
    println!(
        "📝 DB: Updating cut custom_name for {} to {:?}",
        cut_id, custom_name
    );
    let conn = init_database()?;
    let result = conn.execute(
        "UPDATE cuts SET custom_name = ?1 WHERE id = ?2",
        params![custom_name, cut_id],
    );

    match &result {
        Ok(rows_affected) => println!("📝 DB: Update successful, {} rows affected", rows_affected),
        Err(e) => println!("📝 DB: Update failed: {}", e),
    }

    result?;
    let mut stmt = conn.prepare(
        "SELECT custom_name, start_time, end_time
         FROM cuts
         WHERE id = ?1
         LIMIT 1",
    )?;
    let mut rows = stmt.query([cut_id])?;
    if let Some(row) = rows.next()? {
        let custom_name = row.get::<_, Option<String>>(0)?;
        let start_time = row.get::<_, f64>(1)?;
        let end_time = row.get::<_, f64>(2)?;
        rename_project_content_rows_for_item(
            &conn,
            cut_id,
            &[ProjectContentType::Cut],
            &cut_display_name(custom_name, start_time, end_time),
        )?;
    }

    Ok(())
}
