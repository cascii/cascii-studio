use serde::{Deserialize, Serialize};

/// Unique identifier for any node in the tree.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TreeNodeId(pub String);

/// The kind of resource a leaf node references.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ResourceRef {
    SourceFile { source_id: String },
    VideoCut { cut_id: String },
    FrameDirectory { directory_path: String },
    Preview { preview_id: String },
}

/// A single node in a tree (used by both RESOURCES and EXPLORER sections).
#[derive(Clone, Debug, PartialEq)]
pub struct TreeNode {
    pub id: TreeNodeId,
    pub label: String,
    pub node_kind: NodeKind,
    pub depth: u32,
    pub is_expanded: bool,
    pub is_selected: bool,
    pub is_rename_active: bool,
    pub children: Vec<TreeNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    /// A structural folder (can contain children).
    Folder { is_user_created: bool },
    /// A leaf referencing a real project resource.
    Leaf(ResourceRef),
}

impl TreeNode {
    pub fn is_folder(&self) -> bool {
        matches!(self.node_kind, NodeKind::Folder { .. })
    }

    pub fn is_user_folder(&self) -> bool {
        matches!(self.node_kind, NodeKind::Folder { is_user_created: true })
    }
}

/// Persisted explorer layout for a project (stored as JSON in the database).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct ExplorerLayout {
    pub project_id: String,
    pub root_items: Vec<ExplorerItem>,
}

/// A single item in the user's explorer tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ExplorerItem {
    Folder {
        id: String,
        name: String,
        children: Vec<ExplorerItem>,
        is_expanded: bool,
    },
    ResourceRef(ResourceRef),
}

/// Tracks which top-level sections and sub-sections are expanded.
#[derive(Clone, Debug, PartialEq)]
pub struct SidebarState {
    pub resources_expanded: bool,
    pub explorer_expanded: bool,
    pub controls_expanded: bool,
    pub source_files_expanded: bool,
    pub original_files_expanded: bool,
    pub cuts_expanded: bool,
    pub frames_expanded: bool,
    pub source_frames_expanded: bool,
    pub frame_cuts_expanded: bool,
    pub previews_expanded: bool,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            resources_expanded: true,
            explorer_expanded: true,
            controls_expanded: true,
            source_files_expanded: true,
            original_files_expanded: true,
            cuts_expanded: true,
            frames_expanded: true,
            source_frames_expanded: true,
            frame_cuts_expanded: true,
            previews_expanded: true,
        }
    }
}
