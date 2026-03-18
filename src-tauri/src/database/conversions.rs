use super::{
    default_output_mode, delete_project_content_rows_for_item, frame_display_name, init_database,
    rename_project_content_rows_for_item, AsciiConversion, ConversionSettings, ProjectContentType,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Result as SqlResult};

pub fn add_ascii_conversion(conversion: &AsciiConversion) -> SqlResult<()> {
    let conn = init_database()?;

    let font_ratio_rounded = (conversion.settings.font_ratio as f64 * 100.0).round() / 100.0;

    conn.execute(
        "INSERT INTO ascii_conversions (id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, frame_speed, creation_date, total_size, custom_name, color, output_mode, foreground_color, background_color)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            conversion.id,
            conversion.folder_name,
            conversion.folder_path,
            conversion.frame_count,
            conversion.source_file_id,
            conversion.project_id,
            conversion.settings.luminance,
            font_ratio_rounded,
            conversion.settings.columns,
            conversion.settings.fps,
            conversion.settings.frame_speed,
            conversion.creation_date.to_rfc3339(),
            conversion.total_size,
            conversion.custom_name,
            conversion.settings.color,
            conversion.settings.output_mode,
            conversion.settings.foreground_color,
            conversion.settings.background_color,
        ],
    )?;

    Ok(())
}

pub fn get_project_conversions(project_id: &str) -> SqlResult<Vec<AsciiConversion>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, frame_speed, creation_date, total_size, custom_name, color, output_mode, foreground_color, background_color
         FROM ascii_conversions
         WHERE project_id = ?1
         ORDER BY creation_date DESC",
    )?;

    let conversions = stmt
        .query_map([project_id], |row| {
            let date_str: String = row.get(11)?;

            Ok(AsciiConversion {
                id: row.get(0)?,
                folder_name: row.get(1)?,
                folder_path: row.get(2)?,
                frame_count: row.get(3)?,
                source_file_id: row.get(4)?,
                project_id: row.get(5)?,
                settings: ConversionSettings {
                    luminance: row.get(6)?,
                    font_ratio: row.get(7)?,
                    columns: row.get(8)?,
                    fps: row.get(9)?,
                    frame_speed: row.get(10)?,
                    color: row.get::<_, i32>(14).unwrap_or(0) != 0,
                    output_mode: row
                        .get::<_, Option<String>>(15)?
                        .unwrap_or_else(default_output_mode),
                    foreground_color: row.get(16)?,
                    background_color: row.get(17)?,
                },
                creation_date: DateTime::parse_from_rfc3339(&date_str)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                total_size: row.get(12)?,
                custom_name: row.get(13)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    Ok(conversions)
}

pub fn get_conversion(conversion_id: &str) -> SqlResult<Option<AsciiConversion>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, frame_speed, creation_date, total_size, custom_name, color, output_mode, foreground_color, background_color
         FROM ascii_conversions
         WHERE id = ?1
         LIMIT 1",
    )?;
    let mut rows = stmt.query([conversion_id])?;
    if let Some(row) = rows.next()? {
        let date_str: String = row.get(11)?;
        Ok(Some(AsciiConversion {
            id: row.get(0)?,
            folder_name: row.get(1)?,
            folder_path: row.get(2)?,
            frame_count: row.get(3)?,
            source_file_id: row.get(4)?,
            project_id: row.get(5)?,
            settings: ConversionSettings {
                luminance: row.get(6)?,
                font_ratio: row.get(7)?,
                columns: row.get(8)?,
                fps: row.get(9)?,
                frame_speed: row.get(10)?,
                color: row.get::<_, i32>(14).unwrap_or(0) != 0,
                output_mode: row.get::<_, Option<String>>(15)?.unwrap_or_else(default_output_mode),
                foreground_color: row.get(16)?,
                background_color: row.get(17)?,
            },
            creation_date: DateTime::parse_from_rfc3339(&date_str).unwrap_or_else(|_| Utc::now().into()).with_timezone(&Utc),
            total_size: row.get(12)?,
            custom_name: row.get(13)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn update_conversion_frame_speed(conversion_id: &str, frame_speed: u32) -> SqlResult<()> {
    println!(
        "📝 DB: Updating frame_speed for conversion {} to {}",
        conversion_id, frame_speed
    );
    let conn = init_database()?;

    let result = conn.execute(
        "UPDATE ascii_conversions SET frame_speed = ?1 WHERE id = ?2",
        params![frame_speed, conversion_id],
    );

    match &result {
        Ok(rows_affected) => println!("📝 DB: Update successful, {} rows affected", rows_affected),
        Err(e) => println!("📝 DB: Update failed: {}", e),
    }

    result?;
    Ok(())
}

pub fn get_conversion_by_folder_path(folder_path: &str) -> SqlResult<Option<AsciiConversion>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, frame_speed, creation_date, total_size, custom_name, color, output_mode, foreground_color, background_color
         FROM ascii_conversions
         WHERE folder_path = ?1
         LIMIT 1",
    )?;

    let mut rows = stmt.query([folder_path])?;

    if let Some(row) = rows.next()? {
        let date_str: String = row.get(11)?;

        Ok(Some(AsciiConversion {
            id: row.get(0)?,
            folder_name: row.get(1)?,
            folder_path: row.get(2)?,
            frame_count: row.get(3)?,
            source_file_id: row.get(4)?,
            project_id: row.get(5)?,
            settings: ConversionSettings {
                luminance: row.get(6)?,
                font_ratio: row.get(7)?,
                columns: row.get(8)?,
                fps: row.get(9)?,
                frame_speed: row.get(10)?,
                color: row.get::<_, i32>(14).unwrap_or(0) != 0,
                output_mode: row
                    .get::<_, Option<String>>(15)?
                    .unwrap_or_else(default_output_mode),
                foreground_color: row.get(16)?,
                background_color: row.get(17)?,
            },
            creation_date: DateTime::parse_from_rfc3339(&date_str)
                .unwrap_or_else(|_| Utc::now().into())
                .with_timezone(&Utc),
            total_size: row.get(12)?,
            custom_name: row.get(13)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn delete_conversion_by_folder_path(folder_path: &str) -> SqlResult<()> {
    println!("🗑️ DB: Deleting conversion by folder path: {}", folder_path);
    let conn = init_database()?;
    if let Some(conversion_id) = conn
        .query_row(
            "SELECT id FROM ascii_conversions WHERE folder_path = ?1 LIMIT 1",
            [folder_path],
            |row| row.get::<_, String>(0),
        )
        .ok()
    {
        delete_project_content_rows_for_item(&conn, &conversion_id, &[ProjectContentType::Frames])?;
    }
    let result = conn.execute(
        "DELETE FROM ascii_conversions WHERE folder_path = ?1",
        [folder_path],
    );

    match &result {
        Ok(rows_affected) => println!("🗑️ DB: Delete successful, {} rows affected", rows_affected),
        Err(e) => println!("🗑️ DB: Delete failed: {}", e),
    }

    result?;
    Ok(())
}

pub fn update_conversion_custom_name(
    conversion_id: &str,
    custom_name: Option<String>,
) -> SqlResult<()> {
    println!(
        "📝 DB: Updating conversion custom_name for {} to {:?}",
        conversion_id, custom_name
    );
    let conn = init_database()?;
    let result = conn.execute(
        "UPDATE ascii_conversions SET custom_name = ?1 WHERE id = ?2",
        params![custom_name, conversion_id],
    );

    match &result {
        Ok(rows_affected) => println!("📝 DB: Update successful, {} rows affected", rows_affected),
        Err(e) => println!("📝 DB: Update failed: {}", e),
    }

    result?;
    let mut stmt = conn.prepare(
        "SELECT folder_name, custom_name
         FROM ascii_conversions
         WHERE id = ?1
         LIMIT 1",
    )?;
    let mut rows = stmt.query([conversion_id])?;
    if let Some(row) = rows.next()? {
        let folder_name = row.get::<_, String>(0)?;
        let custom_name = row.get::<_, Option<String>>(1)?;
        rename_project_content_rows_for_item(
            &conn,
            conversion_id,
            &[ProjectContentType::Frames],
            &frame_display_name(custom_name, &folder_name),
        )?;
    }

    Ok(())
}

pub fn update_conversion_dimensions(
    conversion_id: &str,
    frame_count: i32,
    total_size: i64,
) -> SqlResult<()> {
    let conn = init_database()?;
    conn.execute(
        "UPDATE ascii_conversions SET frame_count = ?1, total_size = ?2 WHERE id = ?3",
        params![frame_count, total_size, conversion_id],
    )?;
    Ok(())
}
