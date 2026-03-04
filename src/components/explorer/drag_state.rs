use std::cell::RefCell;

thread_local! {
    static ACTIVE_RESOURCE_DRAG_NODE_ID: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn set_active_resource_drag(node_id: Option<String>) {
    ACTIVE_RESOURCE_DRAG_NODE_ID.with(|state| {
        *state.borrow_mut() = node_id;
    });
}

pub fn get_active_resource_drag() -> Option<String> {
    ACTIVE_RESOURCE_DRAG_NODE_ID.with(|state| state.borrow().clone())
}
