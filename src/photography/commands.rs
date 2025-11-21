use super::models::*;
use super::utils::*;
use anyhow::Result;
use csv;
use prettytable::{Table, row};
use serde_json::Value;
use std::fs::File;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client;

/// Import roster from CSV for a competition
pub async fn import_roster(db: &Surreal<Client>, competition: &str, file_path: &str) -> Result<()> {
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

        // If family or has email, upsert family
        let family_id = if parsed.is_family || row.email.is_some() {
            let family_id = format!(
                "family:{}",
                parsed.skaters[0].last_name.to_lowercase().replace(" ", "_")
            );
            let email = row.email.clone().unwrap_or_default();
            let family_resp = db
                .query(
                    "INSERT INTO family (id, name, first_name, last_name, delivery_email, created_at)
                     VALUES ($id, string::concat('Family ', $last), 'Family', $last, $email, time::now())
                     ON DUPLICATE KEY UPDATE first_name = 'Family', last_name = $last, delivery_email = $email, name = string::concat('Family ', $last)",
                )
                .bind(("id", family_id.clone()))
                .bind(("last", parsed.skaters[0].last_name.clone()))
                .bind(("email", email))
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

                // Create family_competition relation
                let fam_comp_resp = db
                    .query(
                        "RELATE (type::thing('family', $family_id))->family_competition->(type::thing('competition', $comp_id))
                         SET created_at = time::now(), gallery_status = (gallery_status OR 'pending')",
                    )
                    .bind(("family_id", family_id.clone()))
                    .bind(("comp_id", comp_id.clone()))
                    .await?;
                fam_comp_resp.check()?;
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

/// Mark a gallery as SENT for a specific competition
pub async fn mark_sent(db: &Surreal<Client>, last_name: &str, comp: &str) -> Result<()> {
    let family_id_full = format_family_id(last_name);
    let family_id_only = last_name.to_lowercase().replace(" ", "_");
    let competition_id_only = comp.to_lowercase().replace(" ", "_");

    // 1. Check existence explicitly using raw SQL to avoid SDK "Table Name" confusion
    let check_sql = r#"SELECT * FROM type::thing('family', $id)"#;
    let mut check_resp = db
        .query(check_sql)
        .bind(("id", family_id_only.clone()))
        .await?;
    let check: Vec<Family> = check_resp.take(0)?;

    if check.is_empty() {
        println!("❌ Error: Family {} not found.", family_id_full);
        return Ok(());
    }

    // 2. Delete existing edges then create new one (avoiding duplicates)
    println!(
        "Marking SENT: {} -> {}",
        family_id_full, competition_id_only
    );
    let delete_sql = "
        DELETE family_competition
        WHERE in = type::thing('family', $family_id)
        AND out = type::thing('competition', $competition_id)
    ";
    let _ = db
        .query(delete_sql)
        .bind(("family_id", family_id_only.clone()))
        .bind(("competition_id", competition_id_only.clone()))
        .await?;

    let sql = "
        RELATE (type::thing('family', $family_id))->family_competition->(type::thing('competition', $competition_id))
        SET gallery_status = 'sent', sent_date = time::now()
    ";
    let _ = db
        .query(sql)
        .bind(("family_id", family_id_only))
        .bind(("competition_id", competition_id_only))
        .await?;
    println!("✅ Marked as sent.");
    Ok(())
}

/// Request a Thank You gallery for a family
pub async fn request_ty(db: &Surreal<Client>, last_name: &str, comp: &str) -> Result<()> {
    let family_id_full = format_family_id(last_name);
    let family_id_only = last_name.to_lowercase().replace(" ", "_");
    let competition_id_only = comp.to_lowercase().replace(" ", "_");

    // Check existence
    let check_sql = r#"SELECT * FROM type::thing('family', $id)"#;
    let mut check_resp = db
        .query(check_sql)
        .bind(("id", family_id_only.clone()))
        .await?;
    let check: Vec<Family> = check_resp.take(0)?;
    if check.is_empty() {
        println!("❌ Error: Family {} not found.", family_id_full);
        return Ok(());
    }

    println!(
        "Requesting TY: {} -> {}",
        family_id_full, competition_id_only
    );
    let delete_sql = "
        DELETE family_competition
        WHERE in = type::thing('family', $family_id)
        AND out = type::thing('competition', $competition_id)
    ";
    let _ = db
        .query(delete_sql)
        .bind(("family_id", family_id_only.clone()))
        .bind(("competition_id", competition_id_only.clone()))
        .await?;

    let sql = "
        RELATE (type::thing('family', $family_id))->family_competition->(type::thing('competition', $competition_id))
        SET ty_requested = true
    ";
    let _ = db
        .query(sql)
        .bind(("family_id", family_id_only))
        .bind(("competition_id", competition_id_only))
        .await?;
    println!("✅ TY requested.");
    Ok(())
}

/// Send a Thank You gallery (sets ty_sent=true, timestamps)
pub async fn send_ty(db: &Surreal<Client>, last_name: &str, comp: &str) -> Result<()> {
    let family_id_full = format_family_id(last_name);
    let family_id_only = last_name.to_lowercase().replace(" ", "_");
    let competition_id_only = comp.to_lowercase().replace(" ", "_");

    // Check existence
    let check_sql = r#"SELECT * FROM type::thing('family', $id)"#;
    let mut check_resp = db
        .query(check_sql)
        .bind(("id", family_id_only.clone()))
        .await?;
    let check: Vec<Family> = check_resp.take(0)?;
    if check.is_empty() {
        println!("❌ Error: Family {} not found.", family_id_full);
        return Ok(());
    }

    println!("Sending TY: {} -> {}", family_id_full, competition_id_only);
    let delete_sql = "
        DELETE family_competition
        WHERE in = type::thing('family', $family_id)
        AND out = type::thing('competition', $competition_id)
    ";
    let _ = db
        .query(delete_sql)
        .bind(("family_id", family_id_only.clone()))
        .bind(("competition_id", competition_id_only.clone()))
        .await?;

    let sql = "
        RELATE (type::thing('family', $family_id))->family_competition->(type::thing('competition', $competition_id))
        SET ty_sent = true, ty_sent_date = time::now()
    ";
    let _ = db
        .query(sql)
        .bind(("family_id", family_id_only))
        .bind(("competition_id", competition_id_only))
        .await?;
    println!("✅ TY sent.");
    Ok(())
}

/// Check delivery status for a competition
pub async fn check_status(
    db: &Surreal<Client>,
    comp_name: &str,
    pending_only: bool,
    ty_pending: bool,
    status_filter: Option<&str>,
) -> Result<()> {
    let comp_name_lower = comp_name.to_lowercase();
    let mut sql = String::from(
        r#"SELECT in.last_name as family_name,
                in.email as email,
                request_status,
                gallery_status,
                sent_date,
                ty_requested,
                ty_sent,
                ty_sent_date
                FROM family_competition
                WHERE out.name CONTAINS $comp"#,
    );
    if pending_only {
        sql.push_str(" AND gallery_status = 'pending'");
    }
    if ty_pending {
        sql.push_str(" AND ty_requested = true AND ty_sent = false");
    }
    if let Some(stat) = status_filter {
        sql.push_str(&format!(" AND request_status = '{}'", stat));
    }
    sql.push_str(" ORDER BY in.last_name");

    let mut resp = db.query(sql).bind(("comp", comp_name_lower)).await?;
    let statuses: Vec<StatusRow> = resp.take(0)?;

    if statuses.is_empty() {
        println!("No families found for competition '{}'.", comp_name);
        return Ok(());
    }

    let mut table = Table::new();
    table.add_row(row![
        "Family Name",
        "Email",
        "Req Status",
        "Gal Status",
        "Sent Date",
        "TY Req",
        "TY Sent",
        "TY Sent Date"
    ]);
    for s in statuses {
        table.add_row(row![
            s.family_name,
            s.email.unwrap_or_default(),
            s.request_status.unwrap_or_default(),
            s.gallery_status.unwrap_or_default(),
            s.sent_date.unwrap_or_default(),
            s.ty_requested.unwrap_or(false),
            s.ty_sent.unwrap_or(false),
            s.ty_sent_date.unwrap_or_default(),
        ]);
    }
    table.printstd();
    Ok(())
}

/// Record a purchase for a family
pub async fn record_purchase(
    db: &Surreal<Client>,
    last_name: &str,
    amount: f64,
    comp: &str,
) -> Result<()> {
    let family_id_full = format_family_id(last_name);
    let family_id_only = last_name.to_lowercase().replace(" ", "_");
    let competition_id_only = comp.to_lowercase().replace(" ", "_");

    // Check existence using raw SQL
    let check_sql = r#"SELECT * FROM type::thing('family', $id)"#;
    let mut check_resp = db
        .query(check_sql)
        .bind(("id", family_id_only.clone()))
        .await?;
    let check: Vec<Family> = check_resp.take(0)?;
    if check.is_empty() {
        println!("❌ Error: Family {} not found.", family_id_full);
        return Ok(());
    }

    println!(
        "Recording purchase: {} -> ${} for {}",
        family_id_full, amount, competition_id_only
    );
    let delete_sql = "
        DELETE family_competition
        WHERE in = type::thing('family', $family_id)
        AND out = type::thing('competition', $competition_id)
    ";
    let _ = db
        .query(delete_sql)
        .bind(("family_id", family_id_only.clone()))
        .bind(("competition_id", competition_id_only.clone()))
        .await?;

    let sql = "
        RELATE (type::thing('family', $family_id))->family_competition->(type::thing('competition', $competition_id))
        SET purchase_amount = $amount, gallery_status = 'purchased'
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

/// List skaters by status
pub async fn list_skaters(db: &Surreal<Client>, status: &str) -> Result<()> {
    let mut resp = if status == "all" {
        db.query(
            "SELECT
                in.first_name AS first_name,
                in.last_name AS last_name,
                out.event_number AS event_number,
                out.split_ice AS split_ice,
                out.request_status AS request_status,
                out.gallery_status AS gallery_status
                FROM competed_in
                FETCH out.event_number, out.split_ice",
        )
        .await?
    } else {
        db.query(
            "SELECT
                in.first_name AS first_name,
                in.last_name AS last_name,
                out.event_number AS event_number,
                out.split_ice AS split_ice,
                out.request_status AS request_status,
                out.gallery_status AS gallery_status
                FROM competed_in
                WHERE out.request_status = $status
                FETCH out.event_number, out.split_ice",
        )
        .bind(("status", status.to_string()))
        .await?
    };

    let skaters: Vec<SkaterRow> = resp.take(0)?;

    let mut table = Table::new();
    table.add_row(row![
        "First Name",
        "Last Name",
        "Event Num",
        "Split Ice",
        "Req Status",
        "Gal Status"
    ]);
    for s in skaters {
        table.add_row(row![
            s.first_name,
            s.last_name,
            s.event_num.unwrap_or(0),
            s.comp_name.unwrap_or_default(),
            s.req_status.unwrap_or_default(),
            s.gal_status.unwrap_or_default(),
        ]);
    }
    table.printstd();
    Ok(())
}

/// List events for competition
pub async fn list_events(db: &Surreal<Client>, competition: &str) -> Result<()> {
    let mut resp = db
        .query(
            "SELECT event_number, split_ice, level, discipline, time_slot
             FROM event
             WHERE competition = type::thing('competition', $comp)",
        )
        .bind(("comp", competition_to_id(competition)))
        .await?;
    let events: Vec<Value> = resp.take(0)?;
    for event in events {
        if let Some(obj) = event.as_object() {
            println!(
                "Event {}: {} - {} at {}",
                obj.get("event_number").unwrap_or(&Value::Null),
                obj.get("level").unwrap_or(&Value::Null),
                obj.get("discipline").unwrap_or(&Value::Null),
                obj.get("time_slot").unwrap_or(&Value::Null)
            );
        }
    }
    Ok(())
}

/// Show event details
pub async fn show_event(
    db: &Surreal<Client>,
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
            "SELECT event_number, split_ice, level, discipline, time_slot, notes FROM event WHERE event_number = {} AND split_ice IS NONE",
            event_number
        )
    };
    let mut resp = db.query(&query).await?;
    let events: Vec<Value> = resp.take(0)?;
    for event in events {
        if let Some(obj) = event.as_object() {
            println!(
                "Event {}: Split: {}, Level: {}, Discipline: {}, Time: {}, Notes: {}",
                obj.get("event_number").unwrap_or(&Value::Null),
                obj.get("split_ice").unwrap_or(&Value::Null),
                obj.get("level").unwrap_or(&Value::Null),
                obj.get("discipline").unwrap_or(&Value::Null),
                obj.get("time_slot").unwrap_or(&Value::Null),
                obj.get("notes").unwrap_or(&Value::Null),
            );
        }
    }
    Ok(())
}

/// Update gallery status
pub async fn update_gallery(
    db: &Surreal<Client>,
    skater: &str,
    event: u32,
    status: &str,
    url: Option<&str>,
    amount: Option<f64>,
) -> Result<()> {
    // Parse skater names
    let parsed = parse_skater_names(skater)?;
    if parsed.skaters.len() != 1 {
        println!("Error: Expect exactly one skater for update.");
        return Ok(());
    }
    let skater = &parsed.skaters[0];

    // Find the relation
    let skater_id = format!(
        "{}_{}",
        skater.last_name.to_lowercase(),
        skater.first_name.to_lowercase()
    )
    .replace('-', "_");

    let query = format!(
        "SELECT meta::id(id) as id FROM competed_in WHERE in = type::thing('skater', '{}') AND out.event_number = {}",
        skater_id, event
    );
    let mut resp = db.query(&query).await?;
    let results: Vec<Value> = resp.take(0)?;
    if results.is_empty() {
        println!("No such skater/event combination.");
        return Ok(());
    }
    let relation_id = results[0].get("id").unwrap().as_str().unwrap();

    // Update
    let mut update_sql = format!("UPDATE {} SET gallery_status = '{}'", relation_id, status);
    if status == "sent" {
        if let Some(u) = url {
            update_sql.push_str(&format!(", gallery_url = '{}'", u));
        }
        update_sql.push_str(", sent_date = time::now()");
    }
    if status == "purchased" {
        if let Some(a) = amount {
            update_sql.push_str(&format!(", purchase_amount = {}", a));
        }
    }
    let _ = db.query(&update_sql).await?;
    println!("Updated gallery status to {}.", status);
    Ok(())
}

/// Query skater details including status
pub async fn query_skater(db: &Surreal<Client>, last_name: &str) -> Result<()> {
    println!("Searching for skater: {}", last_name);

    let sql = "
        SELECT
            first_name,
            last_name,
            array::first(->competed_in->event->competition.name) as comp_name,
            array::first(->competed_in->event.event_number) as event_num,
            array::first(->competed_in->event.split_ice) as split_ice,
            array::first(->competed_in->event.time_slot) as time_slot,
            array::first(->competed_in.out.request_status) as req_status,
            array::first(->competed_in.out.gallery_status) as gal_status,
            array::first(->competed_in.out.sent_date) as sent_date,
            array::first(->competed_in.out.purchase_amount) as purchase_amount
        FROM skater
        WHERE last_name = $last_name
    ";

    let mut resp = db
        .query(sql)
        .bind(("last_name", last_name.to_string()))
        .await?;
    let results: Vec<SkaterRow> = resp.take(0)?;

    if results.is_empty() {
        println!("No skaters found with last name '{}'.", last_name);
        return Ok(());
    }

    let mut table = Table::new();
    table.add_row(row![
        "First", "Last", "Comp", "Event", "Split", "Time", "Req", "Gal", "Sent", "Purchase"
    ]);
    for s in results {
        table.add_row(row![
            s.first_name,
            s.last_name,
            s.comp_name.clone().unwrap_or_default(),
            s.event_num.unwrap_or(0),
            s.split_ice.clone().unwrap_or_default(),
            s.time_slot.clone().unwrap_or_default(),
            s.req_status.clone().unwrap_or_default(),
            s.gal_status.clone().unwrap_or_default(),
            s.sent_date.clone().unwrap_or_default(),
            s.purchase_amount.unwrap_or(0.0),
        ]);
    }
    table.printstd();
    Ok(())
}

/// Get family contact email for gallery delivery
pub async fn get_email(db: &Surreal<Client>, last_name: &str) -> Result<()> {
    let family_id_str = format_family_id(last_name);

    // Use db.select to get the family record directly by its formatted ID
    let family: Vec<Family> = db.select(&family_id_str).await?;

    if !family.is_empty() {
        let f = &family[0];
        println!(
            "Family: {}\nEmail: {}\n",
            f.last_name,
            f.email.as_ref().unwrap_or(&"No email on file".to_string())
        );
    } else {
        println!("No family found with last name '{}'.", last_name);
    }
    Ok(())
}

/// List events for skater
pub async fn list_events_for_skater(
    db: &Surreal<Client>,
    last_name: &str,
    comp: Option<&str>,
) -> Result<()> {
    let query = if let Some(c) = comp {
        format!(
            "SELECT out.competition.name, out.event_number, out.split_ice, request_status, gallery_status, purchase_amount
             FROM competed_in
             WHERE in.last_name = '{}' AND out.competition.name CONTAINS '{}'
             FETCH out, out.competition",
            last_name, c
        )
    } else {
        format!(
            "SELECT out.competition.name, out.event_number, out.split_ice, request_status, gallery_status, purchase_amount
             FROM competed_in
             WHERE in.last_name = '{}'
             FETCH out, out.competition",
            last_name
        )
    };
    let mut resp = db.query(&query).await?;
    let results: Vec<Value> = resp.take(0)?;
    for result in results {
        if let Some(obj) = result.as_object() {
            println!(
                "Competition: {}, Event: {}, Split: {}, Req: {}, Gal: {}, Purchase: {}",
                obj.get("out.competition.name").unwrap_or(&Value::Null),
                obj.get("out.event_number").unwrap_or(&Value::Null),
                obj.get("out.split_ice").unwrap_or(&Value::Null),
                obj.get("request_status").unwrap_or(&Value::Null),
                obj.get("gallery_status").unwrap_or(&Value::Null),
                obj.get("purchase_amount").unwrap_or(&Value::Null),
            );
        }
    }
    Ok(())
}

/// Show competition statistics
pub async fn competition_stats(db: &Surreal<Client>, comp_name: &str) -> Result<()> {
    let lower_comp = comp_name.to_lowercase();
    let event_condition = format!("string::lowercase(out.competition.name OR '') CONTAINS '{}'", lower_comp);
    let family_condition = format!("string::lowercase(out.name OR '') CONTAINS '{}'", lower_comp);

    // Total distinct skaters
    let mut total_skaters_resp = db
        .query(format!(
            "RETURN array::len(array::distinct((SELECT VALUE in FROM competed_in WHERE {})))",
            event_condition
        ))
        .await?;
    let total_skaters: Option<Value> = total_skaters_resp.take(0)?;
    println!(
        "Total distinct skaters: {}",
        total_skaters.unwrap_or(Value::from(0))
    );

    // Total families
    let mut total_families_resp = db
        .query(format!(
            "RETURN array::len(array::distinct((SELECT VALUE in FROM family_competition WHERE {})))",
            family_condition
        ))
        .await?;
    let total_families: Option<Value> = total_families_resp.take(0)?;
    println!(
        "Total families: {}",
        total_families.unwrap_or(Value::from(0))
    );

    // Status breakdown
    let mut status_resp = db
        .query(format!(
            "SELECT request_status, count() as count FROM competed_in WHERE {} GROUP BY request_status FETCH out.competition",
            event_condition
        ))
        .await?;
    let statuses: Vec<Value> = status_resp.take(0)?;
    println!("\nRequest Status Breakdown:");
    for stat in statuses {
        if let Some(obj) = stat.as_object() {
            println!(
                "  {}: {}",
                obj.get("request_status").unwrap_or(&Value::Null),
                obj.get("count").unwrap_or(&Value::Null)
            );
        }
    }

    // Gallery status
    let mut gal_resp = db
        .query(format!(
            "SELECT gallery_status, count() as count FROM competed_in WHERE {} GROUP BY gallery_status FETCH out.competition",
            event_condition
        ))
        .await?;
    let galleries: Vec<Value> = gal_resp.take(0)?;
    println!("\nGallery Status Breakdown:");
    for gal in galleries {
        if let Some(obj) = gal.as_object() {
            println!(
                "  {}: {}",
                obj.get("gallery_status").unwrap_or(&Value::Null),
                obj.get("count").unwrap_or(&Value::Null)
            );
        }
    }

    Ok(())
}

pub async fn set_status(
    db: &Surreal<Client>,
    last_name: &str,
    comp: &str,
    status: &str,
) -> Result<()> {
    let valid_statuses = [
        "pending",
        "culling",
        "processing",
        "sent",
        "purchased",
        "not_shot",
        "needs_research",
    ];

    if !valid_statuses.contains(&status) {
        println!(
            "❌ Error: Invalid status '{}'. Valid statuses are: {}",
            status,
            valid_statuses.join(", ")
        );
        return Ok(());
    }

    let family_id_full = format_family_id(last_name);
    let family_id_only = last_name.to_lowercase().replace(" ", "_");
    let competition_id_only = comp.to_lowercase().replace(" ", "_");

    // Check family exists
    let check_sql = r#"SELECT * FROM type::thing('family', $id)"#;
    let mut check_resp = db
        .query(check_sql)
        .bind(("id", family_id_only.clone()))
        .await?;
    let check: Vec<Family> = check_resp.take(0)?;
    if check.is_empty() {
        println!("❌ Error: Family {} not found.", family_id_full);
        return Ok(());
    }

    // DELETE+RELATE pattern
    println!(
        "Setting status to '{}': {} -> {}",
        status, family_id_full, competition_id_only
    );
    let delete_sql = "
        DELETE family_competition
        WHERE in = type::thing('family', $family_id)
        AND out = type::thing('competition', $competition_id)
    ";
    let _ = db
        .query(delete_sql)
        .bind(("family_id", family_id_only.clone()))
        .bind(("competition_id", competition_id_only.clone()))
        .await?;

    let sql = "
        RELATE (type::thing('family', $family_id))->family_competition->(type::thing('competition', $competition_id))
        SET gallery_status = $status
    ";
    let _ = db
        .query(sql)
        .bind(("family_id", family_id_only))
        .bind(("competition_id", competition_id_only))
        .bind(("status", status.to_string()))
        .await?;
    println!("✅ Status set to '{}'.", status);
    Ok(())
}
