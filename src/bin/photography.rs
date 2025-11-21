use anyhow::Result;
use clap::{Parser, Subcommand};
use surreal_mind::photography::DEFAULT_COMPETITION;
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
    /// Query skater details including status (replaces old QuerySkater)
    QuerySkater { last_name: String },
    /// Get family contact email for gallery delivery (replaces old GetEmail)
    GetEmail { last_name: String },
    /// Mark a gallery as SENT for a specific competition
    MarkSent {
        last_name: String,
        #[arg(default_value = DEFAULT_COMPETITION)]
        competition: String,
    },
    /// Request a Thank You gallery (sets ty_requested=true)
    RequestTy {
        last_name: String,
        #[arg(default_value = DEFAULT_COMPETITION)]
        competition: String,
    },
    /// Send a Thank You gallery (sets ty_sent=true, timestamps)
    SendTy {
        last_name: String,
        #[arg(default_value = DEFAULT_COMPETITION)]
        competition: String,
    },
    /// Record a purchase for a family
    RecordPurchase {
        last_name: String,
        amount: f64,
        #[arg(default_value = DEFAULT_COMPETITION)]
        competition: String,
    },
    /// Check delivery status for a competition
    CheckStatus {
        #[arg(default_value = DEFAULT_COMPETITION)]
        competition: String,
        #[arg(long)]
        pending_only: bool,
        #[arg(long)]
        ty_pending: bool,
        #[arg(long)]
        status: Option<String>,
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
        /// New status (pending, culling, processing, sent, purchased, not_shot, needs_research)
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
    db.use_ns(ns).await?;
    db.use_db(dbname).await?;

    // Parse CLI
    let cli = Cli::parse();

    // Dispatch to commands
    match cli.command {
        Commands::Import { competition, file } => {
            surreal_mind::photography::commands::import_roster(&db, &competition, &file).await?;
        }
        Commands::List { list_command } => match list_command {
            ListCommands::Skaters { status } => {
                surreal_mind::photography::commands::list_skaters(&db, &status).await?;
            }
            ListCommands::Events { competition } => {
                surreal_mind::photography::commands::list_events(&db, &competition).await?;
            }
        },
        Commands::Show { show_command } => match show_command {
            ShowCommands::Event {
                event_number,
                split,
            } => {
                surreal_mind::photography::commands::show_event(
                    &db,
                    event_number,
                    split.as_deref(),
                )
                .await?;
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
                surreal_mind::photography::commands::update_gallery(
                    &db,
                    &skater,
                    event,
                    &status,
                    url.as_deref(),
                    amount,
                )
                .await?;
            }
        },
        Commands::QuerySkater { last_name } => {
            surreal_mind::photography::commands::query_skater(&db, &last_name).await?;
        }
        Commands::GetEmail { last_name } => {
            surreal_mind::photography::commands::get_email(&db, &last_name).await?;
        }
        Commands::MarkSent {
            last_name,
            competition,
        } => {
            surreal_mind::photography::commands::mark_sent(&db, &last_name, &competition).await?;
        }
        Commands::RequestTy {
            last_name,
            competition,
        } => {
            surreal_mind::photography::commands::request_ty(&db, &last_name, &competition).await?;
        }
        Commands::SendTy {
            last_name,
            competition,
        } => {
            surreal_mind::photography::commands::send_ty(&db, &last_name, &competition).await?;
        }
        Commands::RecordPurchase {
            last_name,
            amount,
            competition,
        } => {
            surreal_mind::photography::commands::record_purchase(
                &db,
                &last_name,
                amount,
                &competition,
            )
            .await?;
        }
        Commands::CheckStatus {
            competition,
            pending_only,
            ty_pending,
            status,
        } => {
            surreal_mind::photography::commands::check_status(
                &db,
                &competition,
                pending_only,
                ty_pending,
                status.as_deref(),
            )
            .await?;
        }
        Commands::ListEventsForSkater {
            skater,
            competition,
        } => {
            surreal_mind::photography::commands::list_events_for_skater(
                &db,
                &skater,
                competition.as_deref(),
            )
            .await?;
        }
        Commands::CompetitionStats { comp_name } => {
            surreal_mind::photography::commands::competition_stats(&db, &comp_name).await?;
        }
    }

    Ok(())
}
