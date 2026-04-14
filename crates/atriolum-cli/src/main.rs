use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use comfy_table::{presets::UTF8_FULL, Cell, Color as TableColor, Table};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::io::{self, BufRead, Write};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Parser)]
#[command(name = "atriolum-cli", about = "CLI client for Atriolum error tracking server")]
struct Cli {
    /// Atriolum server URL
    #[arg(long, default_value = "http://localhost:8000")]
    server: String,

    /// Output format
    #[arg(long, default_value = "table")]
    format: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// List recent events
    Events {
        #[command(subcommand)]
        action: EventCommands,
    },
    /// List projects
    Projects {
        #[command(subcommand)]
        action: ProjectCommands,
    },
    /// Show project statistics
    Stats {
        /// Project ID (optional, shows all if omitted)
        #[arg(long)]
        project: Option<String>,
    },
    /// List releases
    Releases {
        /// Project ID
        project: String,
    },
    /// List transactions
    Transactions {
        /// Project ID
        project: String,
        /// Max results
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Text search
        #[arg(long)]
        query: Option<String>,
    },
    /// Live tail events via WebSocket
    Tail {
        /// Project ID (optional, tails all if omitted)
        #[arg(long)]
        project: Option<String>,
    },
    /// Test connection
    Ping,
}

#[derive(Subcommand)]
enum EventCommands {
    /// List recent events
    List {
        /// Project ID
        #[arg(long)]
        project: Option<String>,
        /// Level filter (fatal/error/warning/info/debug)
        #[arg(long)]
        level: Option<String>,
        /// Platform filter
        #[arg(long)]
        platform: Option<String>,
        /// Text search
        #[arg(long)]
        query: Option<String>,
        /// Environment filter
        #[arg(long)]
        environment: Option<String>,
        /// Max events to return
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Show event detail
    Show {
        /// Event ID
        event_id: String,
        /// Project ID
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// List all projects
    List,
    /// Create a new project
    Create {
        /// Project name
        name: String,
        /// Public key (auto-generated if not provided)
        #[arg(long)]
        public_key: Option<String>,
        /// Project ID (auto-generated if not provided)
        #[arg(long)]
        id: Option<String>,
    },
    /// Delete a project
    Delete {
        /// Project ID
        project_id: String,
    },
    /// Show project details
    Show {
        /// Project ID
        project_id: String,
    },
}

// ---- HTTP helpers ----

/// Make a GET request to the server.
async fn http_get(server: &str, path: &str) -> Result<Value> {
    let url = format!("{server}{path}");
    let resp = reqwest::get(&url).await?;
    let status = resp.status();
    let body: Value = resp.json().await?;
    if !status.is_success() {
        let detail = body["detail"].as_str().unwrap_or("unknown error");
        anyhow::bail!("HTTP {}: {detail}", status);
    }
    Ok(body)
}

/// Make a POST request to the server.
async fn http_post(server: &str, path: &str, json: &Value) -> Result<Value> {
    let url = format!("{server}{path}");
    let resp = reqwest::Client::new()
        .post(&url)
        .json(json)
        .send()
        .await?;
    let status = resp.status();
    let body: Value = resp.json().await?;
    if !status.is_success() {
        let detail = body["detail"].as_str().unwrap_or("unknown error");
        anyhow::bail!("HTTP {}: {detail}", status);
    }
    Ok(body)
}

/// Make a DELETE request to the server.
async fn http_delete(server: &str, path: &str) -> Result<Value> {
    let url = format!("{server}{path}");
    let resp = reqwest::Client::new()
        .delete(&url)
        .send()
        .await?;
    let status = resp.status();
    let body: Value = resp.json().await?;
    if !status.is_success() {
        let detail = body["detail"].as_str().unwrap_or("unknown error");
        anyhow::bail!("HTTP {}: {detail}", status);
    }
    Ok(body)
}

// ---- Output formatters ----

fn print_events_table(events: &[Value], format: &str) {
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(events).unwrap());
        return;
    }

    if events.is_empty() {
        println!("No events found.");
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("Event ID").fg(TableColor::Cyan),
        Cell::new("Level").fg(TableColor::Cyan),
        Cell::new("Platform").fg(TableColor::Cyan),
        Cell::new("Message").fg(TableColor::Cyan),
        Cell::new("Timestamp").fg(TableColor::Cyan),
    ]);

    for event in events {
        let id = event["event_id"].as_str().unwrap_or("-");
        let level = event["level"].as_str().unwrap_or("-");
        let platform = event["platform"].as_str().unwrap_or("-");
        let message = event["message"].as_str().unwrap_or("-");
        let timestamp = event["timestamp"].as_str().unwrap_or("-");

        let level_color = match level {
            "fatal" => TableColor::Red,
            "error" => TableColor::Red,
            "warning" => TableColor::Yellow,
            "info" => TableColor::Blue,
            "debug" => TableColor::DarkGrey,
            _ => TableColor::White,
        };

        table.add_row(vec![
            Cell::new(&id[..16.min(id.len())]),
            Cell::new(level).fg(level_color),
            Cell::new(platform),
            Cell::new(&message[..80.min(message.len())]),
            Cell::new(timestamp),
        ]);
    }

    println!("{table}");
    println!("{} event(s)", events.len().to_string().dimmed());
}

fn print_event_detail(event: &Value) {
    let id = event["event_id"].as_str().unwrap_or("unknown");
    let level = event["level"].as_str().unwrap_or("-");
    let platform = event["platform"].as_str().unwrap_or("-");
    let timestamp = event["timestamp"].as_str().unwrap_or("-");
    let message = event["message"].as_str().unwrap_or("");
    let environment = event["environment"].as_str().unwrap_or("");
    let release = event["release"].as_str().unwrap_or("");

    println!(
        "{} {}",
        "Event:".bold(),
        id.to_string().cyan()
    );
    println!("  {} {}", "Level:".dimmed(), level_colored(level));
    println!("  {} {}", "Platform:".dimmed(), platform);
    println!("  {} {}", "Timestamp:".dimmed(), timestamp);
    if !environment.is_empty() {
        println!("  {} {}", "Environment:".dimmed(), environment);
    }
    if !release.is_empty() {
        println!("  {} {}", "Release:".dimmed(), release);
    }

    // Message
    if !message.is_empty() {
        println!();
        println!("{}", "Message:".bold());
        println!("  {message}");
    }

    // Exception
    if let Some(exception) = event.get("exception") {
        if let Some(values) = exception["values"].as_array() {
            println!();
            println!("{}", "Exceptions:".bold());
            for (i, exc) in values.iter().enumerate() {
                let exc_type = exc["type"].as_str().unwrap_or("Unknown");
                let exc_value = exc["value"].as_str().unwrap_or("");
                println!(
                    "  {} {}: {}",
                    format!("[{}]", i).dimmed(),
                    exc_type.red(),
                    exc_value
                );
                // Stacktrace
                if let Some(frames) = exc["stacktrace"]["frames"].as_array() {
                    let show_frames: Vec<_> = frames
                        .iter()
                        .rev()
                        .take(8)
                        .collect();
                    for frame in &show_frames {
                        let filename = frame["filename"].as_str().unwrap_or("?");
                        let func = frame["function"].as_str().unwrap_or("?");
                        let line = frame["lineno"].as_u64().map(|l| l.to_string()).unwrap_or("?".to_string());
                        let context = frame["context_line"].as_str().unwrap_or("");
                        println!(
                            "    {} {}:{} {}",
                            "→".dimmed(),
                            filename.dimmed(),
                            line,
                            func
                        );
                        if !context.is_empty() {
                            println!("      {}", context.trim().yellow());
                        }
                    }
                    if frames.len() > 8 {
                        println!("    {} {} more frames...", "...".dimmed(), frames.len() - 8);
                    }
                }
            }
        }
    }

    // Tags
    if let Some(tags) = event.get("tags") {
        if let Some(obj) = tags.as_object() {
            if !obj.is_empty() {
                println!();
                println!("{}", "Tags:".bold());
                for (k, v) in obj {
                    println!("  {} {}", format!("{k}:").dimmed(), v);
                }
            }
        }
    }

    // User
    if let Some(user) = event.get("user") {
        if user.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
            println!();
            println!("{}", "User:".bold());
            if let Some(id) = user["id"].as_str() {
                println!("  {} {}", "ID:".dimmed(), id);
            }
            if let Some(email) = user["email"].as_str() {
                println!("  {} {}", "Email:".dimmed(), email);
            }
            if let Some(username) = user["username"].as_str() {
                println!("  {} {}", "Username:".dimmed(), username);
            }
        }
    }

    // Extra
    if let Some(extra) = event.get("extra") {
        if let Some(obj) = extra.as_object() {
            if !obj.is_empty() {
                println!();
                println!("{}", "Extra:".bold());
                let json = serde_json::to_string_pretty(extra).unwrap_or_default();
                for line in json.lines().take(10) {
                    println!("  {line}");
                }
                if json.lines().count() > 10 {
                    println!("  {} more...", "...".dimmed());
                }
            }
        }
    }
}

fn print_projects_table(projects: &[Value], format: &str) {
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(projects).unwrap());
        return;
    }

    if projects.is_empty() {
        println!("No projects found.");
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("ID").fg(TableColor::Cyan),
        Cell::new("Name").fg(TableColor::Cyan),
        Cell::new("DSN").fg(TableColor::Cyan),
    ]);

    for project in projects {
        let id = project["project_id"].as_str().unwrap_or("-");
        let name = project["project_name"].as_str().unwrap_or("-");
        let keys = project["keys"].as_array();
        let dsn = keys
            .and_then(|k| k.first())
            .and_then(|k| k["public_key"].as_str())
            .unwrap_or("-");
        table.add_row(vec![Cell::new(id), Cell::new(name), Cell::new(dsn)]);
    }

    println!("{table}");
}

fn level_colored(level: &str) -> String {
    match level {
        "fatal" => level.red().bold().to_string(),
        "error" => level.red().to_string(),
        "warning" => level.yellow().to_string(),
        "info" => level.blue().to_string(),
        "debug" => level.dimmed().to_string(),
        _ => level.to_string(),
    }
}

// ---- Command handlers ----

async fn cmd_events_list(
    server: &str,
    format: &str,
    project: Option<&str>,
    level: Option<&str>,
    platform: Option<&str>,
    query: Option<&str>,
    environment: Option<&str>,
    limit: usize,
) -> Result<()> {
    let project = project.unwrap_or("1");
    let mut params = vec![format!("limit={limit}")];
    if let Some(l) = level {
        params.push(format!("level={l}"));
    }
    if let Some(p) = platform {
        params.push(format!("platform={p}"));
    }
    if let Some(q) = query {
        params.push(format!("query={}", urlencoding(q)));
    }
    if let Some(e) = environment {
        params.push(format!("environment={e}"));
    }
    let qs = params.join("&");
    let body = http_get(server, &format!("/api/projects/{project}/events/?{qs}")).await?;
    let events = body.as_array().cloned().unwrap_or_default();
    print_events_table(&events, format);
    Ok(())
}

async fn cmd_events_show(server: &str, event_id: &str, project: Option<&str>) -> Result<()> {
    let project = project.unwrap_or("1");
    let body = http_get(server, &format!("/api/projects/{project}/events/{event_id}/")).await?;
    print_event_detail(&body);
    Ok(())
}

async fn cmd_projects_list(server: &str, format: &str) -> Result<()> {
    let body = http_get(server, "/api/projects/").await?;
    let projects = body.as_array().cloned().unwrap_or_default();
    print_projects_table(&projects, format);
    Ok(())
}

async fn cmd_projects_create(
    server: &str,
    name: &str,
    public_key: Option<&str>,
    id: Option<&str>,
) -> Result<()> {
    let mut body = serde_json::json!({"name": name});
    if let Some(pk) = public_key {
        body["public_key"] = Value::String(pk.to_string());
    }
    if let Some(pid) = id {
        body["project_id"] = Value::String(pid.to_string());
    }
    let result = http_post(server, "/api/projects/", &body).await?;
    let pid = result["project_id"].as_str().unwrap_or("?");
    let pk = result["keys"][0]["public_key"].as_str().unwrap_or("?");
    println!("{} Project created", "✓".green());
    println!("  {} {}", "ID:".dimmed(), pid);
    println!("  {} {}", "Name:".dimmed(), name);
    println!("  {} {}", "Key:".dimmed(), pk);
    println!(
        "  {} {}",
        "DSN:".dimmed(),
        format!("http://{pk}@localhost:8000/{pid}").cyan()
    );
    Ok(())
}

async fn cmd_projects_delete(server: &str, project_id: &str) -> Result<()> {
    http_delete(server, &format!("/api/projects/{project_id}/")).await?;
    println!("{} Project {} deleted", "✓".green(), project_id);
    Ok(())
}

async fn cmd_projects_show(server: &str, project_id: &str) -> Result<()> {
    let body = http_get(server, &format!("/api/projects/{project_id}/")).await?;
    println!("{}", serde_json::to_string_pretty(&body)?);
    Ok(())
}

async fn cmd_stats(server: &str, project: Option<&str>) -> Result<()> {
    let project = project.unwrap_or("1");
    let body = http_get(server, &format!("/api/projects/{project}/stats/")).await?;

    println!("{} Project {}", "Stats:".bold(), project);
    println!(
        "  {} {} events",
        "Events:".dimmed(),
        body["total_events"].as_u64().unwrap_or(0).to_string().cyan()
    );
    println!(
        "  {} {} transactions",
        "Transactions:".dimmed(),
        body["total_transactions"].as_u64().unwrap_or(0).to_string().cyan()
    );
    println!(
        "  {} {} sessions",
        "Sessions:".dimmed(),
        body["total_sessions"].as_u64().unwrap_or(0).to_string().cyan()
    );

    if let Some(levels) = body["events_by_level"].as_object() {
        println!("  {}", "By level:".dimmed());
        for (level, count) in levels {
            let c = count.as_u64().unwrap_or(0);
            println!("    {} {}", level_colored(level), c);
        }
    }

    if let Some(last) = body["last_event_at"].as_str() {
        println!("  {} {}", "Last event:".dimmed(), last);
    }

    Ok(())
}

async fn cmd_releases(server: &str, project: &str) -> Result<()> {
    let body = http_get(server, &format!("/api/projects/{project}/releases/")).await?;
    let releases = body.as_array().cloned().unwrap_or_default();

    if releases.is_empty() {
        println!("No releases found.");
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("Release").fg(TableColor::Cyan),
        Cell::new("Events").fg(TableColor::Cyan),
        Cell::new("Environment").fg(TableColor::Cyan),
        Cell::new("Last Seen").fg(TableColor::Cyan),
    ]);

    for rel in &releases {
        table.add_row(vec![
            Cell::new(rel["release"].as_str().unwrap_or("-")),
            Cell::new(rel["event_count"].as_u64().unwrap_or(0)),
            Cell::new(rel["environment"].as_str().unwrap_or("-")),
            Cell::new(rel["last_seen"].as_str().unwrap_or("-")),
        ]);
    }
    println!("{table}");
    Ok(())
}

async fn cmd_transactions(server: &str, project: &str, limit: usize, query: Option<&str>) -> Result<()> {
    let mut params = vec![format!("limit={limit}")];
    if let Some(q) = query {
        params.push(format!("query={}", urlencoding(q)));
    }
    let qs = params.join("&");
    let body = http_get(server, &format!("/api/projects/{project}/transactions/?{qs}")).await?;
    let txs = body.as_array().cloned().unwrap_or_default();
    print_events_table(&txs, "table");
    Ok(())
}

async fn cmd_tail(server: &str, _project: Option<&str>) -> Result<()> {
    let ws_url = server.replace("http://", "ws://").replace("https://", "wss://");
    let url = format!("{ws_url}/ws/cli");

    println!("{} Connecting to {}...", "tail".bold(), ws_url.dimmed());
    let (mut ws_stream, _) = connect_async(&url).await?;
    println!("{} Listening for events (Ctrl+C to stop)...", "tail".bold());
    println!();

    // Subscribe to event stream
    let subscribe = serde_json::json!({"type": "tail_subscribe"});
    ws_stream
        .send(Message::Text(subscribe.to_string().into()))
        .await?;

    while let Some(msg) = ws_stream.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(event) = serde_json::from_str::<Value>(&text) {
                    let msg_type = event["type"].as_str().unwrap_or("");
                    if msg_type == "event" {
                        let level = event["level"].as_str().unwrap_or("-");
                        let platform = event["platform"].as_str().unwrap_or("-");
                        let message = event["message"].as_str().unwrap_or("");
                        let event_id = event["event_id"].as_str().unwrap_or("?");
                        println!(
                            "{} {} {} {} {}",
                            chrono::Utc::now().format("%H:%M:%S").to_string().dimmed(),
                            level_colored(level),
                            platform.dimmed(),
                            &event_id[..16.min(event_id.len())].cyan(),
                            &message[..80.min(message.len())]
                        );
                    }
                }
            }
            Ok(Message::Close(_)) => {
                println!("{} Connection closed", "!".yellow());
                break;
            }
            Err(e) => {
                eprintln!("{} WebSocket error: {e}", "!".red());
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn cmd_ping(server: &str) -> Result<()> {
    let start = std::time::Instant::now();
    let body = http_get(server, "/api/health").await?;
    let elapsed = start.elapsed();
    let status = body["status"].as_str().unwrap_or("?");
    println!(
        "{} server={} time={}ms",
        "pong".green(),
        status,
        elapsed.as_millis()
    );
    Ok(())
}

/// Simple URL percent-encoding.
fn urlencoding(s: &str) -> String {
    let mut result = String::new();
    for byte in s.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => {
                result.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    result
}

// ---- REPL ----

async fn run_repl(server: &str) -> Result<()> {
    println!(
        "{} {}",
        "atriolum-cli".bold().green(),
        format!("— {}", server).dimmed()
    );
    println!();
    println!(
        "{}",
        "Type 'help' for commands, 'exit' to quit.".dimmed()
    );

    // Test connection first
    match http_get(server, "/api/health").await {
        Ok(_) => println!("{}", "Connected ✓".green()),
        Err(e) => {
            eprintln!("{} Cannot connect to {server}: {e}", "!".red());
            println!("{}", "Commands will fail until server is available.".dimmed());
        }
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("{}", "atriolum> ".green().bold());
        stdout.flush()?;

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() {
            break;
        }
        let line = line.trim();

        if line.is_empty() {
            continue;
        }
        if line == "exit" || line == "quit" {
            println!("{}", "Goodbye!".dimmed());
            break;
        }
        if line == "help" {
            print_help();
            continue;
        }

        if let Err(e) = exec_repl_command(server, line).await {
            eprintln!("{} {}", "ERROR:".red(), e);
        }
    }

    Ok(())
}

async fn exec_repl_command(server: &str, line: &str) -> Result<()> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }

    match parts[0] {
        "ping" => cmd_ping(server).await,
        "events" if parts.len() > 1 => match parts[1] {
            "list" => {
                let mut project = None;
                let mut level = None;
                let mut platform = None;
                let mut query = None;
                let mut environment = None;
                let mut limit = 20usize;
                let mut i = 2;
                while i < parts.len() {
                    match parts[i] {
                        "--project" | "-p" if i + 1 < parts.len() => {
                            project = Some(parts[i + 1]);
                            i += 2;
                        }
                        "--level" | "-l" if i + 1 < parts.len() => {
                            level = Some(parts[i + 1]);
                            i += 2;
                        }
                        "--platform" if i + 1 < parts.len() => {
                            platform = Some(parts[i + 1]);
                            i += 2;
                        }
                        "--query" | "-q" if i + 1 < parts.len() => {
                            query = Some(parts[i + 1]);
                            i += 2;
                        }
                        "--environment" | "-e" if i + 1 < parts.len() => {
                            environment = Some(parts[i + 1]);
                            i += 2;
                        }
                        "--limit" | "-n" if i + 1 < parts.len() => {
                            limit = parts[i + 1].parse().unwrap_or(20);
                            i += 2;
                        }
                        _ => i += 1,
                    }
                }
                cmd_events_list(server, "table", project, level, platform, query, environment, limit).await
            }
            "show" if parts.len() > 2 => {
                let project = parts.get(3).copied();
                cmd_events_show(server, parts[2], project).await
            }
            _ => {
                println!(
                    "{} {}",
                    "Unknown subcommand:".red(),
                    parts[1].yellow()
                );
                Ok(())
            }
        },
        "projects" if parts.len() > 1 => match parts[1] {
            "list" => cmd_projects_list(server, "table").await,
            "create" if parts.len() > 2 => {
                cmd_projects_create(server, parts[2], None, None).await
            }
            "delete" if parts.len() > 2 => {
                cmd_projects_delete(server, parts[2]).await
            }
            "show" if parts.len() > 2 => {
                cmd_projects_show(server, parts[2]).await
            }
            _ => {
                println!("{} {}", "Unknown subcommand:".red(), parts[1].yellow());
                Ok(())
            }
        },
        "stats" => {
            let project = parts.get(1).copied();
            cmd_stats(server, project).await
        }
        "releases" if parts.len() > 1 => {
            cmd_releases(server, parts[1]).await
        }
        "transactions" if parts.len() > 1 => {
            let mut limit = 20usize;
            let mut query = None;
            let mut i = 2;
            while i < parts.len() {
                match parts[i] {
                    "--limit" | "-n" if i + 1 < parts.len() => {
                        limit = parts[i + 1].parse().unwrap_or(20);
                        i += 2;
                    }
                    "--query" | "-q" if i + 1 < parts.len() => {
                        query = Some(parts[i + 1]);
                        i += 2;
                    }
                    _ => i += 1,
                }
            }
            cmd_transactions(server, parts[1], limit, query).await
        }
        "tail" => {
            let project = parts.get(1).copied();
            cmd_tail(server, project).await
        }
        _ => {
            println!(
                "{} {}",
                "Unknown command:".red(),
                parts[0].yellow()
            );
            println!("{}", "Type 'help' for available commands.".dimmed());
            Ok(())
        }
    }
}

fn print_help() {
    println!();
    println!("{}", "Atriolum CLI Commands:".bold());
    println!();
    println!("{}", "  Events:".bold());
    println!(
        "  {} {}",
        "events list".green(),
        "[-p project] [-l level] [-q query] [-e env] [-n limit]".dimmed()
    );
    println!(
        "  {} {}",
        "events show".green(),
        "<event_id> [project]".dimmed()
    );
    println!();
    println!("{}", "  Projects:".bold());
    println!("  {}", "projects list".green());
    println!("  {} {}", "projects create".green(), "<name>".dimmed());
    println!("  {} {}", "projects delete".green(), "<id>".dimmed());
    println!("  {} {}", "projects show".green(), "<id>".dimmed());
    println!();
    println!("{}", "  Other:".bold());
    println!(
        "  {} {}",
        "stats".green(),
        "[project]".dimmed()
    );
    println!("  {} {}", "releases".green(), "<project>".dimmed());
    println!("  {} {}", "transactions".green(), "<project> [-n limit] [-q query]".dimmed());
    println!("  {} {}", "tail".green(), "[project]".dimmed());
    println!("  {}", "ping".green());
    println!();
    println!(
        "  {}",
        "Short flags: -p project, -l level, -q query, -e env, -n limit".dimmed()
    );
    println!();
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(cmd) => {
            // Single command mode
            let result: Result<()> = match cmd {
                Commands::Events { action } => match action {
                    EventCommands::List {
                        project,
                        level,
                        platform,
                        query,
                        environment,
                        limit,
                    } => cmd_events_list(
                        &cli.server,
                        &cli.format,
                        project.as_deref(),
                        level.as_deref(),
                        platform.as_deref(),
                        query.as_deref(),
                        environment.as_deref(),
                        limit,
                    )
                    .await,
                    EventCommands::Show { event_id, project } => {
                        cmd_events_show(&cli.server, &event_id, project.as_deref()).await
                    }
                },
                Commands::Projects { action } => match action {
                    ProjectCommands::List => {
                        cmd_projects_list(&cli.server, &cli.format).await
                    }
                    ProjectCommands::Create {
                        name,
                        public_key,
                        id,
                    } => {
                        cmd_projects_create(
                            &cli.server,
                            &name,
                            public_key.as_deref(),
                            id.as_deref(),
                        )
                        .await
                    }
                    ProjectCommands::Delete { project_id } => {
                        cmd_projects_delete(&cli.server, &project_id).await
                    }
                    ProjectCommands::Show { project_id } => {
                        cmd_projects_show(&cli.server, &project_id).await
                    }
                },
                Commands::Stats { project } => {
                    cmd_stats(&cli.server, project.as_deref()).await
                }
                Commands::Releases { project } => {
                    cmd_releases(&cli.server, &project).await
                }
                Commands::Transactions {
                    project,
                    limit,
                    query,
                } => {
                    cmd_transactions(&cli.server, &project, limit, query.as_deref()).await
                }
                Commands::Tail { project } => {
                    cmd_tail(&cli.server, project.as_deref()).await
                }
                Commands::Ping => cmd_ping(&cli.server).await,
            };
            result?;
        }
        None => {
            // Interactive REPL mode
            run_repl(&cli.server).await?;
        }
    }

    Ok(())
}
