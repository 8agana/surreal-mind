use anyhow::Result;
use clap::{Parser, Subcommand};
use prettytable::{Cell, Row, Table, row}; // Consolidated prettytable imports
use serde::Deserialize; // Use specific Deserialize trait
use serde_json::Value; // Still needed for some functions
use std::fs::File; // Still needed for import_roster
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;
use surrealdb::sql::Thing; // For surrealdb::sql::Thing in Family struct

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
    /// Query skater details including status (replaces old QuerySkater)
    QuerySkater { last_name: String },
    /// Get family contact email for gallery delivery (replaces old GetEmail)
    GetEmail { last_name: String },
    /// Mark a gallery as SENT for a specific competition
    MarkSent {
        last_name: String,
        #[arg(default_value = "2025_fall_fling")]
        competition: String,
    },
    /// Record a purchase for a family
    RecordPurchase {
        last_name: String,
        amount: f64,
        #[arg(default_value = "2025_fall_fling")]
        competition: String,
    },
    /// Check delivery status for a competition
    CheckStatus {
        #[arg(default_value = "2025_fall_fling")]
        competition: String,
        #[arg(long)]
        pending_only: bool,
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
}

#[derive(Debug, serde::Deserialize)]
struct SkaterRow {
    first_name: String,
    last_name: String,
    comp_name: Option<String>, // Changed to Option<String>
    event_num: Option<i32>,    // Changed to Option<i32>
    req_status: Option<String>,
    gal_status: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct StatusRow {
    family_name: String,
    email: Option<String>,
    request_status: String,
    gallery_status: String,
    sent_date: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct Family {
    id: surrealdb::sql::Thing,
    last_name: String,
    email: Option<String>,
}

/// Returns ID with backticks for safety: family:`appleby-leo`
fn format_family_id(last_name: &str) -> String {
    let lower = last_name.to_lowercase();
    // Always backtick if it contains non-alphanumeric or spaces just to be safe
    if lower.chars().any(|c| !c.is_alphanumeric()) || lower.contains(' ') {
        format!("family:`{}`", lower)
    } else {
        format!("family:{}", lower)
    }
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
        Commands::QuerySkater { last_name } => {
            query_skater(&db, &last_name).await?;
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
        Commands::MarkSent {
            last_name,
            competition,
        } => {
            mark_sent(&db, &last_name, &competition).await?;
        }
        Commands::RecordPurchase {
            last_name,
            amount,
            competition,
        } => {
            record_purchase(&db, &last_name, amount, &competition).await?;
        }
        Commands::CheckStatus {
            competition,
            pending_only,
        } => {
            check_status(&db, &competition, pending_only).await?;
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
                     VALUES ($id, 'first', 'last', time::now())
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

async fn mark_sent(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    last_name: &str,
    comp: &str,
) -> Result<()> {
    let family_id_full = format_family_id(last_name); // e.g., "family:shawhan"
    let family_id_only = last_name.to_lowercase().replace(" ", "-"); // e.g., "shawhan"
    let competition_id_only = comp.to_lowercase(); // e.g., "2025_pony_express"

    // 1. Check existence explicitly using raw SQL to avoid SDK "Table Name" confusion
    let check_sql = "SELECT * FROM type::thing('family', $id)";
    let mut check_resp = db.query(check_sql).bind(("id", &family_id_only)).await?;
    let check: Vec<serde_json::Value> = check_resp.take(0)?;
    
    if check.is_empty() {
        println!("❌ Error: Family {} not found.", family_id_full);
        return Ok(());
    }

    // 2. Update
    println!("Marking SENT: {} -> {}", family_id_full, competition_id_only);
    let sql = "
        UPDATE family_competition
        SET gallery_status = 'sent'
                WHERE in = type::thing('family', $family_id)
                AND out = type::thing('competition', $competition_id)
    ";
    let _ = db.query(sql)
        .bind(("family_id", family_id_only))
        .bind(("competition_id", competition_id_only))
        .await?;
    println!("✅ Update complete.");
    Ok(())
}

async fn check_status(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    comp_name: &str,
    pending_only: bool,
) -> Result<()> {
    let comp_id = format!("competition:{}", comp_name);
    let mut sql = String::from(
        "SELECT in.last_name as family_name,
                in.email as email,
                request_status,
                gallery_status,
                sent_date
         FROM family_competition
         WHERE out = type::thing($comp)
         AND request_status = 'requested' ", // Only care about requested families
    );

    if pending_only {
        sql.push_str(" AND gallery_status = 'pending'");
    }

    // Sort by status (pending first) then name
    sql.push_str(" ORDER BY gallery_status ASC, family_name ASC");

    let mut response = db.query(&sql).bind(("comp", comp_id)).await?;
    let rows: Vec<StatusRow> = response.take(0)?;

    println!("Status Report for {}", comp_name);
    println!("Found {} records", rows.len());

    let mut table = Table::new();
    table.add_row(row!["Family", "Email", "Request", "Status", "Sent Date"]);

    for r in rows {
        table.add_row(row![
            r.family_name,
            r.email.unwrap_or_else(|| "-".to_string()),
            r.request_status,
            r.gallery_status,
            r.sent_date.unwrap_or_else(|| "-".to_string())
        ]);
    }

    table.printstd();
    Ok(())
}

async fn record_purchase(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    last_name: &str,
    amount: f64,
    comp: &str,
) -> Result<()> {
    let family_id_full = format_family_id(last_name);
    let family_id_only = last_name.to_lowercase().replace(" ", "-");
    let competition_id_only = comp.to_lowercase();

    // Check existence using raw SQL
    let check_sql = "SELECT * FROM type::thing('family', $id)";
    let mut check_resp = db.query(check_sql).bind(("id", &family_id_only)).await?;
    let check: Vec<serde_json::Value> = check_resp.take(0)?;

    if check.is_empty() {
        println!("❌ Error: Family {} not found.", family_id_full);
        return Ok(());
    }

    // Update
    println!(
        "Recording purchase: {} -> {} for amount {}",
        family_id_full, competition_id_only, amount
    );
    let sql = "
        UPDATE family_competition
        SET gallery_status = 'purchased', purchase_amount = $amount, purchase_date = time::now()
        WHERE in = type::thing('family', $family_id)
        AND out = type::thing('competition', $competition_id)
    ";
    let _ = db
        .query(sql)
        .bind(("family_id", family_id_only))
        .bind(("competition_id", competition_id_only))
        .bind(("amount", amount))
        .await?;
    println!("✅ Purchase recorded.");
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
    last_name: &str,
) -> Result<()> {
    println!("Searching for skater: {}", last_name);

    let sql = "
        SELECT
            first_name,
            last_name,
            array::first(->competed_in->event->competition.name) as comp_name,
            array::first(->competed_in->event.event_number) as event_num,
            array::first(->belongs_to->family->family_competition.request_status) as req_status,
            array::first(->belongs_to->family->family_competition.gallery_status) as gal_status
        FROM skater
        WHERE string::lowercase(last_name) CONTAINS string::lowercase($name)
    ";

    let mut response = db.query(sql).bind(("name", last_name.to_string())).await?;
    let rows: Vec<SkaterRow> = response.take(0)?;

    if rows.is_empty() {
        println!("No skaters found.");
        return Ok(());
    }

    let mut table = Table::new();
    table.add_row(row![
        "Skater",
        "Competition",
        "Event",
        "Req Status",
        "Gal Status"
    ]);
    for r in rows {
        let r_status = r.req_status.unwrap_or_else(|| "-".to_string());
        let g_status = r.gal_status.unwrap_or_else(|| "pending".to_string());

        table.add_row(row![
            format!("{}, {}", r.last_name, r.first_name),
            r.comp_name.unwrap_or_else(|| "-".to_string()),
            r.event_num
                .map_or_else(|| "-".to_string(), |e| e.to_string()),
            r_status,
            g_status
        ]);
    }
    table.printstd();
    Ok(())
}

async fn get_email(
    db: &Surreal<surrealdb::engine::remote::ws::Client>,
    last_name: &str,
) -> Result<()> {
    let family_id_str = format_family_id(last_name);

    // Use db.select to get the family record directly by its formatted ID
    let family: Vec<Family> = db.select(&family_id_str).await?;

    if !family.is_empty() {
        let f = &family[0];
        let mut table = Table::new();
        table.add_row(row!["Last Name", "Email", "ID"]);
        table.add_row(row![
            f.last_name,
            f.email.clone().unwrap_or_else(|| "NO EMAIL".to_string()),
            f.id.to_string()
        ]);
        table.printstd();
    } else {
        println!(
            "No family found with last name '{}'. (Searched for ID: {})",
            last_name, family_id_str
        );
    }
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
