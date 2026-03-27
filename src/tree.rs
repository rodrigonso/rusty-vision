use anyhow::{Context, Result, bail};
use serde::Serialize;
use uiautomation::types::UIProperty;
use uiautomation::{UIAutomation, UIElement, UITreeWalker};

#[derive(Debug, Serialize)]
pub struct BoundingRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Serialize)]
pub struct TreeNode {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u32>,
    role: String,
    name: String,
    class_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bounding_rect: Option<BoundingRect>,
    is_enabled: bool,
    children: Vec<TreeNode>,
}

/// Inspect the UI element tree of a window belonging to the given PID.
pub fn inspect_tree(pid: u32, max_depth: Option<usize>) -> Result<TreeNode> {
    let automation =
        UIAutomation::new().context("Failed to initialize UI Automation")?;
    let walker = automation
        .get_control_view_walker()
        .context("Failed to get control view walker")?;
    let root = automation
        .get_root_element()
        .context("Failed to get desktop root element")?;

    // Find the top-level window matching the target PID
    let window = find_window_by_pid(&walker, &root, pid)?;

    walk_element(&walker, &window, 0, max_depth)
}

/// Walk top-level children of the desktop root to find a window owned by `pid`.
fn find_window_by_pid(
    walker: &UITreeWalker,
    root: &UIElement,
    pid: u32,
) -> Result<UIElement> {
    let child = walker
        .get_first_child(root)
        .context("Desktop has no child windows")?;

    if matches_pid(&child, pid) {
        return Ok(child);
    }

    let mut current = child;
    while let Ok(sibling) = walker.get_next_sibling(&current) {
        if matches_pid(&sibling, pid) {
            return Ok(sibling);
        }
        current = sibling;
    }

    bail!(
        "No UI Automation element found for PID {pid}.\n\
         The application may not expose a UI Automation tree."
    );
}

fn matches_pid(element: &UIElement, pid: u32) -> bool {
    element
        .get_property_value(UIProperty::ProcessId)
        .ok()
        .and_then(|v| v.try_into().ok())
        .map(|p: i32| p as u32 == pid)
        .unwrap_or(false)
}

fn walk_element(
    walker: &UITreeWalker,
    element: &UIElement,
    depth: usize,
    max_depth: Option<usize>,
) -> Result<TreeNode> {
    let name = element.get_name().unwrap_or_default();
    let class_name = element.get_classname().unwrap_or_default();
    let role = element.get_localized_control_type().unwrap_or_default();

    let bounding_rect = element.get_bounding_rectangle().ok().map(|r| BoundingRect {
        x: r.get_left(),
        y: r.get_top(),
        width: r.get_right() - r.get_left(),
        height: r.get_bottom() - r.get_top(),
    });

    let is_enabled = element
        .get_property_value(UIProperty::IsEnabled)
        .ok()
        .and_then(|v| v.try_into().ok())
        .unwrap_or(false);

    let at_depth_limit = max_depth.is_some_and(|max| depth >= max);

    let children = if at_depth_limit {
        Vec::new()
    } else {
        collect_children(walker, element, depth, max_depth)
    };

    Ok(TreeNode {
        id: None,
        role,
        name,
        class_name,
        bounding_rect,
        is_enabled,
        children,
    })
}

/// Assign sequential IDs to all nodes via depth-first traversal.
pub fn assign_ids(node: &mut TreeNode) {
    let mut counter = 0u32;
    assign_ids_recursive(node, &mut counter);
}

fn assign_ids_recursive(node: &mut TreeNode, counter: &mut u32) {
    node.id = Some(*counter);
    *counter += 1;
    for child in &mut node.children {
        assign_ids_recursive(child, counter);
    }
}

/// Collect all nodes that should be annotated (actionable elements with visible bounding rects).
pub fn collect_annotatable_nodes(node: &TreeNode) -> Vec<(u32, &BoundingRect, &str, &str)> {
    let mut result = Vec::new();
    collect_recursive(node, &mut result);
    result
}

const ACTIONABLE_ROLES: &[&str] = &[
    "button", "menu item", "menu bar", "split button", "tab item",
    "combo box", "text", "edit", "document", "slider", "check box",
    "radio button", "link", "list item", "tree item", "app bar button",
    "toggle button", "custom", "group",
];

fn is_actionable(role: &str) -> bool {
    ACTIONABLE_ROLES.iter().any(|&r| r == role)
}

fn collect_recursive<'a>(
    node: &'a TreeNode,
    result: &mut Vec<(u32, &'a BoundingRect, &'a str, &'a str)>,
) {
    if let (Some(id), Some(rect)) = (node.id, &node.bounding_rect) {
        if is_actionable(&node.role) && rect.width > 0 && rect.height > 0 {
            result.push((id, rect, &node.role, &node.name));
        }
    }
    for child in &node.children {
        collect_recursive(child, result);
    }
}

fn collect_children(
    walker: &UITreeWalker,
    parent: &UIElement,
    parent_depth: usize,
    max_depth: Option<usize>,
) -> Vec<TreeNode> {
    let mut children = Vec::new();

    let Ok(first) = walker.get_first_child(parent) else {
        return children;
    };

    if let Ok(node) = walk_element(walker, &first, parent_depth + 1, max_depth) {
        children.push(node);
    }

    let mut current = first;
    while let Ok(sibling) = walker.get_next_sibling(&current) {
        if let Ok(node) = walk_element(walker, &sibling, parent_depth + 1, max_depth) {
            children.push(node);
        }
        current = sibling;
    }

    children
}
