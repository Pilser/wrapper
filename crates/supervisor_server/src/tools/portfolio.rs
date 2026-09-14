use shared::{Error, Result};
use serde_json::Value;
use telemetry::{Level, Telemetry};

pub async fn handle_portfolio_call(
    tenant_id: &str,
    payload: &Value,
    db: &db::Db,
    telemetry: &Telemetry,
) -> Result<Value> {
    crate::validate_tenant_id(tenant_id)?;
    telemetry.builder().level(Level::Info).kind("supervisor.tool.called").category("supervisor").message(format!("portfolio tool called tenant {tenant_id} op {}", payload.get("op").and_then(|v| v.as_str()).unwrap_or(""))).emit();
    let tenant_db = db.tenant(tenant_id).await?;
    let conn = tenant_db.conn().map_err(|e| Error::Database(e.to_string()))?;
    let op = payload.get("op").and_then(|v| v.as_str()).unwrap_or("");
    let now = shared::now_rfc3339();
    let result = match op {
        "show" => {
            let profile = db::tenant::profile::get(&conn, tenant_id).await.map_err(|e| Error::Database(e.to_string()))?;
            let goals = db::tenant::goals::list(&conn, None, Some(100)).await.map_err(|e| Error::Database(e.to_string()))?;
            let mut tasks_open = 0i64; let mut tasks_done = 0i64;
            let mut tasks = db::tenant::tasks::list(&conn, None, None, None).await.map_err(|e| Error::Database(e.to_string()))?;
            for t in &tasks { if t.status=="done" { tasks_done+=1; } else { tasks_open+=1; } }
            tasks.truncate(5);
            let skills = db::tenant::skills::list(&conn, None).await.map_err(|e| Error::Database(e.to_string()))?;
            let achievements = db::tenant::achievements::list(&conn, Some(5)).await.map_err(|e| Error::Database(e.to_string()))?;
            let timeline = db::tenant::timeline::list(&conn, None, None, Some(5)).await.map_err(|e| Error::Database(e.to_string()))?;
            // last 3 attempts across all tasks
            let mut recent_attempts = Vec::new();
            for t in db::tenant::tasks::list(&conn, None, None, None).await.map_err(|e| Error::Database(e.to_string()))?.iter().take(3) {
                if let Ok(a) = db::tenant::attempts::list_by_task(&conn, t.id, Some(1)).await { recent_attempts.extend(a); }
            }
            serde_json::json!({
                "profile": profile,
                "goals_total": goals.len(),
                "goals": goals.into_iter().take(5).collect::<Vec<_>>(),
                "tasks": {"open": tasks_open, "done": tasks_done, "recent": tasks},
                "skills": skills.into_iter().take(5).collect::<Vec<_>>(),
                "achievements": achievements,
                "recent_attempts": recent_attempts.into_iter().take(3).collect::<Vec<_>>(),
                "timeline": timeline,
            })
        }
        "update" => {
            let existing = db::tenant::profile::get(&conn, tenant_id).await.map_err(|e| Error::Database(e.to_string()))?;
            let field = |k: &str| super::str_arg(payload, k).or_else(|| payload.get("updates").and_then(|u| u.get(k)).and_then(|v| v.as_str()));
            let agent_name = field("agent_name").or(field("name")).map(|s| s.to_string()).or_else(|| existing.as_ref().map(|p| p.agent_name.clone())).unwrap_or_else(|| "researcher".to_string());
            let profile = db::tenant::profile::Profile {
                tenant_id: tenant_id.to_string(),
                agent_name,
                nickname: field("nickname").map(|s| s.to_string()).or_else(|| existing.as_ref().and_then(|p| p.nickname.clone())),
                language: field("language").map(|s| s.to_string()).or_else(|| existing.as_ref().map(|p| p.language.clone())).unwrap_or_else(|| "en".to_string()),
                timezone: field("timezone").map(|s| s.to_string()).or_else(|| existing.as_ref().map(|p| p.timezone.clone())).unwrap_or_else(|| "UTC".to_string()),
                track: field("track").map(|s| s.to_string()).or_else(|| existing.as_ref().and_then(|p| p.track.clone())),
                created_at: existing.as_ref().map(|p| p.created_at.clone()).unwrap_or_else(|| now.clone()),
                updated_at: now.clone(),
            };
            db::tenant::profile::upsert(&conn, &profile).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&profile).map_err(Error::from)?
        }
        "create_goal" => {
            let title = super::str_arg(payload, "title").ok_or_else(|| Error::InvalidInput("portfolio.create_goal missing title".to_string()))?;
            let track = super::str_arg(payload, "track");
            let target_date = super::str_arg(payload, "target_date");
            let id = db::tenant::goals::insert(&conn, title, track, target_date, &now).await.map_err(|e| Error::Database(e.to_string()))?;
            db::tenant::timeline::insert(&conn, "goal", Some(id), &format!("goal created: {title}"), Some("goal"), &now).await.ok();
            serde_json::json!({"id": id, "title": title})
        }
        "update_goal" => {
            let id = super::int_arg(payload, "id").ok_or_else(|| Error::InvalidInput("portfolio.update_goal missing id".to_string()))?;
            let title = super::str_arg(payload, "title");
            let status = super::str_arg(payload, "status");
            let target_date = super::str_arg(payload, "target_date");
            db::tenant::goals::update(&conn, id, title, status, target_date, &now).await.map_err(|e| Error::Database(e.to_string()))?;
            let row = db::tenant::goals::get(&conn, id).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&row).map_err(Error::from)?
        }
        "list_goals" => {
            let status = super::str_arg(payload, "status");
            let limit = super::int_arg(payload, "limit");
            let rows = db::tenant::goals::list(&conn, status, limit).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&rows).map_err(Error::from)?
        }
        "get_goal" => {
            let id = super::int_arg(payload, "id").ok_or_else(|| Error::InvalidInput("portfolio.get_goal missing id".to_string()))?;
            let row = db::tenant::goals::get(&conn, id).await.map_err(|e| Error::Database(e.to_string()))?;
            match row { Some(r) => serde_json::to_value(&r).map_err(Error::from)?, None => Value::Null }
        }
        "add_milestone" => {
            let goal_id = super::int_arg(payload, "goal_id").or_else(|| super::int_arg(payload, "goal")).ok_or_else(|| Error::InvalidInput("portfolio.add_milestone missing goal_id".to_string()))?;
            let title = super::str_arg(payload, "title").ok_or_else(|| Error::InvalidInput("portfolio.add_milestone missing title".to_string()))?;
            let order = super::int_arg(payload, "order_index").or(super::int_arg(payload, "order"));
            let id = db::tenant::milestones::insert(&conn, goal_id, title, order, &now).await.map_err(|e| Error::Database(e.to_string()))?;
            db::tenant::timeline::insert(&conn, "milestone", Some(id), &format!("milestone added: {title}"), Some("milestone"), &now).await.ok();
            serde_json::json!({"id": id, "goal_id": goal_id, "title": title})
        }
        "list_milestones" => {
            let goal_id = super::int_arg(payload, "goal_id").or_else(|| super::int_arg(payload, "goal")).ok_or_else(|| Error::InvalidInput("portfolio.list_milestones missing goal_id".to_string()))?;
            let rows = db::tenant::milestones::list_by_goal(&conn, goal_id).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&rows).map_err(Error::from)?
        }
        "update_milestone" => {
            let id = super::int_arg(payload, "id").ok_or_else(|| Error::InvalidInput("portfolio.update_milestone missing id".to_string()))?;
            let title = super::str_arg(payload, "title");
            let status = super::str_arg(payload, "status");
            db::tenant::milestones::update(&conn, id, title, status, &now).await.map_err(|e| Error::Database(e.to_string()))?;
            let row = db::tenant::milestones::get(&conn, id).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&row).map_err(Error::from)?
        }
        "create_task" => {
            let milestone_id = super::int_arg(payload, "milestone_id").or_else(|| super::int_arg(payload, "milestone")).or_else(|| super::int_arg(payload, "roadmap")).ok_or_else(|| Error::InvalidInput("portfolio.create_task missing milestone_id".to_string()))?;
            let title = super::str_arg(payload, "title").ok_or_else(|| Error::InvalidInput("portfolio.create_task missing title".to_string()))?;
            let kind = super::str_arg(payload, "kind").unwrap_or("task");
            let description = super::str_arg(payload, "description");
            let difficulty = super::str_arg(payload, "difficulty");
            let id = db::tenant::tasks::insert(&conn, milestone_id, title, kind, description, difficulty, &now).await.map_err(|e| Error::Database(e.to_string()))?;
            db::tenant::timeline::insert(&conn, "task", Some(id), &format!("task created: {title}"), Some("task"), &now).await.ok();
            serde_json::json!({"id": id, "milestone_id": milestone_id, "title": title})
        }
        "update_task" => {
            let id = super::int_arg(payload, "id").ok_or_else(|| Error::InvalidInput("portfolio.update_task missing id".to_string()))?;
            let title = super::str_arg(payload, "title");
            let status = super::str_arg(payload, "status");
            let description = super::str_arg(payload, "description");
            db::tenant::tasks::update(&conn, id, title, status, description, &now).await.map_err(|e| Error::Database(e.to_string()))?;
            let row = db::tenant::tasks::get(&conn, id).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&row).map_err(Error::from)?
        }
        "list_tasks" => {
            let milestone_id = super::int_arg(payload, "milestone_id").or_else(|| super::int_arg(payload, "milestone")).or_else(|| super::int_arg(payload, "roadmap"));
            let status = super::str_arg(payload, "status");
            let kind = super::str_arg(payload, "kind");
            let rows = db::tenant::tasks::list(&conn, milestone_id, status, kind).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&rows).map_err(Error::from)?
        }
        "get_task" => {
            let id = super::int_arg(payload, "id").ok_or_else(|| Error::InvalidInput("portfolio.get_task missing id".to_string()))?;
            let row = db::tenant::tasks::get(&conn, id).await.map_err(|e| Error::Database(e.to_string()))?;
            match row { Some(r) => serde_json::to_value(&r).map_err(Error::from)?, None => Value::Null }
        }
        "submit_attempt" => {
            let task_id = super::int_arg(payload, "task_id").or_else(|| super::int_arg(payload, "task")).ok_or_else(|| Error::InvalidInput("portfolio.submit_attempt missing task_id".to_string()))?;
            let result = super::str_arg(payload, "result").unwrap_or("fail");
            let output = super::str_arg(payload, "output").or_else(|| super::str_arg(payload, "student_output"));
            let feedback = super::str_arg(payload, "feedback");
            let id = db::tenant::attempts::insert(&conn, task_id, result, output, feedback, &now).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::json!({"id": id, "task_id": task_id, "result": result})
        }
        "resolve_attempt" => {
            let id = super::int_arg(payload, "id").ok_or_else(|| Error::InvalidInput("portfolio.resolve_attempt missing id".to_string()))?;
            let example = super::str_arg(payload, "example").or_else(|| super::str_arg(payload, "example_solution")).ok_or_else(|| Error::InvalidInput("portfolio.resolve_attempt missing example".to_string()))?;
            let note = super::str_arg(payload, "note");
            db::tenant::attempts::resolve(&conn, id, example, note, &now).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::json!({"id": id, "resolved": true})
        }
        "list_attempts" => {
            let task_id = super::int_arg(payload, "task_id").or_else(|| super::int_arg(payload, "task")).ok_or_else(|| Error::InvalidInput("portfolio.list_attempts missing task_id".to_string()))?;
            let limit = super::int_arg(payload, "limit");
            let rows = db::tenant::attempts::list_by_task(&conn, task_id, limit).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&rows).map_err(Error::from)?
        }
        "get_attempt" => {
            let id = super::int_arg(payload, "id").ok_or_else(|| Error::InvalidInput("portfolio.get_attempt missing id".to_string()))?;
            let row = db::tenant::attempts::get(&conn, id).await.map_err(|e| Error::Database(e.to_string()))?;
            match row { Some(r) => serde_json::to_value(&r).map_err(Error::from)?, None => Value::Null }
        }
        "record_skill" => {
            let name = super::str_arg(payload, "name").ok_or_else(|| Error::InvalidInput("portfolio.record_skill missing name".to_string()))?;
            let domain = super::str_arg(payload, "domain");
            let level = super::str_arg(payload, "level");
            let evidence = super::int_arg(payload, "evidence");
            let confidence = super::float_arg(payload, "confidence");
            let id = db::tenant::skills::insert(&conn, name, domain, level, evidence, confidence, &now).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::json!({"id": id, "name": name})
        }
        "update_skill" => {
            let id = super::int_arg(payload, "id").ok_or_else(|| Error::InvalidInput("portfolio.update_skill missing id".to_string()))?;
            let level = super::str_arg(payload, "level");
            let evidence = super::int_arg(payload, "evidence");
            db::tenant::skills::update(&conn, id, level, evidence, &now).await.map_err(|e| Error::Database(e.to_string()))?;
            let row = db::tenant::skills::get(&conn, id).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&row).map_err(Error::from)?
        }
        "list_skills" => {
            let domain = super::str_arg(payload, "domain");
            let rows = db::tenant::skills::list(&conn, domain).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&rows).map_err(Error::from)?
        }
        "get_skill" => {
            let id = super::int_arg(payload, "id").ok_or_else(|| Error::InvalidInput("portfolio.get_skill missing id".to_string()))?;
            let row = db::tenant::skills::get(&conn, id).await.map_err(|e| Error::Database(e.to_string()))?;
            match row { Some(r) => serde_json::to_value(&r).map_err(Error::from)?, None => Value::Null }
        }
        "award_achievement" => {
            let title = super::str_arg(payload, "title").ok_or_else(|| Error::InvalidInput("portfolio.award_achievement missing title".to_string()))?;
            let skill_id = super::int_arg(payload, "skill_id").or_else(|| super::int_arg(payload, "skill"));
            let task_id = super::int_arg(payload, "task_id").or_else(|| super::int_arg(payload, "task"));
            let id = db::tenant::achievements::insert(&conn, title, skill_id, task_id, &now).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::json!({"id": id, "title": title})
        }
        "list_achievements" => {
            let limit = super::int_arg(payload, "limit");
            let rows = db::tenant::achievements::list(&conn, limit).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&rows).map_err(Error::from)?
        }
        "get_achievement" => {
            let id = super::int_arg(payload, "id").ok_or_else(|| Error::InvalidInput("portfolio.get_achievement missing id".to_string()))?;
            let row = db::tenant::achievements::get(&conn, id).await.map_err(|e| Error::Database(e.to_string()))?;
            match row { Some(r) => serde_json::to_value(&r).map_err(Error::from)?, None => Value::Null }
        }
        "generate_report" => {
            let period = super::str_arg(payload, "period").unwrap_or("weekly");
            let (start, end) = if period=="monthly" { (shared::now_rfc3339(), shared::now_rfc3339()) } else { (shared::now_rfc3339(), shared::now_rfc3339()) };
            let content = super::str_arg(payload, "content").unwrap_or("auto-generated report");
            let highlights = super::str_arg(payload, "highlights");
            let gaps = super::str_arg(payload, "gaps");
            let next_steps = super::str_arg(payload, "next_steps");
            let id = db::tenant::reports::insert(&conn, &start, &end, content, highlights, gaps, next_steps, &now).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::json!({"id": id, "period": period})
        }
        "list_reports" => {
            let limit = super::int_arg(payload, "limit");
            let rows = db::tenant::reports::list(&conn, limit).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&rows).map_err(Error::from)?
        }
        "get_report" => {
            let id = super::int_arg(payload, "id").ok_or_else(|| Error::InvalidInput("portfolio.get_report missing id".to_string()))?;
            let row = db::tenant::reports::get(&conn, id).await.map_err(|e| Error::Database(e.to_string()))?;
            match row { Some(r) => serde_json::to_value(&r).map_err(Error::from)?, None => Value::Null }
        }
        "list_timeline" => {
            let kind = super::str_arg(payload, "kind");
            let since = super::str_arg(payload, "since");
            let limit = super::int_arg(payload, "limit");
            let rows = db::tenant::timeline::list(&conn, kind, since, limit).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&rows).map_err(Error::from)?
        }
        "get_timeline" => {
            let id = super::int_arg(payload, "id").ok_or_else(|| Error::InvalidInput("portfolio.get_timeline missing id".to_string()))?;
            let row = db::tenant::timeline::get(&conn, id).await.map_err(|e| Error::Database(e.to_string()))?;
            match row { Some(r) => serde_json::to_value(&r).map_err(Error::from)?, None => Value::Null }
        }
        "add_timeline" => {
            let summary = super::str_arg(payload, "summary").ok_or_else(|| Error::InvalidInput("portfolio.add_timeline missing summary".to_string()))?;
            let kind = super::str_arg(payload, "kind").unwrap_or("note");
            let ref_id = super::int_arg(payload, "ref_id").or_else(|| super::int_arg(payload, "ref-id"));
            let ref_type = super::str_arg(payload, "ref_type").or_else(|| super::str_arg(payload, "ref-type"));
            let id = db::tenant::timeline::insert(&conn, kind, ref_id, summary, ref_type, &now).await.map_err(|e| Error::Database(e.to_string()))?;
            serde_json::json!({"id": id, "kind": kind})
        }
        _ => return Err(Error::InvalidInput(format!("unknown portfolio op {op}"))),
    };
    telemetry.builder().level(Level::Info).kind("supervisor.tool.completed").category("supervisor").message(format!("portfolio tool completed tenant {tenant_id}")).emit();
    Ok(result)
}
