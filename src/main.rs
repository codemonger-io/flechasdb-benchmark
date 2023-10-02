use anyhow::{Context as _, Error, bail};
use clap::{Parser, Subcommand};
use core::ops::{AddAssign, Div, Mul, Sub};
use futures::future::try_join_all;
use rand::Rng;
use serde::Serialize;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use tokio::runtime::Runtime;

use flechasdb::asyncdb::{
    io::{LocalFileSystem as AsyncLocalFileSystem},
    stored::{Database as AsyncDatabase, LoadDatabase as _},
};
use flechasdb::db::AttributeValue;
use flechasdb::db::build::{
    DatabaseBuilder,
    proto::serialize_database,
};
use flechasdb::db::stored::{self, LoadDatabase as _};
use flechasdb::io::LocalFileSystem;
use flechasdb::linalg::{dot, subtract, sum};
use flechasdb::nbest::NBestByKey;
use flechasdb::numbers::{FromAs, Sqrt, Zero};
use flechasdb::vector::BlockVectorSet;

use flechasdb_benchmark::sift::read_fvecs_file;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Builds the database.
    Build {
        /// Path to the dataset (*.fvecs file).
        dataset_path: String,
        /// Path to the folder where to save the database.
        output_path: String,
        /// Number of partitions.
        #[arg(short = 'p', long, default_value_t = 2048)]
        num_partitions: usize,
        /// Number of subvector divisions.
        #[arg(short = 'd', long, default_value_t = 8)]
        num_divisions: usize,
        /// Number of clusters (codes).
        #[arg(short = 'c', long, default_value_t = 256)]
        num_codes: usize,
    },
    /// Queries the database with a single query vector.
    Query {
        /// Path to the dataset (*.fvecs file).
        dataset_path: String,
        /// Path to the database file.
        database_path: String,
        /// Path to the query vectorset (*.fvecs file).
        queries_path: String,
        /// Index of the query to evaluate.
        /// Randomly chosen if omitted.
        #[arg(short, long)]
        query_index: Option<usize>,
        /// Number of best matches (k-nearest neighbors) to return.
        #[arg(short, long, default_value_t = 100)]
        k: usize,
        /// Number of partitions to search in.
        #[arg(short = 'p', long, default_value_t = 10)]
        nprobe: usize,
    },
    /// Queries the database with every query vector.
    Batch {
        /// Path to the dataset (*.fvecs file).
        dataset_path: String,
        /// Path to the database file.
        database_path: String,
        /// Path to the query vectorset (*.fvecs file).
        queries_path: String,
        /// Number of best matches (k-nearest neighbors) to return.
        #[arg(short, long, default_value_t = 100)]
        k: usize,
        /// Number of partitions to search in.
        #[arg(short = 'p', long, default_value_t = 10)]
        nprobe: usize,
        /// Output path of the statistics.
        #[arg(short, long)]
        stats_path: Option<String>,
        /// Limits the number of queries.
        #[arg(short, long)]
        limit: Option<usize>,
        /// Whether asynchronously executed.
        #[arg(short, long)]
        r#async: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Build {
            dataset_path,
            output_path,
            num_partitions,
            num_divisions,
            num_codes,
        } => do_build(
            dataset_path,
            output_path,
            num_partitions,
            num_divisions,
            num_codes,
        ),
        Command::Query {
            dataset_path,
            database_path,
            queries_path,
            query_index,
            k,
            nprobe,
        } => do_query(
            dataset_path,
            database_path,
            queries_path,
            query_index,
            k,
            nprobe,
        ),
        Command::Batch {
            dataset_path,
            database_path,
            queries_path,
            k,
            nprobe,
            stats_path,
            limit,
            r#async,
        } => {
            if r#async {
                do_batch_async(
                    dataset_path,
                    database_path,
                    queries_path,
                    k,
                    nprobe,
                    limit,
                    stats_path,
                )
            } else {
                do_batch(
                    dataset_path,
                    database_path,
                    queries_path,
                    k,
                    nprobe,
                    limit,
                    stats_path,
                )
            }
        },
    }.unwrap();
}

fn do_build(
    dataset_path: String,
    output_path: String,
    num_partitions: usize,
    num_divisions: usize,
    num_codes: usize,
) -> Result<(), Error> {
    println!("loading dataset: {}", dataset_path);
    let vs = read_fvecs_file(&dataset_path)
        .context(format!("failed to load dataset: {}", dataset_path))?;
    println!("vector size: {}", vs.vector_size());
    println!("number of vectors: {}", vs.len());
    println!("number of partitions: {}", num_partitions);
    println!("number of divisions: {}", num_divisions);
    println!("number of codes: {}", num_codes);
    let time = std::time::Instant::now();
    let event_time = std::time::Instant::now();
    let mut db = DatabaseBuilder::new(vs)
        .with_partitions(num_partitions.try_into()?)
        .with_divisions(num_divisions.try_into()?)
        .with_clusters(num_codes.try_into()?)
        .build_with_events(move |event| println!(
            "{:?} at {} s",
            event,
            event_time.elapsed().as_secs_f32(),
        ))
        .context("failed to build database")?;
    println!("built database in {} s", time.elapsed().as_secs_f32());
    println!("assigning vector indices (datum_id)");
    let time = std::time::Instant::now();
    for i in 0..db.num_vectors() {
        db.set_attribute_at(i, ("datum_id", i as u64))?;
    }
    println!("assigned vector indices in {} s", time.elapsed().as_secs_f32());
    println!("saving database: {}", output_path);
    let time = std::time::Instant::now();
    serialize_database(&db, &mut LocalFileSystem::new(&output_path))
        .context(format!("failed to save database: {}", output_path))?;
    println!("saved database in {} s", time.elapsed().as_secs_f32());
    Ok(())
}

fn do_query(
    dataset_path: String,
    database_path: String,
    queries_path: String,
    query_index: Option<usize>,
    k: usize,
    nprobe: usize,
) -> Result<(), Error> {
    println!("loading dataset: {}", dataset_path);
    let time = std::time::Instant::now();
    let vs = read_fvecs_file(&dataset_path)
        .context(format!("failed to load dataset: {}", dataset_path))?;
    println!("loaded dataset in {} s", time.elapsed().as_secs_f32());
    println!("loading database: {}", database_path);
    let time = std::time::Instant::now();
    let database_path = Path::new(&database_path);
    let db = stored::Database::<f32, _>::load_database(
        LocalFileSystem::new(database_path.parent().unwrap()),
        database_path.file_name().unwrap().to_str().unwrap(),
    ).context(format!("failed to load database: {:?}", database_path))?;
    println!("loaded database in {} s", time.elapsed().as_secs_f32());
    println!("loading query vectors: {}", queries_path);
    let qvs = read_fvecs_file(&queries_path)
        .context(format!("failed to read query vectors: {}", queries_path))?;
    let query_index = match query_index {
        Some(i) => i,
        None => {
            let mut rng = rand::thread_rng();
            rng.gen_range(0..qvs.len())
        },
    };
    println!("query vector index: {}", query_index);
    if query_index >= qvs.len() {
        bail!("query index out of bounds: {} ≥ {}", query_index, qvs.len());
    }
    let qv = qvs.get(query_index);
    println!("k: {}", k);
    println!("nprobe: {}", nprobe);
    let time = std::time::Instant::now();
    let event_time = std::time::Instant::now();
    let results = db.query_with_events(
        qv,
        k.try_into()?,
        nprobe.try_into()?,
        move |event| println!(
            "{:?} at {} s",
            event,
            event_time.elapsed().as_secs_f32(),
        ),
    )?;
    let results = results
        .into_iter()
        .map(|result| {
            result.get_attribute("datum_id")
                .and_then(|value| value.ok_or(
                    flechasdb::error::Error::InvalidData(
                        format!("missing datum_id"),
                    ),
                ))
                .and_then(|v| match *v {
                    AttributeValue::Uint64(v) => Ok(v as usize),
                    _ => Err(flechasdb::error::Error::InvalidData(format!(
                        "datum_id is not a u64 but {:?}",
                        v,
                    ))),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    println!("queried k-NN in {} s", time.elapsed().as_secs_f32());
    println!("selected datum IDs: {:?}", results);
    let time = std::time::Instant::now();
    let flat_results = flat_query(&vs, qv, k);
    println!("flat-queried k-NN in {} s", time.elapsed().as_secs_f32());
    // evaluates recalls
    let recall = results
        .iter()
        .map(|i| if flat_results.contains(i) { 1 } else { 0 })
        .sum::<usize>();
    println!(
        "recall: {}/{} ({:.0}%)",
        recall,
        k,
        recall as f32 / k as f32 * 100.0f32,
    );
    Ok(())
}

fn do_batch(
    dataset_path: String,
    database_path: String,
    queries_path: String,
    k: usize,
    nprobe: usize,
    limit: Option<usize>,
    stats_path: Option<String>,
) -> Result<(), Error> {
    println!("loading dataset: {}", dataset_path);
    let time = std::time::Instant::now();
    let vs = read_fvecs_file(&dataset_path)
        .context(format!("failed to load dataset: {}", dataset_path))?;
    println!("loaded dataset in {} s", time.elapsed().as_secs_f32());
    println!("loading database: {}", database_path);
    let time = std::time::Instant::now();
    let database_path = Path::new(&database_path);
    let db = stored::Database::<f32, _>::load_database(
        LocalFileSystem::new(database_path.parent().unwrap()),
        database_path.file_name().unwrap().to_str().unwrap(),
    ).context(format!("failed to load database: {:?}", database_path))?;
    println!("loaded database in {} s", time.elapsed().as_secs_f32());
    println!("loading query vectors: {}", queries_path);
    let qvs = read_fvecs_file(&queries_path)
        .context(format!("failed to read query vectors: {}", queries_path))?;
    let mut stats = QueryStatsRecorder::new(k, nprobe);
    let num_queries = limit
        .map(|n| std::cmp::min(n, qvs.len()))
        .unwrap_or(qvs.len());
    for qi in 0..num_queries {
        if qi % 100 == 0 {
            println!("processing query vector:\t{}/{}", qi, num_queries);
        }
        let qv = qvs.get(qi);
        // indexed query
        let time = std::time::Instant::now();
        let results = db.query(qv, k.try_into()?, nprobe.try_into()?)?;
        let results = results
            .into_iter()
            .map(|result| {
                result.get_attribute("datum_id")
                    .and_then(|value| value.ok_or(
                        flechasdb::error::Error::InvalidData(
                            format!("missing datum_id"),
                        ),
                    ))
                    .and_then(|v| match *v {
                        AttributeValue::Uint64(v) => Ok(v as usize),
                        _ => Err(flechasdb::error::Error::InvalidData(format!(
                            "datum_id is not a u64 but {:?}",
                            v,
                        ))),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let query_time = time.elapsed().as_secs_f64();
        // flat query
        let time = std::time::Instant::now();
        let flat_results = flat_query(&vs, qv, k);
        let flat_query_time = time.elapsed().as_secs_f64();
        // records stats
        let recall = calculate_recall(&flat_results, &results);
        stats.add_record(query_time, flat_query_time, recall);
    }
    println!("Statistics");
    println!("k: {}", k);
    println!("nprobe: {}", nprobe);
    let stats = stats.finish();
    let time_unit: f64 = 1_000.0; // s → ms
    println!(
        "indexed time (ms): {:.3}±{:.3}, median={:.3}, q1={:.3}, q3={:.3}, min={:.3}, max={:.3}",
        stats.seconds.mean * time_unit,
        stats.seconds.std * time_unit,
        stats.seconds.median * time_unit,
        stats.seconds.q1 * time_unit,
        stats.seconds.q3 * time_unit,
        stats.seconds.min * time_unit,
        stats.seconds.max * time_unit,
    );
    println!(
        "flat time (ms): {:.3}±{:.3}, median={:.3}, q1={:.3}, q3={:.3}, min={:.3}, max={:.3}",
        stats.flat_seconds.mean * time_unit,
        stats.flat_seconds.std * time_unit,
        stats.flat_seconds.median * time_unit,
        stats.flat_seconds.q1 * time_unit,
        stats.flat_seconds.q3 * time_unit,
        stats.flat_seconds.min * time_unit,
        stats.flat_seconds.max * time_unit,
    );
    println!(
        "recall (%): {:.1}±{:.1}, median={:.1}, q1={:.1}, q3={:.1}, min={:.1}, max={:.1}",
        stats.recalls.mean * 100.0,
        stats.recalls.std * 100.0,
        stats.recalls.median * 100.0,
        stats.recalls.q1 * 100.0,
        stats.recalls.q3 * 100.0,
        stats.recalls.min * 100.0,
        stats.recalls.max * 100.0,
    );
    if let Some(stats_path) = stats_path.as_ref() {
        println!("saving stats: {}", stats_path);
        let file = File::create(stats_path)
            .context(format!("failed to create stats file: {}", stats_path))?;
        serde_json::to_writer_pretty(BufWriter::new(file), &stats)
            .context(format!("failed to write stats to file: {}", stats_path))?;
    }
    Ok(())
}

fn do_batch_async(
    dataset_path: String,
    database_path: String,
    queries_path: String,
    k: usize,
    nprobe: usize,
    limit: Option<usize>,
    stats_path: Option<String>,
) -> Result<(), Error> {
    println!("loading dataset: {}", dataset_path);
    let time = std::time::Instant::now();
    let vs = read_fvecs_file(&dataset_path)
        .context(format!("failed to load dataset: {}", dataset_path))?;
    println!("loaded dataset in {} s", time.elapsed().as_secs_f32());
    println!("loading query vectors: {}", queries_path);
    let qvs = read_fvecs_file(&queries_path)
        .context(format!("failed to read query vectors: {}", queries_path))?;
    let rt = Runtime::new()?;
    let stats = rt.block_on(_do_batch_async(
        database_path,
        k,
        nprobe,
        limit,
        vs,
        qvs,
    ))?;
    println!("Statistics");
    println!("k: {}", k);
    println!("nprobe: {}", nprobe);
    let time_unit: f64 = 1_000.0; // s → ms
    println!(
        "indexed time (ms): {:.3}±{:.3}, median={:.3}, q1={:.3}, q3={:.3}, min={:.3}, max={:.3}",
        stats.seconds.mean * time_unit,
        stats.seconds.std * time_unit,
        stats.seconds.median * time_unit,
        stats.seconds.q1 * time_unit,
        stats.seconds.q3 * time_unit,
        stats.seconds.min * time_unit,
        stats.seconds.max * time_unit,
    );
    println!(
        "flat time (ms): {:.3}±{:.3}, median={:.3}, q1={:.3}, q3={:.3}, min={:.3}, max={:.3}",
        stats.flat_seconds.mean * time_unit,
        stats.flat_seconds.std * time_unit,
        stats.flat_seconds.median * time_unit,
        stats.flat_seconds.q1 * time_unit,
        stats.flat_seconds.q3 * time_unit,
        stats.flat_seconds.min * time_unit,
        stats.flat_seconds.max * time_unit,
    );
    println!(
        "recall (%): {:.1}±{:.1}, median={:.1}, q1={:.1}, q3={:.1}, min={:.1}, max={:.1}",
        stats.recalls.mean * 100.0,
        stats.recalls.std * 100.0,
        stats.recalls.median * 100.0,
        stats.recalls.q1 * 100.0,
        stats.recalls.q3 * 100.0,
        stats.recalls.min * 100.0,
        stats.recalls.max * 100.0,
    );
    if let Some(stats_path) = stats_path.as_ref() {
        println!("saving stats: {}", stats_path);
        let file = File::create(stats_path)
            .context(format!("failed to create stats file: {}", stats_path))?;
        serde_json::to_writer_pretty(BufWriter::new(file), &stats)
            .context(format!("failed to write stats to file: {}", stats_path))?;
    }
    Ok(())
}

async fn _do_batch_async(
    database_path: String,
    k: usize,
    nprobe: usize,
    limit: Option<usize>,
    vs: BlockVectorSet<f32>,
    qvs: BlockVectorSet<f32>,
) -> Result<QueryStats, Error> {
    println!("loading database: {}", database_path);
    let time = std::time::Instant::now();
    let database_path = Path::new(&database_path);
    let db = AsyncDatabase::<f32, _>::load_database(
        AsyncLocalFileSystem::new(database_path.parent().unwrap()),
        database_path.file_name().unwrap().to_str().unwrap(),
    )
        .await
        .context(format!("failed to load database: {:?}", database_path))?;
    println!("loaded database in {} s", time.elapsed().as_secs_f32());
    let mut stats = QueryStatsRecorder::new(k, nprobe);
    let num_queries = limit
        .map(|n| std::cmp::min(n, qvs.len()))
        .unwrap_or(qvs.len());
    for qi in 0..num_queries {
        if qi % 100 == 0 {
            println!("processing query vector:\t{}/{}", qi, num_queries);
        }
        let qv = qvs.get(qi);
        // indexed query
        let time = std::time::Instant::now();
        let results = db.query(qv, k.try_into()?, nprobe.try_into()?).await?;
        let results = try_join_all(results
            .into_iter()
            .map(|result| async move {
                result.get_attribute("datum_id").await
                    .and_then(|value| value.ok_or(
                        flechasdb::error::Error::InvalidData(
                            format!("missing datum_id"),
                        ),
                    ))
                    .and_then(|v| match v {
                        AttributeValue::Uint64(v) => Ok(v as usize),
                        _ => Err(flechasdb::error::Error::InvalidData(format!(
                            "datum_id is not a u64 but {:?}",
                            v,
                        ))),
                    })
            }),
        ).await?;
        let query_time = time.elapsed().as_secs_f64();
        // flat query
        let time = std::time::Instant::now();
        let flat_results = flat_query(&vs, qv, k);
        let flat_query_time = time.elapsed().as_secs_f64();
        // records stats
        let recall = calculate_recall(&flat_results, &results);
        stats.add_record(query_time, flat_query_time, recall);
    }
    Ok(stats.finish())
}

// Quries in a given flat table.
fn flat_query(vs: &BlockVectorSet<f32>, qv: &[f32], k: usize) -> Vec<usize> {
    let mut distances: NBestByKey<(usize, f32), f32, _> =
        NBestByKey::new(k, |t: &(usize, f32)| t.1);
    let mut buf: Vec<f32> = Vec::with_capacity(vs.vector_size());
    unsafe { buf.set_len(vs.vector_size()); }
    for i in 0..vs.len() {
        subtract(vs.get(i), qv, &mut buf);
        distances.push((i, dot(&buf, &buf)));
    }
    distances.sort_by(|l, r| l.1.partial_cmp(&r.1).unwrap());
    distances
        .into_iter()
        .map(|(i, _)| i)
        .collect()
}

// Calculates the recall.
fn calculate_recall<T>(reference_results: &Vec<T>, results: &Vec<T>) -> f32
where
    T: PartialEq<T>,
{
    assert_eq!(reference_results.len(), results.len());
    let recall: f32 = results
        .iter()
        .map(|i| if reference_results.contains(i) { 1.0f32 } else { 0.0f32 })
        .sum();
    recall / results.len() as f32
}

// Recorder of statistics on queries.
struct QueryStatsRecorder {
    k: usize,
    nprobe: usize,
    seconds: Vec<f64>,
    flat_seconds: Vec<f64>,
    recalls: Vec<f32>,
}

impl QueryStatsRecorder {
    fn new(k: usize, nprobe: usize) -> Self {
        Self {
            k,
            nprobe,
            seconds: Vec::with_capacity(10_000),
            flat_seconds: Vec::with_capacity(10_000),
            recalls: Vec::with_capacity(10_000),
        }
    }

    fn add_record(&mut self, seconds: f64, flat_seconds: f64, recall: f32) {
        self.seconds.push(seconds);
        self.flat_seconds.push(flat_seconds);
        self.recalls.push(recall);
    }

    fn finish(self) -> QueryStats {
        QueryStats {
            k: self.k,
            nprobe: self.nprobe,
            num_queries: self.seconds.len(),
            seconds: Stats::compute(self.seconds),
            flat_seconds: Stats::compute(self.flat_seconds),
            recalls: Stats::compute(self.recalls),
        }
    }
}

// Statistics on queries.
#[derive(Debug, Serialize)]
struct QueryStats {
    k: usize,
    nprobe: usize,
    num_queries: usize,
    seconds: Stats<f64>,
    flat_seconds: Stats<f64>,
    recalls: Stats<f32>,
}

// Generic statistics.
#[derive(Debug, Serialize)]
struct Stats<T> {
    mean: T,
    std: T,
    median: T,
    min: T,
    max: T,
    q1: T,
    q3: T,
}

impl<T> Stats<T> {
    fn compute(mut records: Vec<T>) -> Stats<T>
    where
        T: FromAs<usize>
            + Sqrt
            + Zero
            + AddAssign
            + Div<Output = T>
            + Mul<Output = T>
            + Sub<Output = T>
            + Copy
            + PartialOrd,
    {
        records.sort_by(|l, r| l.partial_cmp(r).unwrap());
        let sum = sum(&records);
        let mean = sum / T::from_as(records.len());
        let squared_sum = dot(&records, &records);
        let var = (squared_sum - T::from_as(records.len()) * mean * mean) / T::from_as(records.len() - 1);
        Stats {
            mean: sum / T::from_as(records.len()),
            std: var.sqrt(),
            median: records[records.len() / 2],
            min: records[0],
            max: records[records.len() - 1],
            q1: records[records.len() / 4],
            q3: records[records.len() * 3 / 4],
        }
    }
}
