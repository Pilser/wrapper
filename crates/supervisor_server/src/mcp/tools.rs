/// Tool names the wrapper-side MCP bridge understands.
/// Portfolio ledger — hard cut, no memory/projects/history legacy.
pub const TOOL_NAMES: &[&str] = &[
    "portfolio.show",
    "portfolio.update",
    "portfolio.create_goal",
    "portfolio.update_goal",
    "portfolio.list_goals",
    "portfolio.get_goal",
    "portfolio.add_milestone",
    "portfolio.list_milestones",
    "portfolio.update_milestone",
    "portfolio.create_task",
    "portfolio.update_task",
    "portfolio.list_tasks",
    "portfolio.get_task",
    "portfolio.submit_attempt",
    "portfolio.resolve_attempt",
    "portfolio.list_attempts",
    "portfolio.get_attempt",
    "portfolio.record_skill",
    "portfolio.update_skill",
    "portfolio.list_skills",
    "portfolio.get_skill",
    "portfolio.award_achievement",
    "portfolio.list_achievements",
    "portfolio.get_achievement",
    "portfolio.generate_report",
    "portfolio.list_reports",
    "portfolio.get_report",
    "portfolio.list_timeline",
    "portfolio.get_timeline",
    "portfolio.add_timeline",
];

pub fn tool_names() -> Vec<String> {
    TOOL_NAMES.iter().map(|s| s.to_string()).collect()
}

pub fn is_known_tool(name: &str) -> bool {
    TOOL_NAMES.contains(&name)
}
