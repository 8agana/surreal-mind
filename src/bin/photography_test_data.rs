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
    let _ = db.query("CREATE competition SET id = 'fall_fling_2025', name = '2025 Fall Fling', venue = 'Line Creek Community Center', start_date = d'2025-10-25T10:00:00Z', end_date = d'2025-10-25T18:35:00Z', notes = 'First production test of photography database'").await?;

    // Insert Events
    println!("Creating events...");
    let event_queries = vec![
        "CREATE event SET id = 'e10', competition = competition:fall_fling_2025, event_number = 10, time_slot = '1:15-1:40', notes = 'Corinne Ruiz Peace'",
        "CREATE event SET id = 'e23', competition = competition:fall_fling_2025, event_number = 23, time_slot = '3:25-3:55', notes = 'Cecilia Ruiz Peace'",
        "CREATE event SET id = 'e24', competition = competition:fall_fling_2025, event_number = 24, time_slot = '3:25-3:55', notes = 'Harlee Carrico'",
        "CREATE event SET id = 'e31_l', competition = competition:fall_fling_2025, event_number = 31, split_ice = 'L', time_slot = '5:15-5:30', notes = 'Celeste Ruiz Peace - Line Creek ice'",
        "CREATE event SET id = 'e33_z', competition = competition:fall_fling_2025, event_number = 33, split_ice = 'Z', time_slot = '5:15-5:30', notes = 'Cecilia Ruiz Peace - Zamboni ice'",
    ];
    for query in event_queries {
        let _ = db.query(query).await?;
    }

    // Insert Skaters
    println!("Creating skaters...");
    let skater_queries = vec![
        "CREATE skater SET id = 'ruiz_peace_corinne', first_name = 'Corinne', last_name = 'Ruiz Peace', notes = 'Part of Ruiz Peace family delivery unit'",
        "CREATE skater SET id = 'ruiz_peace_cecilia', first_name = 'Cecilia', last_name = 'Ruiz Peace', notes = 'Part of Ruiz Peace family delivery unit'",
        "CREATE skater SET id = 'ruiz_peace_celeste', first_name = 'Celeste', last_name = 'Ruiz Peace', notes = 'Part of Ruiz Peace family delivery unit'",
        "CREATE skater SET id = 'carrico_harlee', first_name = 'Harlee', last_name = 'Carrico', notes = 'VIP skater - individual delivery'",
    ];
    for query in skater_queries {
        let _ = db.query(query).await?;
    }

    // Insert Client
    println!("Creating client (Ruiz Peace family contact)...");
    let _ = db
        .query("CREATE client SET id = 'ruiz_peace_parent', first_name = 'Ruiz Peace', last_name = 'Family', email = 'example@example.com', notes = 'Primary contact for Ruiz Peace family'")
        .await?;

    // Insert Family
    println!("Creating family unit...");
    let _ = db
            .query("CREATE family SET id = 'ruiz_peace', name = 'Ruiz Peace', primary_contact = client:ruiz_peace_parent, delivery_email = 'example@example.com', notes = '3 skaters - Corinne, Cecilia, Celeste'")
            .await?;

    // Create Relations: parent_of
    println!("Creating parent_of relations...");
    let parent_of_queries = vec![
        "RELATE client:ruiz_peace_parent->parent_of->skater:ruiz_peace_corinne SET relationship = 'parent/guardian'",
        "RELATE client:ruiz_peace_parent->parent_of->skater:ruiz_peace_cecilia SET relationship = 'parent/guardian'",
        "RELATE client:ruiz_peace_parent->parent_of->skater:ruiz_peace_celeste SET relationship = 'parent/guardian'",
    ];
    for query in parent_of_queries {
        let _ = db.query(query).await?;
    }

    // Create Relations: family_member
    println!("Creating family_member relations...");
    let family_member_queries = vec![
        "RELATE skater:ruiz_peace_corinne->family_member->family:ruiz_peace",
        "RELATE skater:ruiz_peace_cecilia->family_member->family:ruiz_peace",
        "RELATE skater:ruiz_peace_celeste->family_member->family:ruiz_peace",
    ];
    for query in family_member_queries {
        let _ = db.query(query).await?;
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
        let _ = db.query(query).await?;
    }

    // Insert Shotlog entries
    println!("Creating shotlog entries...");
    let shotlog_queries = vec![
        "CREATE shotlog SET id = 'ruiz_peace_corinne_e10', skater = skater:ruiz_peace_corinne, event = event:e10, raw_count = 0, picked_count = 0, borderline_count = 0, creative_count = 0, notes = 'Awaiting culling'",
        "CREATE shotlog SET id = 'carrico_harlee_e24', skater = skater:carrico_harlee, event = event:e24, raw_count = 0, picked_count = 0, borderline_count = 0, creative_count = 0, notes = 'Awaiting culling'",
    ];
    for query in shotlog_queries {
        let _ = db.query(query).await?;
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
