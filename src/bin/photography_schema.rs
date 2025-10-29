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
        "DEFINE TABLE client SCHEMAFULL PERMISSIONS FOR select, create, update FULL;",
        "DEFINE FIELD first_name ON client TYPE string;",
        "DEFINE FIELD last_name ON client TYPE string;",
        "DEFINE FIELD preferred_name ON client TYPE option<string>;",
        "DEFINE FIELD email ON client TYPE string;",
        "DEFINE FIELD phone ON client TYPE option<string>;",
        "DEFINE FIELD notes ON client TYPE option<string>;",
        "DEFINE FIELD created_at ON client TYPE datetime DEFAULT time::now();",
        "DEFINE TABLE skater SCHEMAFULL PERMISSIONS FOR select, create, update FULL;",
        "DEFINE FIELD first_name ON skater TYPE string;",
        "DEFINE FIELD last_name ON skater TYPE string;",
        "DEFINE FIELD birth_date ON skater TYPE option<datetime>;",
        "DEFINE FIELD notes ON skater TYPE option<string>;",
        "DEFINE FIELD created_at ON skater TYPE datetime DEFAULT time::now();",
        "DEFINE TABLE family SCHEMAFULL PERMISSIONS FOR select, create, update FULL;",
        "DEFINE FIELD name ON family TYPE string;",
        "DEFINE FIELD primary_contact ON family TYPE record<client>;",
        "DEFINE FIELD delivery_email ON family TYPE string;",
        "DEFINE FIELD notes ON family TYPE option<string>;",
        "DEFINE TABLE competition SCHEMAFULL PERMISSIONS FOR select, create, update FULL;",
        "DEFINE FIELD name ON competition TYPE string;",
        "DEFINE FIELD venue ON competition TYPE string;",
        "DEFINE FIELD start_date ON competition TYPE option<datetime>;",
        "DEFINE FIELD end_date ON competition TYPE option<datetime>;",
        "DEFINE FIELD notes ON competition TYPE option<string>;",
        "DEFINE TABLE event SCHEMAFULL PERMISSIONS FOR select, create, update FULL;",
        "DEFINE FIELD competition ON event TYPE record<competition>;",
        "DEFINE FIELD event_number ON event TYPE int;",
        "DEFINE FIELD split_ice ON event TYPE option<string> ASSERT $value == NONE OR $value INSIDE ['L', 'Z'];",
        "DEFINE FIELD level ON event TYPE option<string>;",
        "DEFINE FIELD discipline ON event TYPE option<string>;",
        "DEFINE FIELD time_slot ON event TYPE option<string>;",
        "DEFINE FIELD notes ON event TYPE option<string>;",
        "DEFINE TABLE shotlog SCHEMAFULL PERMISSIONS FOR select, create, update FULL;",
        "DEFINE FIELD skater ON shotlog TYPE record<skater>;",
        "DEFINE FIELD event ON shotlog TYPE record<event>;",
        "DEFINE FIELD raw_count ON shotlog TYPE int DEFAULT 0;",
        "DEFINE FIELD picked_count ON shotlog TYPE int DEFAULT 0;",
        "DEFINE FIELD borderline_count ON shotlog TYPE int DEFAULT 0;",
        "DEFINE FIELD creative_count ON shotlog TYPE int DEFAULT 0;",
        "DEFINE FIELD notes ON shotlog TYPE option<string>;",
        "DEFINE FIELD updated_at ON shotlog TYPE datetime DEFAULT time::now();",
        "DEFINE TABLE parent_of TYPE RELATION FROM client TO skater SCHEMAFULL PERMISSIONS FOR select, create, update FULL;",
        "DEFINE FIELD relationship ON parent_of TYPE string DEFAULT 'parent/guardian';",
        "DEFINE FIELD created_at ON parent_of TYPE datetime DEFAULT time::now();",
        "DEFINE TABLE family_member TYPE RELATION FROM skater TO family SCHEMAFULL PERMISSIONS FOR select, create, update FULL;",
        "DEFINE FIELD created_at ON family_member TYPE datetime DEFAULT time::now();",
        "DEFINE TABLE competed_in TYPE RELATION FROM skater TO event SCHEMAFULL PERMISSIONS FOR select, create, update FULL;",
        "DEFINE FIELD skate_order ON competed_in TYPE option<int>;",
        "DEFINE FIELD request_status ON competed_in TYPE string DEFAULT 'unrequested' ASSERT $value INSIDE ['requested', 'vip', 'unrequested'];",
        "DEFINE FIELD gallery_status ON competed_in TYPE string DEFAULT 'pending' ASSERT $value INSIDE ['pending', 'culling', 'processing', 'sent', 'purchased'];",
        "DEFINE FIELD gallery_url ON competed_in TYPE option<string>;",
        "DEFINE FIELD gallery_sent_at ON competed_in TYPE option<datetime>;",
        "DEFINE FIELD purchase_amount ON competed_in TYPE option<float>;",
        "DEFINE FIELD purchase_date ON competed_in TYPE option<datetime>;",
        "DEFINE FIELD notes ON competed_in TYPE option<string>;",
        "DEFINE FIELD created_at ON competed_in TYPE datetime DEFAULT time::now();",
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
