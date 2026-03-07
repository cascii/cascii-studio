use std::cell::RefCell;
use std::collections::HashMap;

use super::open::Project;
use super::project::{FrameDirectory, Preview, SourceContent};
use crate::components::explorer::SidebarState;
use crate::components::settings::available_cuts::VideoCut;

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectSidebarCache {
    pub project: Option<Project>,
    pub source_files: Vec<SourceContent>,
    pub frame_directories: Vec<FrameDirectory>,
    pub video_cuts: Vec<VideoCut>,
    pub previews: Vec<Preview>,
    pub sidebar_state: SidebarState,
}

impl Default for ProjectSidebarCache {
    fn default() -> Self {
        Self {
            project: None,
            source_files: Vec::new(),
            frame_directories: Vec::new(),
            video_cuts: Vec::new(),
            previews: Vec::new(),
            sidebar_state: SidebarState::default(),
        }
    }
}

thread_local! {
    static PROJECT_SIDEBAR_CACHE: RefCell<HashMap<String, ProjectSidebarCache>> =
        RefCell::new(HashMap::new());
}

pub fn get_project_sidebar_cache(project_id: &str) -> Option<ProjectSidebarCache> {
    if project_id.is_empty() {
        return None;
    }

    PROJECT_SIDEBAR_CACHE.with(|cache| cache.borrow().get(project_id).cloned())
}

pub fn set_project_sidebar_cache(project_id: &str, data: ProjectSidebarCache) {
    if project_id.is_empty() {
        return;
    }

    PROJECT_SIDEBAR_CACHE.with(|cache| {
        cache.borrow_mut().insert(project_id.to_string(), data);
    });
}
