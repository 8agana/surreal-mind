use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

#[derive(Parser)]
#[command(name = "photography")]
#[command(about = "Photography database CLI tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Import roster from CSV
    Import {
        /// Competition name
        #[arg(long)]
        competition: String,
        /// CSV file path
        #[arg(long)]
        file: String,
    },
    /// List operations
    List {
        #[command(subcommand)]
        list_command: ListCommands,
    },
    /// Show operations
    Show {
        #[command(subcommand)]
        show_command: ShowCommands,
    },
    /// Update operations
    Update {
        #[command(subcommand)]
        update_command: UpdateCommands,
    },
}

#[derive(Subcommand)]
enum ListCommands {
    /// List skaters by status
    Skaters {
        /// Status filter (requested, vip, unrequested, all)
        #[arg(long, default_value = "all")]
        status: String,
    },
    /// List events for competition
    Events {
        /// Competition name
        #[arg(long)]
        competition: String,
    },
}

#[derive(Subcommand)]
enum ShowCommands {
    /// Show event details
    Event {
        /// Event number
        event_number: u32,
        /// Split ice (optional)
        #[arg(long)]
        split: Option<String>,
    },
}

#[derive(Subcommand)]
enum UpdateCommands {
    /// Update gallery status
    Gallery {
        /// Skater name (LastName, FirstName)
        #[arg(long)]
        skater: String,
        /// Event number
        #[arg(long)]
        event: u32,
        /// New status (pending, culling, processing, sent, purchased)
        #[arg(long)]
        status: String,
        /// Gallery URL (for sent status)
        #[arg(long)]
        url: Option<String>,
        /// Purchase amount (for purchased status)
        #[arg(long)]
        amount: Option<f64>,
    },
}

#[derive(Debug, serde::Deserialize)]
struct RosterRow {
    #[serde(rename = "Time")]
    time: Option<String>,
    #[serde(rename = "Event")]
    event: u32,
    #[serde(rename = "Split Ice")]
    split_ice: Option<String>,
    #[serde(rename = "Skate Order")]
    skate_order: Option<u32>,
    #[serde(rename = "Skater Name")]
    skater_name: String,
    #[serde(rename = "SignUp")]
    signup: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    let url = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
    let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
    let ns = "photography";
    let dbname = "ops";

    // Connect to SurrealDB
    let db = Surreal::new::<Ws>(url).await?;
    db.signin(Root {
        username: &user,
        password: &pass,
    })
    .await?;
    db.use_ns(ns).use_db(dbname).await?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Import { competition, file } => {
            import_roster(&db, &competition, &file).await?;
        }
        Commands::List { list_command } => match list_command {
            ListCommands::Skaters { status } => {
                list_skaters(&db, &status).await?;
            }
            ListCommands::Events { competition } => {
                list_events(&db, &competition).await?;
            }
        },
        Commands::Show { show_command } => match show_command {
            ShowCommands::Event {
                event_number,
                split,
            } => {
                show_event(&db, event_number, split.as_deref()).await?;
            }
        },
        Commands::Update { update_command } => match update_command {
            UpdateCommands::Gallery {
                skater,
                event,
                status,
                url,
                amount,
            } => {
                update_gallery(&db, &skater, event, &status, url.as_deref(), amount).await?;
            }
        },
    }

    Ok(())
}

async fn import_roster(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    competition: &str,
    file_path: &str,
) -> Result<()> {
    println!("Importing roster for competition: {}", competition);
    println!("From file: {}", file_path);

    // Create competition if not exists
    let comp_id = competition_to_id(competition);
    let _ = db
        .query(&format!(
            "CREATE competition SET id = '{}', name = '{}'",
            comp_id, competition
        ))
        .await?;

    // Read CSV
    let file = File::open(file_path)?;
    let mut rdr = csv::Reader::from_reader(file);

    for result in rdr.deserialize() {
        let row: RosterRow = result?;
        println!("Processing: {:?}", row);

        // Parse skater name
        let (last_name, first_name) = parse_skater_name(&row.skater_name)?;
        let skater_id = format!("{}_{}", last_name.to_lowercase(), first_name.to_lowercase());

        // Create skater
        let skater_id =
            format!("{}_{}", last_name.to_lowercase(), first_name.to_lowercase()).replace('-', "_");
        let _ = db
            .query(&format!(
                "CREATE skater SET id = '{}', first_name = '{}', last_name = '{}'",
                skater_id, first_name, last_name
            ))
            .await?;

        // Create event
        let event_id = format!(
            "{}_{}{}",
            comp_id,
            row.event,
            row.split_ice
                .as_ref()
                .map(|s| format!("_{}", s))
                .unwrap_or_default()
        );
        let event_query = if let Some(split) = &row.split_ice {
            format!(
                "CREATE event SET id = '{}', competition = competition:{}, event_number = {}, split_ice = '{}'",
                event_id, comp_id, row.event, split
            )
        } else {
            format!(
                "CREATE event SET id = '{}', competition = competition:{}, event_number = {}",
                event_id, comp_id, row.event
            )
        };
        let _ = db.query(&event_query).await?;

        // Determine request status
        let request_status = match row.signup.as_deref() {
            Some("VIP") => "vip",
            Some("TRUE") => "requested",
            _ => "unrequested",
        };

        // Create competed_in relation
        let skate_order = row.skate_order.unwrap_or(0);
        let _ = db.query(&format!(
            "RELATE skater:{}->competed_in->event:{} SET skate_order = {}, request_status = '{}', gallery_status = 'pending'",
            skater_id, event_id, skate_order, request_status
        )).await?;
    }

    println!("Import completed successfully!");
    Ok(())
}

async fn list_skaters(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    status: &str,
) -> Result<()> {
    let query = if status == "all" {
        "SELECT skater.first_name, skater.last_name, competed_in.event.event_number, competed_in.request_status, competed_in.gallery_status FROM competed_in FETCH skater, event".to_string()
    } else {
        format!(
            "SELECT skater.first_name, skater.last_name, competed_in.event.event_number, competed_in.request_status, competed_in.gallery_status FROM competed_in WHERE competed_in.request_status = '{}' FETCH skater, event",
            status
        )
    };

    let result: Vec<Value> = db.query(&query).await?.take(0)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn list_events(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    competition: &str,
) -> Result<()> {
    let result: Vec<Value> = db.query(&format!(
        "SELECT id, event_number, split_ice, level, discipline FROM event WHERE competition.name = '{}'",
        competition
    )).await?.take(0)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn show_event(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    event_number: u32,
    split: Option<&str>,
) -> Result<()> {
    let query = if let Some(split_val) = split {
        format!(
            "SELECT id, event_number, split_ice, level, discipline, time_slot, notes FROM event WHERE event_number = {} AND split_ice = '{}'",
            event_number, split_val
        )
    } else {
        format!(
            "SELECT id, event_number, split_ice, level, discipline, time_slot, notes FROM event WHERE event_number = {}",
            event_number
        )
    };

    let result: Vec<Value> = db.query(&query).await?.take(0)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn update_gallery(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    skater: &str,
    event: u32,
    status: &str,
    url: Option<&str>,
    amount: Option<f64>,
) -> Result<()> {
    // Parse skater name
    let (last_name, first_name) = parse_skater_name(skater)?;
    let skater_id = format!("{}_{}", last_name.to_lowercase(), first_name.to_lowercase());

    // Find the competed_in relation
    let mut update_query = format!(
        "UPDATE competed_in SET gallery_status = '{}' WHERE skater = skater:{} AND event.event_number = {}",
        status, skater_id, event
    );

    if let Some(url_val) = url {
        update_query.push_str(&format!(", gallery_url = '{}'", url_val));
    }

    if let Some(amount_val) = amount {
        update_query.push_str(&format!(", purchase_amount = {}", amount_val));
        update_query.push_str(", purchase_date = time::now()");
    }

    if status == "sent" {
        update_query.push_str(", gallery_sent_at = time::now()");
    }

    let _ = db.query(&update_query).await?;
    println!("Gallery status updated successfully!");
    Ok(())
}

fn competition_to_id(competition: &str) -> String {
    competition
        .to_lowercase()
        .replace(" ", "_")
        .replace(",", "")
        .replace("-", "_")
}

fn parse_skater_name(name: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = name.split_whitespace().collect();
    match parts.len() {
        0 => Err(anyhow::anyhow!("Empty skater name")),
        1 => Ok(("Synchro".to_string(), parts[0].to_string())),
        2 => Ok((parts[0].to_string(), parts[1].to_string())),
        _ => {
            let first = parts[0].to_string();
            let last = parts[1..].join("-");
            Ok((first, last))
        }
    }
}
