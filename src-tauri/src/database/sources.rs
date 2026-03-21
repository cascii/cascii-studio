use super::{
    delete_project_content_rows_for_item, init_database, rename_project_content_rows_for_item,
    source_display_name, ProjectContentType, SourceContent, SourceType,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Result as SqlResult};

pub fn add_source_content(source: &SourceContent) -> SqlResult<()> {
    let conn = init_database()?;

    conn.execute(
        "INSERT INTO source_content (id, content_type, project_id, date_added, size, file_path, custom_name)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            source.id,
            source.content_type.to_string(),
            source.project_id,
            source.date_added.to_rfc3339(),
            source.size,
            source.file_path,
            source.custom_name,
        ],
    )?;

    Ok(())
}

pub fn get_project_sources(project_id: &str) -> SqlResult<Vec<SourceContent>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, content_type, project_id, date_added, size, file_path, custom_name
         FROM source_content
         WHERE project_id = ?1
         ORDER BY date_added ASC",
    )?;

    let sources = stmt
        .query_map([project_id], |row| {
            let date_str: String = row.get(3)?;

            Ok(SourceContent {
                id: row.get(0)?,
                content_type: SourceType::from_string(&row.get::<_, String>(1)?),
                project_id: row.get(2)?,
                date_added: DateTime::parse_from_rfc3339(&date_str)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                size: row.get(4)?,
                file_path: row.get(5)?,
                custom_name: row.get(6)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    Ok(sources)
}

pub fn get_source(source_id: &str) -> SqlResult<Option<SourceContent>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, content_type, project_id, date_added, size, file_path, custom_name
         FROM source_content
         WHERE id = ?1
         LIMIT 1",
    )?;
    let mut rows = stmt.query([source_id])?;
    if let Some(row) = rows.next()? {
        let date_str: String = row.get(3)?;
        Ok(Some(SourceContent {
            id: row.get(0)?,
            content_type: SourceType::from_string(&row.get::<_, String>(1)?),
            project_id: row.get(2)?,
            date_added: DateTime::parse_from_rfc3339(&date_str)
                .unwrap_or_else(|_| Utc::now().into())
                .with_timezone(&Utc),
            size: row.get(4)?,
            file_path: row.get(5)?,
            custom_name: row.get(6)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn update_source_custom_name(source_id: &str, custom_name: Option<String>) -> SqlResult<()> {
    let conn = init_database()?;

    conn.execute(
        "UPDATE source_content SET custom_name = ?1 WHERE id = ?2",
        params![custom_name, source_id],
    )?;

    let mut stmt = conn.prepare(
        "SELECT content_type, file_path, custom_name
         FROM source_content
         WHERE id = ?1
         LIMIT 1",
    )?;
    let mut rows = stmt.query([source_id])?;
    if let Some(row) = rows.next()? {
        let content_type = row.get::<_, String>(0)?;
        let file_path = row.get::<_, String>(1)?;
        let custom_name = row.get::<_, Option<String>>(2)?;
        let item_type = if content_type == "image" {
            ProjectContentType::Image
        } else {
            ProjectContentType::Source
        };
        rename_project_content_rows_for_item(
            &conn,
            source_id,
            &[item_type],
            &source_display_name(custom_name, &file_path),
        )?;
    }

    Ok(())
}

pub fn delete_source_content(source_id: &str) -> SqlResult<()> {
    println!("🗑️ DB: Deleting source content: {}", source_id);
    let conn = init_database()?;
    let source_type = conn
        .query_row(
            "SELECT content_type FROM source_content WHERE id = ?1 LIMIT 1",
            [source_id],
            |row| row.get::<_, String>(0),
        )
        .ok();
    let source_item_type = if source_type.as_deref() == Some("image") {
        ProjectContentType::Image
    } else {
        ProjectContentType::Source
    };
    let conversion_ids = {
        let mut stmt =
            conn.prepare("SELECT id FROM ascii_conversions WHERE source_file_id = ?1")?;
        let rows = stmt
            .query_map([source_id], |row| row.get::<_, String>(0))?
            .collect::<SqlResult<Vec<_>>>()?;
        rows
    };
    let cut_ids = {
        let mut stmt = conn.prepare("SELECT id FROM cuts WHERE source_file_id = ?1")?;
        let rows = stmt
            .query_map([source_id], |row| row.get::<_, String>(0))?
            .collect::<SqlResult<Vec<_>>>()?;
        rows
    };
    let preview_ids = {
        let mut stmt = conn.prepare("SELECT id FROM previews WHERE source_file_id = ?1")?;
        let rows = stmt
            .query_map([source_id], |row| row.get::<_, String>(0))?
            .collect::<SqlResult<Vec<_>>>()?;
        rows
    };

    delete_project_content_rows_for_item(&conn, source_id, &[source_item_type])?;
    for conversion_id in conversion_ids {
        delete_project_content_rows_for_item(&conn, &conversion_id, &[ProjectContentType::Frames])?;
    }
    for cut_id in cut_ids {
        delete_project_content_rows_for_item(&conn, &cut_id, &[ProjectContentType::Cut])?;
    }
    for preview_id in preview_ids {
        delete_project_content_rows_for_item(
            &conn,
            &preview_id,
            &[ProjectContentType::Preview, ProjectContentType::Frame],
        )?;
    }

    let result = conn.execute(
        "DELETE FROM ascii_conversions WHERE source_file_id = ?1",
        [source_id],
    );
    match &result {
        Ok(rows) => println!("🗑️ DB: Deleted {} associated conversions", rows),
        Err(e) => println!("🗑️ DB: Error deleting conversions: {}", e),
    }
    result?;

    let result = conn.execute("DELETE FROM cuts WHERE source_file_id = ?1", [source_id]);
    match &result {
        Ok(rows) => println!("🗑️ DB: Deleted {} associated cuts", rows),
        Err(e) => println!("🗑️ DB: Error deleting cuts: {}", e),
    }
    result?;

    let result = conn.execute("DELETE FROM audio WHERE source_file_id = ?1", [source_id]);
    match &result {
        Ok(rows) => println!("🗑️ DB: Deleted {} associated audio extractions", rows),
        Err(e) => println!("🗑️ DB: Error deleting audio: {}", e),
    }
    result?;

    let result = conn.execute(
        "DELETE FROM previews WHERE source_file_id = ?1",
        [source_id],
    );
    match &result {
        Ok(rows) => println!("🗑️ DB: Deleted {} associated previews", rows),
        Err(e) => println!("🗑️ DB: Error deleting previews: {}", e),
    }
    result?;

    let result = conn.execute("DELETE FROM source_content WHERE id = ?1", [source_id]);
    match &result {
        Ok(rows) => println!("🗑️ DB: Deleted {} source content rows", rows),
        Err(e) => println!("🗑️ DB: Error deleting source content: {}", e),
    }
    result?;

    Ok(())
}
