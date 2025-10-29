use anyhow::Result;
use serde_json::Value;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    let url = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
    let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
    let ns = "photography";
    let dbname = "ops";

    println!("Connecting to SurrealDB at {}...", url);

    // Connect to the running SurrealDB service
    let db = Surreal::new::<Ws>(url).await?;

    // Authenticate
    db.signin(Root {
        username: &user,
        password: &pass,
    })
    .await?;

    // Select namespace and database
    db.use_ns(ns).use_db(dbname).await?;

    println!(
        "Connected successfully to namespace '{}' and database '{}'",
        ns, dbname
    );

    // Define the schema for photography client and competition tracking
    let schema_queries = vec![
        // Namespace and DB are already selected

        // Define client table
        "DEFINE TABLE client SCHEMAFULL PERMISSIONS FULL;",
        "DEFINE FIELD id ON client TYPE string;",
        "DEFINE FIELD first_name ON client TYPE string;",
        "DEFINE FIELD last_name ON client TYPE string;",
        "DEFINE FIELD preferred_name ON client TYPE string;",
        "DEFINE FIELD email ON client TYPE string;",
        "DEFINE FIELD phone ON client TYPE string;",
        "DEFINE FIELD notes ON client TYPE string;",
        "DEFINE FIELD created_at ON client TYPE datetime DEFAULT time::now();",
        // Define family table
        "DEFINE TABLE family SCHEMAFULL PERMISSIONS FULL;",
        "DEFINE FIELD id ON family TYPE string;",
        "DEFINE FIELD name ON family TYPE string;",
        "DEFINE FIELD primary_contact ON family TYPE record<client>;",
        "DEFINE FIELD email ON family TYPE string;",
        "DEFINE FIELD phone ON family TYPE string;",
        "DEFINE FIELD notes ON family TYPE string;",
        // Define competition table
        "DEFINE TABLE competition SCHEMAFULL PERMISSIONS FULL;",
        "DEFINE FIELD id ON competition TYPE string;",
        "DEFINE FIELD name ON competition TYPE string;",
        "DEFINE FIELD venue ON competition TYPE string;",
        "DEFINE FIELD start_date ON competition TYPE datetime;",
        "DEFINE FIELD end_date ON competition TYPE datetime;",
        "DEFINE FIELD notes ON competition TYPE string;",
        // Define event table
        "DEFINE TABLE event SCHEMAFULL PERMISSIONS FULL;",
        "DEFINE FIELD id ON event TYPE string;",
        "DEFINE FIELD competition ON event TYPE record<competition>;",
        "DEFINE FIELD event_number ON event TYPE int;",
        "DEFINE FIELD level ON event TYPE string;",
        "DEFINE FIELD discipline ON event TYPE string;",
        "DEFINE FIELD notes ON event TYPE string;",
        // Define membership relation
        "DEFINE TABLE membership TYPE RELATION FROM client TO family SCHEMAFULL PERMISSIONS FULL;",
        "DEFINE FIELD role ON membership TYPE string DEFAULT 'parent/guardian';",
        "DEFINE FIELD created_at ON membership TYPE datetime DEFAULT time::now();",
        // Define registration relation
        "DEFINE TABLE registration TYPE RELATION FROM client TO event SCHEMAFULL PERMISSIONS FULL;",
        "DEFINE FIELD status ON registration TYPE string ASSERT $value INSIDE ['Unrequested', 'Requested', 'Sent', 'Purchased'];",
        "DEFINE FIELD gallery_url ON registration TYPE string;",
        "DEFINE FIELD gallery_sent_at ON registration TYPE option<datetime>;",
        "DEFINE FIELD purchase_amount ON registration TYPE decimal DEFAULT 0;",
        "DEFINE FIELD notes ON registration TYPE string;",
        // Define shotlog table
        "DEFINE TABLE shotlog SCHEMAFULL PERMISSIONS FULL;",
        "DEFINE FIELD id ON shotlog TYPE string;",
        "DEFINE FIELD event ON shotlog TYPE record<event>;",
        "DEFINE FIELD client ON shotlog TYPE record<client>;",
        "DEFINE FIELD raw_count ON shotlog TYPE int;",
        "DEFINE FIELD picked_count ON shotlog TYPE int;",
        "DEFINE FIELD creative_count ON shotlog TYPE int;",
        "DEFINE FIELD notes ON shotlog TYPE string;",
        "DEFINE FIELD updated_at ON shotlog TYPE datetime DEFAULT time::now();",
    ];

    // Execute each schema query
    for query in schema_queries {
        println!("Executing: {}", query);
        let _resp = db.query(query).await?;
    }

    println!("Schema defined successfully.");

    // Run INFO FOR DB to confirm
    let mut resp = db.query("INFO FOR DB").await?;
    let info_result: Vec<Value> = resp.take(0)?;
    println!("Database info: {:?}", info_result);

    Ok(())
}
