use chrono::NaiveDate;
use firestore::*;
use futures::TryStreamExt;

use crate::app::Checkpoint;

pub async fn connect() -> FirestoreResult<FirestoreDb> {
    FirestoreDb::with_options(
        FirestoreDbOptions::new("double-vehicle-452318-e4".into())
            .with_database_id("tcheater".into()),
    )
    .await
}

pub async fn find_checkpoints(
    db: &FirestoreDb,
    day: &NaiveDate,
) -> FirestoreResult<Vec<Checkpoint>> {
    // Calculate start and end of today in UTC
    let start_of_day = day.and_hms_opt(0, 0, 0).unwrap();
    let end_of_day = day.and_hms_opt(23, 59, 59).unwrap();

    let stream = db
        .fluent()
        .select()
        .from("checkpoints")
        .filter(|q| {
            q.for_all([
                q.field(path!(Checkpoint::time))
                    .greater_than_or_equal(start_of_day),
                q.field(path!(Checkpoint::time))
                    .less_than_or_equal(end_of_day),
            ])
        })
        .order_by([(path!(Checkpoint::time), FirestoreQueryDirection::Ascending)])
        .obj()
        .stream_query_with_errors()
        .await?;
    stream.try_collect().await
}

pub async fn insert_checkpoint(db: &FirestoreDb) -> FirestoreResult<Checkpoint> {
    let checkpoint = Checkpoint::new();
    db.fluent()
        .insert()
        .into("checkpoints")
        .document_id(String::new())
        .object(&checkpoint)
        .execute()
        .await
}

pub async fn update_checkpoint(db: &FirestoreDb, ch: &Checkpoint) -> FirestoreResult<Checkpoint> {
    db.fluent()
        .update()
        .fields(vec![
            path!(Checkpoint::time),
            path!(Checkpoint::project),
            path!(Checkpoint::message),
            path!(Checkpoint::registered),
        ])
        .in_col("checkpoints")
        .document_id(ch.id.as_ref().unwrap())
        .object(ch)
        .execute()
        .await
}

pub async fn delete_checkpoint(db: &FirestoreDb, ch: &Checkpoint) -> FirestoreResult<()> {
    db.fluent()
        .delete()
        .from("checkpoints")
        .document_id(ch.id.as_ref().unwrap())
        .execute()
        .await
}

pub async fn find_distinct_dates(db: &FirestoreDb) -> FirestoreResult<Vec<chrono::NaiveDate>> {
    let stream = db
        .fluent()
        .select()
        .from("checkpoints")
        .order_by([(path!(Checkpoint::time), FirestoreQueryDirection::Ascending)])
        .obj()
        .stream_query_with_errors()
        .await?;

    let checkpoints: Vec<Checkpoint> = stream.try_collect().await?;

    let mut dates: Vec<chrono::NaiveDate> = checkpoints
        .iter()
        .map(|checkpoint| checkpoint.time.date_naive())
        .collect();

    dates.sort();
    dates.dedup();

    Ok(dates)
}
