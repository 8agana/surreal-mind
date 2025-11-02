use anyhow::Result;
use clap::{Parser, Subcommand};
use prettytable::{Cell, Row, Table};
use serde_json::Value;
use std::fs::File;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;

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
    /// Query skater by name
    QuerySkater {
        /// Skater name (partial match on first or last)
        name: String,
        /// Optional competition name
        #[arg(long)]
        competition: Option<String>,
    },
    /// List pending galleries
    PendingGalleries {
        /// Optional competition name
        #[arg(long)]
        competition: Option<String>,
    },
    /// List events for skater
    ListEventsForSkater {
        /// Skater last name
        #[arg(long)]
        skater: String,
        /// Optional competition name
        #[arg(long)]
        competition: Option<String>,
    },
    /// Show competition statistics
    CompetitionStats {
        /// Competition name
        comp_name: String,
    },
    /// Get email for skater/family
    GetEmail {
        /// Last name to lookup
        last_name: String,
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

#[derive(Debug)]
struct ParsedSkater {
    first_name: String,
    last_name: String,
    family_email: Option<String>,
}

#[derive(Debug)]
struct ParsedName {
    skaters: Vec<ParsedSkater>,
    is_family: bool,
    is_synchro: bool,
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
    db.signin(surrealdb::opt::auth::Root {
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
        Commands::QuerySkater { name, competition } => {
            query_skater(&db, &name, competition.as_deref()).await?;
        }
        Commands::PendingGalleries { competition } => {
            pending_galleries(&db, competition.as_deref()).await?;
        }
        Commands::ListEventsForSkater {
            skater,
            competition,
        } => {
            list_events_for_skater(&db, &skater, competition.as_deref()).await?;
        }
        Commands::CompetitionStats { comp_name } => {
            competition_stats(&db, &comp_name).await?;
        }
        Commands::GetEmail { last_name } => {
            get_email(&db, &last_name).await?;
        }
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

    // Upsert competition record
    let comp_id = competition_to_id(competition);
    let comp_resp = db
        .query(
            "INSERT INTO competition (id, name, venue, start_date, end_date)
             VALUES ($id, $name, $venue, time::now(), time::now())
             ON DUPLICATE KEY UPDATE name = $name",
        )
        .bind(("id", comp_id.clone()))
        .bind(("name", competition.to_string()))
        .bind(("venue", ""))
        .await?;
    comp_resp.check()?;

    // Read CSV
    let file = File::open(file_path)?;
    let mut rdr = csv::Reader::from_reader(file);

    for result in rdr.deserialize() {
        let row: RosterRow = result?;
        println!("Processing: {:?}", row);

        // Parse skater names
        let parsed = parse_skater_names(&row.skater_name)?;

        // If family, upsert family
        let family_id = if parsed.is_family {
            let family_id = format!(
                "family:{}",
                parsed.skaters[0].last_name.to_lowercase().replace(" ", "_")
            );
            let family_resp = db
                .query(
                    "INSERT INTO family (id, first_name, last_name, created_at)
                     VALUES ($id, 'Family', $last, time::now())
                     ON DUPLICATE KEY UPDATE first_name = 'Family', last_name = $last",
                )
                .bind(("id", family_id.clone()))
                .bind(("last", parsed.skaters[0].last_name.clone()))
                .await?;
            family_resp.check()?;
            Some(family_id)
        } else {
            None
        };

        // Upsert event (once per row)
        let event_id = format!(
            "{}_{}{}",
            comp_id,
            row.event,
            row.split_ice
                .as_ref()
                .map(|s| format!("_{}", s))
                .unwrap_or_default()
        );
        let event_resp = db
            .query(
                "INSERT INTO event (id, competition, event_number, split_ice, time_slot)
                 VALUES ($id, type::thing('competition', $comp), $event_number, $split, $time)
                 ON DUPLICATE KEY UPDATE
                    competition = type::thing('competition', $comp),
                    event_number = $event_number,
                    split_ice = $split,
                    time_slot = $time",
            )
            .bind(("id", event_id.clone()))
            .bind(("comp", comp_id.clone()))
            .bind(("event_number", row.event))
            .bind(("split", row.split_ice.clone()))
            .bind(("time", row.time.clone()))
            .await?;
        event_resp.check()?;

        // Determine request status
        let request_status = match row.signup.as_deref() {
            Some("VIP") => "vip",
            Some("TRUE") => "requested",
            _ => "unrequested",
        };

        // For each skater
        for skater in &parsed.skaters {
            let skater_id = format!(
                "{}_{}",
                skater.last_name.to_lowercase(),
                skater.first_name.to_lowercase()
            )
            .replace('-', "_");

            // Upsert skater
            let skater_resp = db
                .query(
                    "INSERT INTO skater (id, first_name, last_name, created_at)
                     VALUES ($id, $first, $last, time::now())
                     ON DUPLICATE KEY UPDATE first_name = $first, last_name = $last",
                )
                .bind(("id", skater_id.clone()))
                .bind(("first", skater.first_name.clone()))
                .bind(("last", skater.last_name.clone()))
                .await?;
            skater_resp.check()?;

            // If family, create belongs_to
            if let Some(ref family_id) = family_id {
                let belongs_resp = db
                    .query(
                        "RELATE (type::thing('skater', $skater_id))->belongs_to->(type::thing('family', $family_id))
                         CONTENT { created_at: time::now() }",
                    )
                    .bind(("skater_id", skater_id.clone()))
                    .bind(("family_id", family_id.clone()))
                    .await?;
                belongs_resp.check()?;
            }

            // Create competed_in relation
            let relation_resp = db
                .query(
                    "RELATE (type::thing('skater', $skater_id))->competed_in->(type::thing('event', $event_id))
                     CONTENT {
                        skate_order: $skate_order,
                        request_status: $request_status,
                        gallery_status: 'pending'
                     }",
                )
                .bind(("skater_id", skater_id.clone()))
                .bind(("event_id", event_id.clone()))
                .bind(("skate_order", row.skate_order.unwrap_or(0)))
                .bind(("request_status", request_status.to_string()))
                .await?;
            relation_resp.check()?;
        }
    }

    println!("Import completed successfully!");
    Ok(())
}

async fn list_skaters(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    status: &str,
) -> Result<()> {
    let mut resp = if status == "all" {
        db.query(
            "SELECT
                in.first_name AS first_name,
                in.last_name AS last_name,
                out.event_number AS event_number,
                out.split_ice AS split_ice,
                request_status,
                gallery_status
             FROM competed_in FETCH in, out",
        )
        .await?
    } else {
        db.query(
            "SELECT
                in.first_name AS first_name,
                in.last_name AS last_name,
                out.event_number AS event_number,
                out.split_ice AS split_ice,
                request_status,
                gallery_status
             FROM competed_in
             WHERE request_status = $status
             FETCH in, out",
        )
        .bind(("status", status.to_string()))
        .await?
    };

    let result: Vec<Value> = resp.take(0)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn list_events(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    competition: &str,
) -> Result<()> {
    let mut resp = db
        .query(
            "SELECT event_number, split_ice, level, discipline, time_slot
             FROM event
             WHERE competition = type::thing('competition', $comp)",
        )
        .bind(("comp", competition_to_id(competition)))
        .await?;
    let result: Vec<Value> = resp.take(0)?;
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
            "SELECT event_number, split_ice, level, discipline, time_slot, notes FROM event WHERE event_number = {} AND split_ice = '{}'",
            event_number, split_val
        )
    } else {
        format!(
            "SELECT event_number, split_ice, level, discipline, time_slot, notes FROM event WHERE event_number = {}",
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
    // Parse skater names
    let parsed = parse_skater_names(skater)?;
    if parsed.skaters.len() != 1 {
        return Err(anyhow::anyhow!(
            "Update gallery requires exactly one skater"
        ));
    }
    let skater_data = &parsed.skaters[0];
    let first_name = skater_data.first_name.clone();
    let last_name = skater_data.last_name.clone();
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

async fn query_skater(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    name: &str,
    comp: Option<&str>,
) -> Result<()> {
    let query = if let Some(c) = comp {
        format!(
            "SELECT in.first_name, in.last_name, out.competition.name, out.event_number, request_status, gallery_status
             FROM competed_in
             WHERE (in.first_name CONTAINS '{}' OR in.last_name CONTAINS '{}')
               AND out.competition.name CONTAINS '{}'
             FETCH in, out, out.competition",
            name, name, c
        )
    } else {
        format!(
            "SELECT in.first_name, in.last_name, out.competition.name, out.event_number, request_status, gallery_status
             FROM competed_in
             WHERE in.first_name CONTAINS '{}' OR in.last_name CONTAINS '{}'
             FETCH in, out, out.competition",
            name, name
        )
    };
    let mut resp = db.query(&query).await?;
    let results: Vec<Value> = resp.take(0)?;
    let mut table = Table::new();
    table.add_row(Row::new(vec![
        Cell::new("Last Name"),
        Cell::new("First Name"),
        Cell::new("Competition"),
        Cell::new("Event #"),
        Cell::new("Request Status"),
        Cell::new("Gallery Status"),
    ]));
    for r in results {
        table.add_row(Row::new(vec![
            Cell::new(r["in"]["last_name"].as_str().unwrap_or("")),
            Cell::new(r["in"]["first_name"].as_str().unwrap_or("")),
            Cell::new(r["out"]["competition"]["name"].as_str().unwrap_or("")),
            Cell::new(&r["out"]["event_number"].to_string()),
            Cell::new(r["request_status"].as_str().unwrap_or("")),
            Cell::new(r["gallery_status"].as_str().unwrap_or("")),
        ]));
    }
    table.printstd();
    Ok(())
}

async fn get_email(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    last_name: &str,
) -> Result<()> {
    // First try to find family by last name (case-insensitive)
    let query = format!(
        "SELECT last_name, email FROM family WHERE string::lowercase(last_name) CONTAINS string::lowercase('{}')",
        last_name
    );
    let mut resp = db.query(&query).await?;
    let families: Vec<Value> = resp.take(0)?;

    if families.is_empty() {
        println!("No family found with last name containing: {}", last_name);
        return Ok(());
    }

    // Display results
    let mut table = Table::new();
    table.add_row(Row::new(vec![
        Cell::new("Last Name"),
        Cell::new("Email"),
    ]));

    for family in families {
        let name = family["last_name"].as_str().unwrap_or("N/A");
        let email = family["email"].as_str().unwrap_or("No email on file");
        table.add_row(Row::new(vec![
            Cell::new(name),
            Cell::new(email),
        ]));
    }

    table.printstd();
    Ok(())
}

async fn pending_galleries(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    comp: Option<&str>,
) -> Result<()> {
    let query = if let Some(c) = comp {
        format!(
            "SELECT in.first_name, in.last_name, out.competition.name, out.event_number, request_status
             FROM competed_in
             WHERE gallery_status = 'pending' AND out.competition.name CONTAINS '{}'
             FETCH in, out, out.competition",
            c
        )
    } else {
        "SELECT in.first_name, in.last_name, out.competition.name, out.event_number, request_status
         FROM competed_in
         WHERE gallery_status = 'pending'
         FETCH in, out, out.competition"
            .to_string()
    };
    let mut resp = db.query(&query).await?;
    let results: Vec<Value> = resp.take(0)?;
    let mut table = Table::new();
    table.add_row(Row::new(vec![
        Cell::new("Last Name"),
        Cell::new("First Name"),
        Cell::new("Competition"),
        Cell::new("Event #"),
        Cell::new("Request Status"),
    ]));
    for r in results {
        table.add_row(Row::new(vec![
            Cell::new(r["in"]["last_name"].as_str().unwrap_or("")),
            Cell::new(r["in"]["first_name"].as_str().unwrap_or("")),
            Cell::new(r["out"]["competition"]["name"].as_str().unwrap_or("")),
            Cell::new(&r["out"]["event_number"].to_string()),
            Cell::new(r["request_status"].as_str().unwrap_or("")),
        ]));
    }
    table.printstd();
    Ok(())
}

async fn list_events_for_skater(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    last_name: &str,
    comp: Option<&str>,
) -> Result<()> {
    let query = if let Some(c) = comp {
        format!(
            "SELECT out.competition.name, out.event_number, request_status, gallery_status, purchase_amount
             FROM competed_in
             WHERE in.last_name = '{}' AND out.competition.name CONTAINS '{}'
             FETCH in, out, out.competition",
            last_name, c
        )
    } else {
        format!(
            "SELECT out.competition.name, out.event_number, request_status, gallery_status, purchase_amount
             FROM competed_in
             WHERE in.last_name = '{}'
             FETCH in, out, out.competition",
            last_name
        )
    };
    let mut resp = db.query(&query).await?;
    let results: Vec<Value> = resp.take(0)?;
    let mut table = Table::new();
    table.add_row(Row::new(vec![
        Cell::new("Competition"),
        Cell::new("Event #"),
        Cell::new("Request Status"),
        Cell::new("Gallery Status"),
        Cell::new("Purchase Amount"),
    ]));
    for r in results {
        table.add_row(Row::new(vec![
            Cell::new(r["out"]["competition"]["name"].as_str().unwrap_or("")),
            Cell::new(&r["out"]["event_number"].to_string()),
            Cell::new(r["request_status"].as_str().unwrap_or("")),
            Cell::new(r["gallery_status"].as_str().unwrap_or("")),
            Cell::new(&r["purchase_amount"].to_string()),
        ]));
    }
    table.printstd();
    Ok(())
}

async fn competition_stats(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    comp_name: &str,
) -> Result<()> {
    let lower_comp = comp_name.to_lowercase();
    let condition = format!("LOWER(out.competition.name) CONTAINS '{}'", lower_comp);

    // Total distinct skaters
    let mut total_skaters_resp = db
        .query(format!(
            "SELECT count(DISTINCT in) FROM competed_in WHERE {} FETCH out.competition",
            condition
        ))
        .await?;
    let total_skaters_results: Vec<Value> = total_skaters_resp.take(0)?;
    let total_skaters = total_skaters_results
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_i64())
        .unwrap_or(0i64);

    // Total competed_in relations
    let mut total_competed_resp = db
        .query(format!(
            "SELECT count() FROM competed_in WHERE {}",
            condition
        ))
        .await?;
    let total_competed_results: Vec<Value> = total_competed_resp.take(0)?;
    let total_competed = total_competed_results
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_i64())
        .unwrap_or(0i64);

    // Requested (vip or requested)
    let mut requested_resp = db
        .query(format!("SELECT count() FROM competed_in WHERE {} AND (request_status = 'requested' OR request_status = 'vip')", condition))
        .await?;
    let requested_results: Vec<Value> = requested_resp.take(0)?;
    let requested = requested_results
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_i64())
        .unwrap_or(0i64);

    // Unrequested
    let mut unrequested_resp = db
        .query(format!(
            "SELECT count() FROM competed_in WHERE {} AND request_status = 'unrequested'",
            condition
        ))
        .await?;
    let unrequested_results: Vec<Value> = unrequested_resp.take(0)?;
    let unrequested = unrequested_results
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_i64())
        .unwrap_or(0i64);

    // Pending galleries
    let mut pending_galleries_resp = db
        .query(format!(
            "SELECT count() FROM competed_in WHERE {} AND gallery_status = 'pending'",
            condition
        ))
        .await?;
    let pending_galleries_results: Vec<Value> = pending_galleries_resp.take(0)?;
    let pending_galleries = pending_galleries_results
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_i64())
        .unwrap_or(0i64);

    // Sent galleries
    let mut sent_galleries_resp = db
        .query(format!(
            "SELECT count() FROM competed_in WHERE {} AND gallery_status = 'sent'",
            condition
        ))
        .await?;
    let sent_galleries_results: Vec<Value> = sent_galleries_resp.take(0)?;
    let sent_galleries = sent_galleries_results
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_i64())
        .unwrap_or(0i64);

    // Purchased galleries
    let mut purchased_galleries_resp = db
        .query(format!(
            "SELECT count() FROM competed_in WHERE {} AND gallery_status = 'purchased'",
            condition
        ))
        .await?;
    let purchased_galleries_results: Vec<Value> = purchased_galleries_resp.take(0)?;
    let purchased_galleries = purchased_galleries_results
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_i64())
        .unwrap_or(0i64);

    println!("Statistics for competition '{}':", comp_name);
    println!("Total skaters: {}", total_skaters);
    println!("Total competed_in relations: {}", total_competed);
    println!("Requested relations: {}", requested);
    println!("Unrequested relations: {}", unrequested);
    println!("Pending galleries: {}", pending_galleries);
    println!("Sent galleries: {}", sent_galleries);
    println!("Purchased galleries: {}", purchased_galleries);
    Ok(())
}

fn parse_skater_names(name: &str) -> Result<ParsedName> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow::anyhow!("Empty skater name"));
    }

    // Check if it's synchro
    if name.to_lowercase().starts_with("synchro ") {
        let team = name[8..].trim();
        let skater = ParsedSkater {
            first_name: "Synchro".to_string(),
            last_name: team.to_string(),
            family_email: None,
        };
        return Ok(ParsedName {
            skaters: vec![skater],
            is_family: false,
            is_synchro: true,
        });
    }

    // Split into words
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.is_empty() {
        return Err(anyhow::anyhow!("Empty skater name"));
    }

    // Last word is last_name
    let last_name = words.last().unwrap().to_string();

    // First part is all except last word
    let first_part = &name[..name.len() - last_name.len()].trim();

    // Parse first_part
    let first_names: Vec<String> = first_part
        .split(',')
        .flat_map(|s| s.split(" and "))
        .map(|s| s.trim().trim_end_matches(','))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    if first_names.is_empty() {
        return Err(anyhow::anyhow!("No first names found"));
    }

    let skaters: Vec<ParsedSkater> = first_names
        .into_iter()
        .map(|first| ParsedSkater {
            first_name: first,
            last_name: last_name.clone(),
            family_email: None,
        })
        .collect();

    let is_family = skaters.len() > 1;

    Ok(ParsedName {
        skaters,
        is_family,
        is_synchro: false,
    })
}
