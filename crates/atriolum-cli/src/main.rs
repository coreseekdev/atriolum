use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use comfy_table::{presets::UTF8_FULL, Cell, Color as TableColor, Table};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Parser)]
#[command(name = "atriolum-cli", about = "CLI client for Atriolum error tracking server")]
struct Cli {
    /// Atriolum server WebSocket URL
    #[arg(long, default_value = "ws://localhost:8000/ws/cli")]
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
    EventsList {
        /// Project ID filter
        #[arg(long)]
        project: Option<String>,
        /// Level filter (fatal/error/warning/info/debug)
        #[arg(long)]
        level: Option<String>,
        /// Max events to return
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Show event detail
    EventsShow {
        /// Event ID
        event_id: String,
        /// Project ID
        #[arg(long)]
        project: Option<String>,
    },
    /// List projects
    ProjectsList,
    /// Create a new project
    ProjectsCreate {
        /// Project name
        name: String,
        /// Public key (auto-generated if not provided)
        #[arg(long)]
        public_key: Option<String>,
    },
    /// Test connection
    Ping,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum CliRequest {
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "events_list")]
    EventsList {
        project: Option<String>,
        level: Option<String>,
        limit: Option<usize>,
    },
    #[serde(rename = "events_show")]
    EventsShow {
        event_id: String,
        project: Option<String>,
    },
    #[serde(rename = "projects_list")]
    ProjectsList,
    #[serde(rename = "projects_create")]
    ProjectsCreate {
        name: String,
        public_key: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum CliResponse {
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "ok")]
    Ok { message: String },
    #[serde(rename = "events")]
    Events { data: serde_json::Value },
    #[serde(rename = "event_detail")]
    EventDetail { data: serde_json::Value },
    #[serde(rename = "projects")]
    Projects { data: serde_json::Value },
    #[serde(rename = "project")]
    Project { data: serde_json::Value },
    #[serde(rename = "error")]
    Error { message: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(cmd) => {
            // Single command mode
            let request = match cmd {
                Commands::EventsList { project, level, limit } => CliRequest::EventsList {
                    project,
                    level,
                    limit: Some(limit),
                },
                Commands::EventsShow { event_id, project } => CliRequest::EventsShow {
                    event_id,
                    project,
                },
                Commands::ProjectsList => CliRequest::ProjectsList,
                Commands::ProjectsCreate { name, public_key } => CliRequest::ProjectsCreate {
                    name,
                    public_key: public_key.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                },
                Commands::Ping => CliRequest::Ping,
            };

            let response = send_request(&cli.server, request).await?;
            print_response(&response, &cli.format);
        }
        None => {
            // Interactive REPL mode
            run_repl(&cli.server).await?;
        }
    }

    Ok(())
}

async fn send_request(server: &str, request: CliRequest) -> Result<CliResponse> {
    let (mut ws_stream, _) = connect_async(server).await?;

    let json = serde_json::to_string(&request)?;
    ws_stream.send(Message::Text(json.into())).await?;

    let msg = ws_stream.next().await;
    match msg {
        Some(Ok(Message::Text(text))) => {
            let response: CliResponse = serde_json::from_str(&text)?;
            Ok(response)
        }
        Some(Ok(Message::Close(_))) => {
            Err(anyhow::anyhow!("server closed connection"))
        }
        Some(Err(e)) => Err(anyhow::anyhow!("WebSocket error: {e}")),
        None => Err(anyhow::anyhow!("no response from server")),
        _ => Err(anyhow::anyhow!("unexpected message type")),
    }
}

fn print_response(response: &CliResponse, format: &str) {
    match response {
        CliResponse::Pong => {
            println!("{}", "pong".green());
        }
        CliResponse::Ok { message } => {
            println!("{} {}", "OK:".green(), message);
        }
        CliResponse::Events { data } => {
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(data).unwrap());
            } else {
                print_events_table(data);
            }
        }
        CliResponse::EventDetail { data } => {
            println!("{}", serde_json::to_string_pretty(data).unwrap());
        }
        CliResponse::Projects { data } => {
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(data).unwrap());
            } else {
                print_projects_table(data);
            }
        }
        CliResponse::Project { data } => {
            println!("{}", serde_json::to_string_pretty(data).unwrap());
        }
        CliResponse::Error { message } => {
            eprintln!("{} {}", "ERROR:".red(), message);
        }
    }
}

fn print_events_table(data: &serde_json::Value) {
    let events = match data.as_array() {
        Some(arr) => arr,
        None => {
            println!("{data}");
            return;
        }
    };

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
            Cell::new(&message[..60.min(message.len())]),
            Cell::new(timestamp),
        ]);
    }

    println!("{table}");
}

fn print_projects_table(data: &serde_json::Value) {
    let projects = match data.as_array() {
        Some(arr) => arr,
        None => {
            println!("{data}");
            return;
        }
    };

    if projects.is_empty() {
        println!("No projects found.");
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("ID").fg(TableColor::Cyan),
        Cell::new("Name").fg(TableColor::Cyan),
        Cell::new("Keys").fg(TableColor::Cyan),
    ]);

    for project in projects {
        let id = project["project_id"].as_str().unwrap_or("-");
        let name = project["project_name"].as_str().unwrap_or("-");
        let keys_count = project["keys"].as_array().map(|a| a.len()).unwrap_or(0);
        table.add_row(vec![
            Cell::new(id),
            Cell::new(name),
            Cell::new(format!("{keys_count} key(s)")),
        ]);
    }

    println!("{table}");
}

async fn run_repl(server: &str) -> Result<()> {
    println!(
        "{} {}",
        "atriolum-cli".bold().green(),
        "— connected to".dimmed()
    );
    println!("{} {}", "  Server:".dimmed(), server.dimmed());
    println!();
    println!(
        "{}",
        "Type 'help' for commands, 'exit' to quit.".dimmed()
    );

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

        // Parse REPL command
        let request = parse_repl_command(line);
        match request {
            Some(req) => match send_request(server, req).await {
                Ok(resp) => print_response(&resp, "table"),
                Err(e) => eprintln!("{} {}", "ERROR:".red(), e),
            },
            None => {
                eprintln!(
                    "{} {}",
                    "Unknown command:".red(),
                    line.yellow()
                );
                println!(
                    "{}",
                    "Type 'help' for available commands.".dimmed()
                );
            }
        }
    }

    Ok(())
}

fn parse_repl_command(line: &str) -> Option<CliRequest> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    match parts[0] {
        "ping" => Some(CliRequest::Ping),
        "events" if parts.len() > 1 => {
            match parts[1] {
                "list" => {
                    let mut project = None;
                    let mut level = None;
                    let mut limit = None;
                    let mut i = 2;
                    while i < parts.len() {
                        match parts[i] {
                            "--project" if i + 1 < parts.len() => {
                                project = Some(parts[i + 1].to_string());
                                i += 2;
                            }
                            "--level" if i + 1 < parts.len() => {
                                level = Some(parts[i + 1].to_string());
                                i += 2;
                            }
                            "--limit" if i + 1 < parts.len() => {
                                limit = Some(parts[i + 1].parse().unwrap_or(20));
                                i += 2;
                            }
                            _ => i += 1,
                        }
                    }
                    Some(CliRequest::EventsList { project, level, limit })
                }
                "show" if parts.len() > 2 => Some(CliRequest::EventsShow {
                    event_id: parts[2].to_string(),
                    project: parts.get(3).map(|s| s.to_string()),
                }),
                _ => None,
            }
        }
        "projects" if parts.len() > 1 => {
            match parts[1] {
                "list" => Some(CliRequest::ProjectsList),
                "create" if parts.len() > 2 => Some(CliRequest::ProjectsCreate {
                    name: parts[2].to_string(),
                    public_key: uuid::Uuid::new_v4().to_string(),
                }),
                _ => None,
            }
        }
        _ => None,
    }
}

fn print_help() {
    println!();
    println!("{}", "Available commands:".bold());
    println!(
        "  {} {}",
        "events list".green(),
        "[--project P] [--level L] [--limit N]".dimmed()
    );
    println!("  {} {}", "events show".green(), "<event_id> [project]".dimmed());
    println!("  {}", "projects list".green());
    println!("  {} {}", "projects create".green(), "<name>".dimmed());
    println!("  {}", "ping".green());
    println!("  {}", "help".green());
    println!("  {}", "exit".green());
    println!();
}
