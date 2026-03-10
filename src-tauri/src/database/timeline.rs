use super::{
    init_database, FrameRenderMode, ProjectTimeline, Timeline, TimelineClip, TimelineClipDraft,
    TimelineMediaType, TimelineResourceKind,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqlResult};
use uuid::Uuid;

fn parse_rfc3339_or_now(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .unwrap_or_else(|_| Utc::now().into())
        .with_timezone(&Utc)
}

fn get_timeline_by_id_with_conn(
    conn: &Connection,
    timeline_id: &str,
) -> SqlResult<Option<Timeline>> {
    let mut stmt = conn.prepare(
        "SELECT timeline_id, project_id, creation_date, last_updated
         FROM timelines
         WHERE timeline_id = ?1
         LIMIT 1",
    )?;

    let mut rows = stmt.query([timeline_id])?;
    if let Some(row) = rows.next()? {
        let creation_date: String = row.get(2)?;
        let last_updated: String = row.get(3)?;
        Ok(Some(Timeline {
            timeline_id: row.get(0)?,
            project_id: row.get(1)?,
            creation_date: parse_rfc3339_or_now(&creation_date),
            last_updated: parse_rfc3339_or_now(&last_updated),
        }))
    } else {
        Ok(None)
    }
}

fn get_timeline_clips_with_conn(
    conn: &Connection,
    timeline_id: &str,
) -> SqlResult<Vec<TimelineClip>> {
    let mut stmt = conn.prepare(
        "SELECT clip_id, project_id, timeline_id, order_index, media_type, resource_kind, actual_resource_id, frame_render_mode, length_seconds, creation_date, last_updated
         FROM clips
         WHERE timeline_id = ?1
         ORDER BY order_index ASC, creation_date ASC",
    )?;

    let clips = stmt
        .query_map([timeline_id], |row| {
            let creation_date: String = row.get(9)?;
            let last_updated: String = row.get(10)?;
            let frame_render_mode = row
                .get::<_, Option<String>>(7)?
                .map(|value| FrameRenderMode::from_string(&value));

            Ok(TimelineClip {
                clip_id: row.get(0)?,
                project_id: row.get(1)?,
                timeline_id: row.get(2)?,
                order_index: row.get(3)?,
                media_type: TimelineMediaType::from_string(&row.get::<_, String>(4)?),
                resource_kind: TimelineResourceKind::from_string(&row.get::<_, String>(5)?),
                actual_resource_id: row.get(6)?,
                frame_render_mode,
                length_seconds: row.get(8)?,
                creation_date: parse_rfc3339_or_now(&creation_date),
                last_updated: parse_rfc3339_or_now(&last_updated),
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    Ok(clips)
}

pub fn get_active_project_timeline(project_id: &str) -> SqlResult<ProjectTimeline> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT timeline_id, project_id, creation_date, last_updated
         FROM timelines
         WHERE project_id = ?1
         ORDER BY last_updated DESC, creation_date DESC
         LIMIT 1",
    )?;

    let mut rows = stmt.query([project_id])?;
    let timeline = if let Some(row) = rows.next()? {
        let creation_date: String = row.get(2)?;
        let last_updated: String = row.get(3)?;
        Some(Timeline {
            timeline_id: row.get(0)?,
            project_id: row.get(1)?,
            creation_date: parse_rfc3339_or_now(&creation_date),
            last_updated: parse_rfc3339_or_now(&last_updated),
        })
    } else {
        None
    };

    let clips = if let Some(timeline) = &timeline {
        get_timeline_clips_with_conn(&conn, &timeline.timeline_id)?
    } else {
        Vec::new()
    };

    Ok(ProjectTimeline { timeline, clips })
}

pub fn save_project_timeline(
    project_id: &str,
    timeline_id: Option<&str>,
    clip_drafts: &[TimelineClipDraft],
) -> SqlResult<ProjectTimeline> {
    let mut conn = init_database()?;
    let tx = conn.transaction()?;
    let now = Utc::now();
    let timeline_id = timeline_id
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let existing_timeline = get_timeline_by_id_with_conn(&tx, &timeline_id)?;
    let creation_date = existing_timeline
        .as_ref()
        .map(|timeline| timeline.creation_date.clone())
        .unwrap_or(now);

    tx.execute(
        "INSERT INTO timelines (timeline_id, project_id, creation_date, last_updated)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(timeline_id) DO UPDATE
         SET project_id = excluded.project_id,
             last_updated = excluded.last_updated",
        params![
            timeline_id,
            project_id,
            creation_date.to_rfc3339(),
            now.to_rfc3339(),
        ],
    )?;

    let mut existing_dates = std::collections::HashMap::<String, String>::new();
    {
        let mut stmt =
            tx.prepare("SELECT clip_id, creation_date FROM clips WHERE timeline_id = ?1")?;
        let mut rows = stmt.query([timeline_id.as_str()])?;
        while let Some(row) = rows.next()? {
            existing_dates.insert(row.get(0)?, row.get(1)?);
        }
    }

    tx.execute(
        "DELETE FROM clips WHERE timeline_id = ?1",
        [timeline_id.as_str()],
    )?;

    for (index, clip) in clip_drafts.iter().enumerate() {
        let clip_creation_date = existing_dates
            .get(&clip.clip_id)
            .cloned()
            .unwrap_or_else(|| now.to_rfc3339());

        tx.execute(
            "INSERT INTO clips (clip_id, project_id, timeline_id, order_index, media_type, resource_kind, actual_resource_id, frame_render_mode, length_seconds, creation_date, last_updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                clip.clip_id,
                project_id,
                timeline_id,
                index as i32,
                clip.media_type.to_string(),
                clip.resource_kind.to_string(),
                clip.actual_resource_id,
                clip.frame_render_mode.as_ref().map(FrameRenderMode::to_string),
                clip.length_seconds,
                clip_creation_date,
                now.to_rfc3339(),
            ],
        )?;
    }

    tx.commit()?;
    get_active_project_timeline(project_id)
}
