mod content;
mod context_menu;
mod conversion;
mod cuts;
mod export;
mod media;
mod preview;
mod project;
mod settings_cmd;
mod source;
mod timeline;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(context_menu::ResourcesContextMenuState::default())
        .manage(context_menu::ExplorerContextMenuState::default())
        .manage(context_menu::OpenProjectsContextMenuState::default())
        .on_menu_event(|app, event| {
            context_menu::handle_resources_menu_event(app, &event);
            context_menu::handle_explorer_menu_event(app, &event);
            context_menu::handle_open_projects_menu_event(app, &event);
        })
        .invoke_handler(tauri::generate_handler![
            crate::util::greet,
            settings_cmd::load_settings,
            settings_cmd::save_settings,
            settings_cmd::set_loop_enabled,
            settings_cmd::get_loop_enabled,
            crate::util::pick_directory,
            crate::util::pick_save_file_mp4,
            crate::util::pick_export_directory,
            crate::util::open_directory,
            source::pick_files,
            source::add_source_files,
            project::create_project,
            project::get_all_projects,
            project::rename_project,
            project::open_project_folder,
            project::get_project,
            source::get_project_sources,
            conversion::get_project_conversions,
            conversion::get_conversion_by_folder_path,
            conversion::get_project_frames,
            conversion::get_frame_files,
            conversion::read_frame_file,
            conversion::read_cframe_file,
            project::duplicate_project,
            project::duplicate_resource,
            project::delete_project,
            conversion::delete_frame_directory,
            conversion::update_frame_custom_name,
            media::prepare_media,
            conversion::convert_to_ascii,
            conversion::update_conversion_frame_speed,
            source::rename_source_file,
            source::delete_source_file,
            cuts::cut_video,
            cuts::get_project_cuts,
            cuts::delete_cut,
            cuts::rename_cut,
            conversion::cut_frames,
            conversion::crop_frames,
            cuts::preprocess_video,
            crate::ffmpeg::check_system_ffmpeg,
            crate::ffmpeg::check_sidecar_ffmpeg,
            preview::create_preview,
            preview::get_project_previews,
            timeline::get_active_project_timeline,
            timeline::save_project_timeline,
            preview::delete_preview,
            preview::rename_preview,
            content::get_project_content,
            content::save_project_content,
            context_menu::show_resources_context_menu,
            context_menu::show_explorer_context_menu,
            context_menu::show_open_projects_context_menu,
            export::export_timeline_mp4
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
