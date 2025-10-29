use anyhow::Result;
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
    println!("Cleaning up existing test data...\n");

    // Clean up existing data
    let cleanup_queries = vec![
        "REMOVE TABLE competed_in",
        "REMOVE TABLE parent_of",
        "REMOVE TABLE family_member",
        "REMOVE TABLE shotlog",
        "REMOVE TABLE event",
        "REMOVE TABLE skater",
        "REMOVE TABLE client",
        "REMOVE TABLE family",
        "DELETE FROM competition",
    ];
    for query in cleanup_queries {
        let _ = db.query(query).await?;
    }

    println!("Inserting test data...\n");

    // Insert Competition
    println!("Creating competition: 2025 Fall Fling");
    let mut resp = db.query("CREATE comps SET id = '1', name = 'Test'").await?;
    let result: Vec<serde_json::Value> = resp.take(0)?;
    if result.is_empty() {
        println!("Warning: Competition creation failed");
    }

    // Insert Events
    println!("Creating events...");
    let event_queries = vec![
        "CREATE event SET id = 'e10', competition = comps:1, event_number = 10",
        "CREATE event SET id = 'e23', competition = comps:1, event_number = 23",
        "CREATE event SET id = 'e24', competition = comps:1, event_number = 24",
        "CREATE event SET id = 'e31', competition = comps:1, event_number = 31",
        "CREATE event SET id = 'e33', competition = comps:1, event_number = 33",
    ];
    for query in event_queries {
        let mut resp = db.query(query).await?;
        let result: Vec<serde_json::Value> = resp.take(0)?;
        println!("Event insert result: {:?}", result);
        if result.is_empty() {
            println!("Warning: No result returned for query: {}", query);
        }
    }

    // Verify event insertion
    let event_count: Vec<serde_json::Value> = db
        .query("SELECT count() FROM event GROUP ALL")
        .await?
        .take(0)?;
    println!("Event count after insertion: {:?}", event_count);

    let event_list: Vec<serde_json::Value> = db
        .query("SELECT id, event_number FROM event")
        .await?
        .take(0)?;
    println!("Events inserted: {:?}", event_list);

    // Insert Skaters
    println!("Creating skaters...");
    let skater_queries = vec![
        "CREATE skater SET id = 'ruiz_peace_corinne', first_name = 'Corinne', last_name = 'Ruiz Peace', notes = 'Part of Ruiz Peace family delivery unit'",
        "CREATE skater SET id = 'ruiz_peace_cecilia', first_name = 'Cecilia', last_name = 'Ruiz Peace', notes = 'Part of Ruiz Peace family delivery unit'",
        "CREATE skater SET id = 'ruiz_peace_celeste', first_name = 'Celeste', last_name = 'Ruiz Peace', notes = 'Part of Ruiz Peace family delivery unit'",
        "CREATE skater SET id = 'carrico_harlee', first_name = 'Harlee', last_name = 'Carrico', notes = 'VIP skater - individual delivery'",
    ];
    for query in skater_queries {
        let mut resp = db.query(query).await?;
        let result: Vec<serde_json::Value> = resp.take(0)?;
        if result.is_empty() {
            println!("Warning: No result returned for skater query: {}", query);
        }
    }

    // Insert Client
    println!("Creating client (Ruiz Peace family contact)...");
    let mut resp = db
        .query("CREATE client SET id = 'ruiz_peace_parent', first_name = 'Ruiz Peace', last_name = 'Family', email = 'example@example.com', notes = 'Primary contact for Ruiz Peace family'")
        .await?;
    let result: Vec<serde_json::Value> = resp.take(0)?;
    if result.is_empty() {
        println!("Warning: Client creation failed");
    }

    // Insert Family
    println!("Creating family unit...");
    let mut resp = db
            .query("CREATE family SET id = 'ruiz_peace', name = 'Ruiz Peace', primary_contact = client:ruiz_peace_parent, delivery_email = 'example@example.com', notes = '3 skaters - Corinne, Cecilia, Celeste'")
            .await?;
    let result: Vec<serde_json::Value> = resp.take(0)?;
    if result.is_empty() {
        println!("Warning: Family creation failed");
    }

    // Create Relations: parent_of
    println!("Creating parent_of relations...");
    let parent_of_queries = vec![
        "RELATE client:ruiz_peace_parent->parent_of->skater:ruiz_peace_corinne SET relationship = 'parent/guardian'",
        "RELATE client:ruiz_peace_parent->parent_of->skater:ruiz_peace_cecilia SET relationship = 'parent/guardian'",
        "RELATE client:ruiz_peace_parent->parent_of->skater:ruiz_peace_celeste SET relationship = 'parent/guardian'",
    ];
    for query in parent_of_queries {
        let mut resp = db.query(query).await?;
        let result: Vec<serde_json::Value> = resp.take(0)?;
        if result.is_empty() {
            println!("Warning: No result returned for parent_of query: {}", query);
        }
    }

    // Create Relations: family_member
    println!("Creating family_member relations...");
    let family_member_queries = vec![
        "RELATE skater:ruiz_peace_corinne->family_member->family:ruiz_peace",
        "RELATE skater:ruiz_peace_cecilia->family_member->family:ruiz_peace",
        "RELATE skater:ruiz_peace_celeste->family_member->family:ruiz_peace",
    ];
    for query in family_member_queries {
        let mut resp = db.query(query).await?;
        let result: Vec<serde_json::Value> = resp.take(0)?;
        if result.is_empty() {
            println!(
                "Warning: No result returned for family_member query: {}",
                query
            );
        }
    }

    // Create Relations: competed_in
    println!("Creating competed_in relations...");
    let competed_in_queries = vec![
        "RELATE skater:ruiz_peace_corinne->competed_in->event:e10 SET skate_order = 2, request_status = 'requested', gallery_status = 'pending', notes = 'SignUp: TRUE'",
        "RELATE skater:ruiz_peace_cecilia->competed_in->event:e23 SET skate_order = 5, request_status = 'requested', gallery_status = 'pending', notes = 'SignUp: TRUE'",
        "RELATE skater:ruiz_peace_cecilia->competed_in->event:e33_z SET skate_order = 3, request_status = 'requested', gallery_status = 'pending', notes = 'SignUp: TRUE'",
        "RELATE skater:ruiz_peace_celeste->competed_in->event:e31_l SET skate_order = 2, request_status = 'requested', gallery_status = 'pending', notes = 'SignUp: TRUE'",
        "RELATE skater:carrico_harlee->competed_in->event:e24 SET skate_order = 6, request_status = 'vip', gallery_status = 'pending', notes = 'VIP - documented in brain file'",
    ];
    for query in competed_in_queries {
        let mut resp = db.query(query).await?;
        let result: Vec<serde_json::Value> = resp.take(0)?;
        if result.is_empty() {
            println!(
                "Warning: No result returned for competed_in query: {}",
                query
            );
        }
    }

    // Verify competed_in insertion
    let competed_in_count: Vec<serde_json::Value> = db
        .query("SELECT count() FROM competed_in GROUP ALL")
        .await?
        .take(0)?;
    println!("Competed_in count after insertion: {:?}", competed_in_count);

    // Insert Shotlog entries
    println!("Creating shotlog entries...");
    let shotlog_queries = vec![
        "CREATE shotlog SET id = 'ruiz_peace_corinne_e10', skater = skater:ruiz_peace_corinne, event = event:e10, raw_count = 0, picked_count = 0, borderline_count = 0, creative_count = 0, notes = 'Awaiting culling'",
        "CREATE shotlog SET id = 'carrico_harlee_e24', skater = skater:carrico_harlee, event = event:e24, raw_count = 0, picked_count = 0, borderline_count = 0, creative_count = 0, notes = 'Awaiting culling'",
    ];
    for query in shotlog_queries {
        let mut resp = db.query(query).await?;
        let result: Vec<serde_json::Value> = resp.take(0)?;
        if result.is_empty() {
            println!("Warning: No result returned for shotlog query: {}", query);
        }
    }

    println!("\n✅ Test data inserted successfully!");
    println!("\nTest dataset includes:");
    println!("  - 1 competition (2025 Fall Fling)");
    println!("  - 5 events (10, 23, 24, 31-L, 33-Z)");
    println!("  - 4 skaters (3 Ruiz Peace family + Carrico)");
    println!("  - 1 client (Ruiz Peace parent)");
    println!("  - 1 family unit (Ruiz Peace)");
    println!("  - 3 parent_of relations");
    println!("  - 3 family_member relations");
    println!("  - 5 competed_in relations");
    println!("  - 2 shotlog entries");

    Ok(())
}
