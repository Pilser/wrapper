/// Tool names the wrapper-side MCP bridge understands.
/// Supervisors forward agent tool calls here; each prefix maps to a
/// `tools::*` handler backed by the per-tenant DB.
pub const TOOL_NAMES: &[&str] = &[
    "memory.save",
    "memory.get",
    "memory.list",
    "profile.get",
    "profile.update",
    "history.append",
    "history.list",
    "projects.list",
    "projects.get",
    "skills.list",
    "skills.get",
    "reports.list",
    "reports.get",
    "configs.get",
    "configs.set",
    "tenant.info",
];

pub fn tool_names() -> Vec<String> {
    TOOL_NAMES.iter().map(|s| s.to_string()).collect()
}

pub fn is_known_tool(name: &str) -> bool {
    TOOL_NAMES.contains(&name)
}
