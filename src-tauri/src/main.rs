#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::{Datelike, Duration, Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::{timeout, Duration as TokioDuration};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
  codex_home_override: Option<String>,
  refresh_interval_seconds: u64,
  start_on_login: bool,
  notifications_enabled: bool,
  notification_thresholds: Vec<u8>,
  daily_budget_tokens: Option<u64>,
  weekly_budget_tokens: Option<u64>,
  monthly_budget_tokens: Option<u64>,
  show_fallback_when_rpc_unavailable: bool,
}

impl Default for AppSettings {
  fn default() -> Self {
    Self {
      codex_home_override: None,
      refresh_interval_seconds: 60,
      start_on_login: false,
      notifications_enabled: true,
      notification_thresholds: vec![50, 75, 90, 100],
      daily_budget_tokens: None,
      weekly_budget_tokens: None,
      monthly_budget_tokens: None,
      show_fallback_when_rpc_unavailable: true,
    }
  }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AppStatus {
  platform: String,
  cli: serde_json::Value,
  rpc: serde_json::Value,
  paths: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UsageSnapshot {
  official: serde_json::Value,
  local_buckets: Vec<serde_json::Value>,
  recent_sessions: Vec<serde_json::Value>,
  last_updated_at: String,
  parser_warnings: Vec<String>,
}

#[derive(Default)]
struct AppState {
  settings: Mutex<AppSettings>,
  last_snapshot: Mutex<Option<UsageSnapshot>>,
  notified_keys: Mutex<HashSet<String>>,
}

fn threshold_crossings(previous_percent: f64, next_percent: f64, thresholds: &[u8]) -> Vec<u8> {
  thresholds
    .iter()
    .copied()
    .filter(|t| previous_percent < *t as f64 && next_percent >= *t as f64)
    .collect()
}

fn dedupe_key(limit_id: &str, resets_at: &str) -> String {
  format!("{limit_id}:{resets_at}")
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Result<AppSettings, String> {
  state.settings.lock().map_err(|e| e.to_string()).map(|s| s.clone())
}

#[tauri::command]
fn update_settings(state: State<AppState>, settings: AppSettings) -> Result<AppSettings, String> {
  let mut locked = state.settings.lock().map_err(|e| e.to_string())?;
  *locked = settings.clone();
  Ok(settings)
}

#[tauri::command]
fn set_start_on_login(state: State<AppState>, enabled: bool) -> Result<(), String> {
  let mut locked = state.settings.lock().map_err(|e| e.to_string())?;
  locked.start_on_login = enabled;
  Ok(())
}

#[tauri::command]
fn show_main_window(app: AppHandle) -> Result<(), String> {
  let window = app.get_webview_window("main").ok_or("main window missing")?;
  window.show().map_err(|e| e.to_string())?;
  window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
fn hide_main_window(app: AppHandle) -> Result<(), String> {
  let window = app.get_webview_window("main").ok_or("main window missing")?;
  window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
fn quit_app(app: AppHandle) -> Result<(), String> {
  app.exit(0);
  Ok(())
}

#[tauri::command]
async fn get_app_status(state: State<'_, AppState>) -> Result<AppStatus, String> {
  let settings = state.settings.lock().map_err(|e| e.to_string())?.clone();
  Ok(build_app_status(&settings))
}

#[tauri::command]
async fn get_usage_snapshot(state: State<'_, AppState>) -> Result<UsageSnapshot, String> {
  if let Some(s) = state.last_snapshot.lock().map_err(|e| e.to_string())?.clone() {
    return Ok(s);
  }
  refresh_usage(state).await
}

#[tauri::command]
async fn refresh_usage(state: State<'_, AppState>) -> Result<UsageSnapshot, String> {
  let settings = state.settings.lock().map_err(|e| e.to_string())?.clone();
  let snapshot = fetch_snapshot(&settings).await?;
  let mut last = state.last_snapshot.lock().map_err(|e| e.to_string())?;
  *last = Some(snapshot.clone());
  Ok(snapshot)
}

fn default_codex_home(override_path: &Option<String>) -> PathBuf {
  if let Some(value) = override_path {
    return PathBuf::from(value);
  }
  dirs::home_dir().unwrap_or_else(|| PathBuf::from("~")).join(".codex")
}

fn build_app_status(settings: &AppSettings) -> AppStatus {
  let codex_home = default_codex_home(&settings.codex_home_override);
  let sessions_dir = codex_home.join("sessions");
  AppStatus {
    platform: std::env::consts::OS.to_string(),
    cli: serde_json::json!({"installed": true}),
    rpc: serde_json::json!({"available": true}),
    paths: serde_json::json!({
      "codexHome": codex_home,
      "exists": codex_home.exists(),
      "sessionsDirExists": sessions_dir.exists()
    }),
  }
}

async fn fetch_snapshot(settings: &AppSettings) -> Result<UsageSnapshot, String> {
  let rpc = read_rpc_usage().await;
  let fallback = read_local_usage(settings);
  let now = Utc::now().to_rfc3339();
  match rpc {
    Ok(official) => Ok(UsageSnapshot {
      official,
      local_buckets: fallback.0,
      recent_sessions: fallback.1,
      parser_warnings: vec![],
      last_updated_at: now,
    }),
    Err(err) => {
      let local_official = read_local_official_usage(settings);
      Ok(UsageSnapshot {
        official: local_official
          .unwrap_or_else(|| serde_json::json!({"available": false, "status": "unknown"})),
        local_buckets: fallback.0,
        recent_sessions: fallback.1,
        parser_warnings: vec![format!("rpc unavailable: {err}")],
        last_updated_at: now,
      })
    }
  }
}

async fn read_rpc_usage() -> Result<serde_json::Value, String> {
  let mut child = Command::new("codex")
    .arg("app-server")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .map_err(|e| format!("spawn failed: {e}"))?;
  let mut stdin = child.stdin.take().ok_or("stdin unavailable")?;
  let stdout = child.stdout.take().ok_or("stdout unavailable")?;
  let mut lines = BufReader::new(stdout).lines();
  write_rpc_line(
    &mut stdin,
    &serde_json::json!({
      "jsonrpc":"2.0",
      "id":1,
      "method":"initialize",
      "params":{
        "clientInfo":{"name":"codex-usage-monitor","version":"0.1.0"},
        "capabilities":null
      }
    }),
  )
  .await?;
  write_rpc_line(
    &mut stdin,
    &serde_json::json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
  )
  .await?;
  write_rpc_line(
    &mut stdin,
    &serde_json::json!({"jsonrpc":"2.0","id":2,"method":"account/read","params":{}}),
  )
  .await?;
  write_rpc_line(
    &mut stdin,
    &serde_json::json!({"jsonrpc":"2.0","id":3,"method":"account/rateLimits/read","params":{}}),
  )
  .await?;

  let mut account: Option<serde_json::Value> = None;
  let mut rate_limits: Option<serde_json::Value> = None;
  for _ in 0..20 {
    let line = timeout(TokioDuration::from_secs(3), lines.next_line())
      .await
      .map_err(|_| "rpc timeout".to_string())?
      .map_err(|e| e.to_string())?
      .ok_or("rpc eof")?;
    let value: serde_json::Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
    match value.get("id").and_then(|id| id.as_i64()) {
      Some(2) => {
        if let Some(err) = value.get("error") {
          return Err(format!("account/read failed: {err}"));
        }
        account = value.get("result").cloned();
      }
      Some(3) => {
        if let Some(err) = value.get("error") {
          return Err(format!("account/rateLimits/read failed: {err}"));
        }
        rate_limits = value.get("result").cloned();
        break;
      }
      _ => {}
    }
  }
  let _ = child.kill().await;
  let rate_limits = rate_limits.ok_or("rate limit response not received")?;

  let used = read_number(
    &rate_limits,
    &[
      "/rateLimits/primary/usedPercent",
      "/rateLimits/primary/used_percent",
      "/rateLimitsByLimitId/codex/primary/usedPercent",
      "/rateLimitsByLimitId/codex/primary/used_percent",
    ],
  )
  .unwrap_or(0.0);
  let window_minutes = read_u64(
    &rate_limits,
    &[
      "/rateLimits/primary/windowDurationMins",
      "/rateLimits/primary/window_minutes",
      "/rateLimitsByLimitId/codex/primary/windowDurationMins",
      "/rateLimitsByLimitId/codex/primary/window_minutes",
    ],
  );
  let resets_epoch = read_i64(
    &rate_limits,
    &[
      "/rateLimits/primary/resetsAt",
      "/rateLimits/primary/resets_at",
      "/rateLimitsByLimitId/codex/primary/resetsAt",
      "/rateLimitsByLimitId/codex/primary/resets_at",
    ],
  );
  let resets_at = resets_epoch.map(|epoch| {
    chrono::DateTime::<Utc>::from_timestamp(epoch, 0)
      .map(|ts| ts.to_rfc3339())
      .unwrap_or_else(|| Utc::now().to_rfc3339())
  });
  let plan_type = account
    .as_ref()
    .and_then(|a| a.pointer("/planType").and_then(|v| v.as_str()))
    .or_else(|| {
      rate_limits
        .pointer("/rateLimits/planType")
        .and_then(|v| v.as_str())
    });
  let rate_limit_reached_type = rate_limits
    .pointer("/rateLimits/rateLimitReachedType")
    .and_then(|v| v.as_str());
  let status = if used >= 100.0 {
    "exhausted"
  } else if used >= 90.0 {
    "critical"
  } else if used >= 75.0 {
    "warning"
  } else {
    "ok"
  };
  let windows = build_official_windows_from_rate_limits(&rate_limits);
  Ok(serde_json::json!({
    "available": true,
    "usedPercent": used,
    "remainingPercent": (100.0 - used).max(0.0),
    "windowMinutes": window_minutes,
    "resetsAt": resets_at,
    "planType": plan_type,
    "rateLimitReachedType": rate_limit_reached_type,
    "windows": windows,
    "status": status
  }))
}

fn read_local_usage(settings: &AppSettings) -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
  let base = default_codex_home(&settings.codex_home_override).join("sessions");
  let mut sessions: BTreeMap<String, LocalSession> = BTreeMap::new();
  if base.exists() {
    let _ = visit_files(&base, &mut |path| {
      if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
        return;
      }
      if let Ok(content) = fs::read_to_string(path) {
        let key = path.to_string_lossy().to_string();
        let session = sessions.entry(key.clone()).or_insert_with(|| LocalSession {
          id: key.clone(),
          date: extract_date_from_path(path),
          ..LocalSession::default()
        });
        for line in content.lines() {
          if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            let item_type = json.get("type").and_then(|v| v.as_str()).unwrap_or_default();
            if item_type == "session_meta" {
              let payload = json.pointer("/payload").unwrap_or(&serde_json::Value::Null);
              session.id = payload
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or(&session.id)
                .to_string();
              session.cwd = payload.get("cwd").and_then(|v| v.as_str()).map(|s| s.to_string());
              session.model = payload
                .get("model")
                .or_else(|| payload.get("model_slug"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
              session.title = payload
                .get("title")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
              session.started_at = payload
                .get("timestamp")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| json.get("timestamp").and_then(|v| v.as_str()).map(|s| s.to_string()));
            } else if item_type == "event_msg"
              && json.pointer("/payload/type").and_then(|v| v.as_str()) == Some("token_count")
            {
              let usage = json.pointer("/payload/info/total_token_usage");
              let total = usage
                .and_then(|u| u.get("total_tokens").and_then(|v| v.as_u64()))
                .or_else(|| usage.and_then(|u| u.get("totalTokens").and_then(|v| v.as_u64())))
                .unwrap_or(0);
              let input = usage
                .and_then(|u| u.get("input_tokens").and_then(|v| v.as_u64()))
                .or_else(|| usage.and_then(|u| u.get("inputTokens").and_then(|v| v.as_u64())))
                .unwrap_or(0);
              let cached = usage
                .and_then(|u| u.get("cached_input_tokens").and_then(|v| v.as_u64()))
                .or_else(|| usage.and_then(|u| u.get("cachedInputTokens").and_then(|v| v.as_u64())))
                .unwrap_or(0);
              let output = usage
                .and_then(|u| u.get("output_tokens").and_then(|v| v.as_u64()))
                .or_else(|| usage.and_then(|u| u.get("outputTokens").and_then(|v| v.as_u64())))
                .unwrap_or(0);
              let reasoning = usage
                .and_then(|u| u.get("reasoning_output_tokens").and_then(|v| v.as_u64()))
                .or_else(|| usage.and_then(|u| u.get("reasoningOutputTokens").and_then(|v| v.as_u64())))
                .unwrap_or(0);

              session.total_tokens = session.total_tokens.max(total);
              session.input_tokens = session.input_tokens.max(input);
              session.cached_input_tokens = session.cached_input_tokens.max(cached);
              session.output_tokens = session.output_tokens.max(output);
              session.reasoning_output_tokens = session.reasoning_output_tokens.max(reasoning);
              session.updated_at = json
                .get("timestamp")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| session.updated_at.clone());
            }
          }
        }
      }
    });
  }

  let now = Local::now();
  let today = now.date_naive();
  let week_start = today - Duration::days(6);
  let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
  let mut day_total = 0_u64;
  let mut week_total = 0_u64;
  let mut month_total = 0_u64;
  let mut day_tokens = TokenParts::default();
  let mut week_tokens = TokenParts::default();
  let mut month_tokens = TokenParts::default();
  let mut day_count = 0_usize;
  let mut week_count = 0_usize;
  let mut month_count = 0_usize;
  for session in sessions.values() {
    if let Some(date) = session.date {
      if date == today {
        day_total += session.total_tokens;
        day_tokens.add(session);
        day_count += 1;
      }
      if date >= week_start && date <= today {
        week_total += session.total_tokens;
        week_tokens.add(session);
        week_count += 1;
      }
      if date >= month_start && date <= today {
        month_total += session.total_tokens;
        month_tokens.add(session);
        month_count += 1;
      }
    }
  }
  let buckets = vec![
    serde_json::json!({"label":"today","start": today.to_string(),"end": today.to_string(),"totalTokens":day_total,"inputTokens":day_tokens.input_tokens,"cachedInputTokens":day_tokens.cached_input_tokens,"outputTokens":day_tokens.output_tokens,"reasoningOutputTokens":day_tokens.reasoning_output_tokens,"sessionCount": day_count}),
    serde_json::json!({"label":"thisWeek","start": week_start.to_string(),"end": today.to_string(),"totalTokens":week_total,"inputTokens":week_tokens.input_tokens,"cachedInputTokens":week_tokens.cached_input_tokens,"outputTokens":week_tokens.output_tokens,"reasoningOutputTokens":week_tokens.reasoning_output_tokens,"sessionCount": week_count}),
    serde_json::json!({"label":"thisMonth","start": month_start.to_string(),"end": today.to_string(),"totalTokens":month_total,"inputTokens":month_tokens.input_tokens,"cachedInputTokens":month_tokens.cached_input_tokens,"outputTokens":month_tokens.output_tokens,"reasoningOutputTokens":month_tokens.reasoning_output_tokens,"sessionCount": month_count})
  ];
  let recents = sessions
    .iter()
    .rev()
    .take(10)
    .map(|(_, s)| {
      serde_json::json!({
        "id": s.id,
        "title": s.title,
        "cwd": s.cwd,
        "model": s.model,
        "startedAt": s.started_at.clone().unwrap_or_else(|| Utc::now().to_rfc3339()),
        "updatedAt": s.updated_at.clone().unwrap_or_else(|| s.started_at.clone().unwrap_or_else(|| Utc::now().to_rfc3339())),
        "totalTokens": s.total_tokens
      })
    })
    .collect::<Vec<_>>();
  (buckets, recents)
}

fn read_local_official_usage(settings: &AppSettings) -> Option<serde_json::Value> {
  let base = default_codex_home(&settings.codex_home_override).join("sessions");
  if !base.exists() {
    return None;
  }
  let mut latest: Option<(String, serde_json::Value)> = None;
  let _ = visit_files(&base, &mut |path| {
    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
      return;
    }
    if let Ok(content) = fs::read_to_string(path) {
      for line in content.lines() {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
          continue;
        };
        let is_token = json.get("type").and_then(|v| v.as_str()) == Some("event_msg")
          && json.pointer("/payload/type").and_then(|v| v.as_str()) == Some("token_count");
        if !is_token {
          continue;
        }
        let Some(rate) = json.pointer("/payload/rate_limits").cloned() else {
          continue;
        };
        let ts = json
          .get("timestamp")
          .and_then(|v| v.as_str())
          .unwrap_or_default()
          .to_string();
        match &latest {
          Some((prev_ts, _)) if prev_ts >= &ts => {}
          _ => latest = Some((ts, rate)),
        }
      }
    }
  });

  let (_, rate) = latest?;
  let used = read_number(
    &rate,
    &["/primary/used_percent", "/primary/usedPercent", "/secondary/used_percent"],
  )?;
  let window_minutes = read_u64(
    &rate,
    &[
      "/primary/window_minutes",
      "/primary/windowDurationMins",
      "/secondary/window_minutes",
    ],
  );
  let resets_epoch = read_i64(
    &rate,
    &["/primary/resets_at", "/primary/resetsAt", "/secondary/resets_at"],
  );
  let resets_at = resets_epoch.and_then(|epoch| chrono::DateTime::<Utc>::from_timestamp(epoch, 0));
  let status = if used >= 100.0 {
    "exhausted"
  } else if used >= 90.0 {
    "critical"
  } else if used >= 75.0 {
    "warning"
  } else {
    "ok"
  };
  let windows = build_official_windows_from_rate_limits(&serde_json::json!({
    "primary": rate.get("primary").cloned(),
    "secondary": rate.get("secondary").cloned()
  }));

  Some(serde_json::json!({
    "available": true,
    "limitKind": "weekly",
    "usedPercent": used,
    "remainingPercent": (100.0 - used).max(0.0),
    "windowMinutes": window_minutes,
    "resetsAt": resets_at.map(|v| v.to_rfc3339()),
    "planType": rate.get("plan_type").and_then(|v| v.as_str()),
    "rateLimitReachedType": rate.get("rate_limit_reached_type").and_then(|v| v.as_str()),
    "windows": windows,
    "status": status
  }))
}

fn build_official_windows_from_rate_limits(rate_limits: &serde_json::Value) -> Vec<serde_json::Value> {
  let mut windows = Vec::new();
  if let Some(window) = read_window(rate_limits, "primary") {
    windows.push(window);
  }
  if let Some(window) = read_window(rate_limits, "secondary") {
    windows.push(window);
  }
  windows
}

fn read_window(rate_limits: &serde_json::Value, key: &str) -> Option<serde_json::Value> {
  let used = read_number(
    rate_limits,
    &[
      &format!("/{key}/usedPercent"),
      &format!("/{key}/used_percent"),
      &format!("/rateLimits/{key}/usedPercent"),
      &format!("/rateLimits/{key}/used_percent"),
      &format!("/rateLimitsByLimitId/codex/{key}/usedPercent"),
      &format!("/rateLimitsByLimitId/codex/{key}/used_percent"),
    ],
  )?;
  let window_minutes = read_u64(
    rate_limits,
    &[
      &format!("/{key}/windowDurationMins"),
      &format!("/{key}/window_minutes"),
      &format!("/rateLimits/{key}/windowDurationMins"),
      &format!("/rateLimits/{key}/window_minutes"),
      &format!("/rateLimitsByLimitId/codex/{key}/windowDurationMins"),
      &format!("/rateLimitsByLimitId/codex/{key}/window_minutes"),
    ],
  );
  let resets_epoch = read_i64(
    rate_limits,
    &[
      &format!("/{key}/resetsAt"),
      &format!("/{key}/resets_at"),
      &format!("/rateLimits/{key}/resetsAt"),
      &format!("/rateLimits/{key}/resets_at"),
      &format!("/rateLimitsByLimitId/codex/{key}/resetsAt"),
      &format!("/rateLimitsByLimitId/codex/{key}/resets_at"),
    ],
  );
  let resets_at = resets_epoch.and_then(|epoch| chrono::DateTime::<Utc>::from_timestamp(epoch, 0));
  let label = match window_minutes {
    Some(300) => "5 hour usage limit",
    Some(10080) => "Weekly usage limit",
    _ if key == "primary" => "Primary usage limit",
    _ => "Secondary usage limit",
  };
  Some(serde_json::json!({
    "label": label,
    "usedPercent": used,
    "remainingPercent": (100.0 - used).max(0.0),
    "windowMinutes": window_minutes,
    "resetsAt": resets_at.map(|v| v.to_rfc3339())
  }))
}

#[derive(Default)]
struct LocalSession {
  id: String,
  title: Option<String>,
  cwd: Option<String>,
  model: Option<String>,
  started_at: Option<String>,
  updated_at: Option<String>,
  total_tokens: u64,
  input_tokens: u64,
  cached_input_tokens: u64,
  output_tokens: u64,
  reasoning_output_tokens: u64,
  date: Option<NaiveDate>,
}

#[derive(Default)]
struct TokenParts {
  input_tokens: u64,
  cached_input_tokens: u64,
  output_tokens: u64,
  reasoning_output_tokens: u64,
}

impl TokenParts {
  fn add(&mut self, s: &LocalSession) {
    self.input_tokens += s.input_tokens;
    self.cached_input_tokens += s.cached_input_tokens;
    self.output_tokens += s.output_tokens;
    self.reasoning_output_tokens += s.reasoning_output_tokens;
  }
}

fn extract_date_from_path(path: &Path) -> Option<NaiveDate> {
  let parts = path
    .iter()
    .filter_map(|p| p.to_str())
    .collect::<Vec<_>>();
  for idx in 0..parts.len().saturating_sub(2) {
    let y = parts[idx].parse::<i32>().ok();
    let m = parts[idx + 1].parse::<u32>().ok();
    let d = parts[idx + 2].parse::<u32>().ok();
    if let (Some(y), Some(m), Some(d)) = (y, m, d) {
      if (2000..=2100).contains(&y) {
        if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
          return Some(date);
        }
      }
    }
  }
  None
}

fn read_number(value: &serde_json::Value, paths: &[&str]) -> Option<f64> {
  for path in paths {
    if let Some(n) = value.pointer(path).and_then(|v| v.as_f64()) {
      return Some(n);
    }
  }
  None
}

fn read_u64(value: &serde_json::Value, paths: &[&str]) -> Option<u64> {
  for path in paths {
    if let Some(n) = value.pointer(path).and_then(|v| v.as_u64()) {
      return Some(n);
    }
  }
  None
}

fn read_i64(value: &serde_json::Value, paths: &[&str]) -> Option<i64> {
  for path in paths {
    if let Some(n) = value.pointer(path).and_then(|v| v.as_i64()) {
      return Some(n);
    }
  }
  None
}

async fn write_rpc_line(
  stdin: &mut tokio::process::ChildStdin,
  value: &serde_json::Value,
) -> Result<(), String> {
  let line = serde_json::to_string(value).map_err(|e| e.to_string())?;
  stdin
    .write_all(format!("{line}\n").as_bytes())
    .await
    .map_err(|e| e.to_string())
}

fn visit_files(root: &Path, handler: &mut dyn FnMut(&Path)) -> std::io::Result<()> {
  for entry in fs::read_dir(root)? {
    let path = entry?.path();
    if path.is_dir() {
      visit_files(&path, handler)?;
    } else {
      handler(&path);
    }
  }
  Ok(())
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
  let open = MenuItem::with_id(app, "open", "Open Usage Monitor", true, None::<&str>)?;
  let refresh = MenuItem::with_id(app, "refresh", "Refresh Now", true, None::<&str>)?;
  let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
  let menu = Menu::with_items(app, &[&open, &refresh, &quit])?;
  let _tray = TrayIconBuilder::with_id("main-tray")
    .menu(&menu)
    .on_menu_event(|app, event| match event.id.as_ref() {
      "open" => {
        let _ = show_main_window(app.clone());
      }
      "refresh" => {
        let _ = app.emit("refresh-requested", ());
      }
      "quit" => app.exit(0),
      _ => {}
    })
    .on_tray_icon_event(|tray, event| {
      if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
      } = event
      {
        let _ = show_main_window(tray.app_handle().clone());
      }
    })
    .build(app)?;
  Ok(())
}

fn main() {
  tauri::Builder::default()
    .plugin(tauri_plugin_shell::init())
    .plugin(tauri_plugin_store::Builder::default().build())
    .plugin(tauri_plugin_notification::init())
    .plugin(tauri_plugin_single_instance::init(|app, _, _| {
      let _ = show_main_window(app.clone());
    }))
    .plugin(tauri_plugin_autostart::init(
      tauri_plugin_autostart::MacosLauncher::LaunchAgent,
      None,
    ))
    .manage(AppState::default())
    .setup(|app| {
      build_tray(&app.handle())?;
      if let Some(win) = app.get_webview_window("main") {
        let app_handle = app.handle().clone();
        win.on_window_event(move |event| {
          if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = hide_main_window(app_handle.clone());
          }
        });
      }
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      get_app_status,
      get_usage_snapshot,
      refresh_usage,
      get_settings,
      update_settings,
      set_start_on_login,
      show_main_window,
      hide_main_window,
      quit_app
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn threshold_detection_works() {
    assert_eq!(threshold_crossings(49.0, 76.0, &[50, 75, 90, 100]), vec![50, 75]);
  }

  #[test]
  fn dedupe_key_is_stable() {
    assert_eq!(dedupe_key("weekly", "2026-05-20T00:00:00Z"), "weekly:2026-05-20T00:00:00Z");
  }

  #[test]
  fn token_count_line_parsing() {
    let line = r#"{"type":"token_count","totalTokens":1234}"#;
    let json: serde_json::Value = serde_json::from_str(line).expect("valid");
    assert_eq!(json.get("totalTokens").and_then(|v| v.as_u64()), Some(1234));
  }
}
