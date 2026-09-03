use std::env;
use std::hint::black_box;
use std::time::Instant;
use windowfilter::{BloomFilter, CuckooFilter, WindowedCuckooFilter};

trait BenchFilter {
    fn insert(&mut self, key: &str) -> bool;
    fn contains(&self, key: &str) -> bool;
    fn memory_bytes(&self) -> usize;
}

impl BenchFilter for BloomFilter {
    fn insert(&mut self, key: &str) -> bool {
        BloomFilter::insert(self, key)
    }
    fn contains(&self, key: &str) -> bool {
        BloomFilter::contains(self, key)
    }
    fn memory_bytes(&self) -> usize {
        BloomFilter::memory_bytes(self)
    }
}

impl BenchFilter for CuckooFilter {
    fn insert(&mut self, key: &str) -> bool {
        CuckooFilter::insert(self, key)
    }
    fn contains(&self, key: &str) -> bool {
        CuckooFilter::contains(self, key)
    }
    fn memory_bytes(&self) -> usize {
        CuckooFilter::memory_bytes(self)
    }
}

impl BenchFilter for WindowedCuckooFilter {
    fn insert(&mut self, key: &str) -> bool {
        WindowedCuckooFilter::insert(self, key)
    }
    fn contains(&self, key: &str) -> bool {
        WindowedCuckooFilter::contains(self, key)
    }
    fn memory_bytes(&self) -> usize {
        WindowedCuckooFilter::memory_bytes(self)
    }
}

struct ResultRow {
    name: &'static str,
    bytes_per_item: f64,
    false_positive_rate: f64,
    lookup_ns: f64,
    insert_ns: f64,
    maximum_load: Option<f64>,
}

fn main() {
    let items = argument("--items", 100_000);
    let queries = argument("--queries", 200_000);
    let fpr_bits = argument("--fpr-bits", 10) as u8;
    assert!(items > 0 && queries > 0);

    let present: Vec<String> = (0..items).map(|i| format!("present-{i}")).collect();
    let absent: Vec<String> = (0..queries).map(|i| format!("absent-{i}")).collect();
    let probe_size = items.clamp(1_000, 10_000);

    let rows = [
        measure(
            "Bloom",
            BloomFilter::new(items, fpr_bits),
            &present,
            &absent,
            None,
        ),
        measure(
            "Cuckoo (2,4)",
            CuckooFilter::new(items, fpr_bits),
            &present,
            &absent,
            Some(cuckoo_maximum_load(probe_size, fpr_bits)),
        ),
        measure(
            "Windowed cuckoo (2,2)",
            WindowedCuckooFilter::new(items, fpr_bits),
            &present,
            &absent,
            Some(windowed_maximum_load(probe_size, fpr_bits)),
        ),
    ];

    println!("items={items}, queries={queries}, target_fpr=2^-{fpr_bits}");
    println!(
        "{:<24} {:>12} {:>14} {:>12} {:>12} {:>12}",
        "filter", "bytes/item", "false-positive", "lookup ns", "insert ns", "max load"
    );
    for row in rows {
        let load = row
            .maximum_load
            .map(|value| format!("{value:.4}"))
            .unwrap_or_else(|| "unbounded".to_owned());
        println!(
            "{:<24} {:>12.4} {:>14.6} {:>12.2} {:>12.2} {:>12}",
            row.name,
            row.bytes_per_item,
            row.false_positive_rate,
            row.lookup_ns,
            row.insert_ns,
            load
        );
    }
}

fn measure<F: BenchFilter>(
    name: &'static str,
    mut filter: F,
    present: &[String],
    absent: &[String],
    maximum_load: Option<f64>,
) -> ResultRow {
    let started = Instant::now();
    for key in present {
        assert!(filter.insert(black_box(key)), "{name} insertion failed");
    }
    let insert_ns = started.elapsed().as_nanos() as f64 / present.len() as f64;

    let started = Instant::now();
    let mut matches = 0_usize;
    for index in 0..absent.len() {
        let key = if index & 1 == 0 {
            &present[index % present.len()]
        } else {
            &absent[index]
        };
        matches += filter.contains(black_box(key)) as usize;
    }
    black_box(matches);
    let lookup_ns = started.elapsed().as_nanos() as f64 / absent.len() as f64;

    let false_positives = absent.iter().filter(|key| filter.contains(key)).count();
    ResultRow {
        name,
        bytes_per_item: filter.memory_bytes() as f64 / present.len() as f64,
        false_positive_rate: false_positives as f64 / absent.len() as f64,
        lookup_ns,
        insert_ns,
        maximum_load,
    }
}

fn cuckoo_maximum_load(capacity: usize, fpr_bits: u8) -> f64 {
    let mut filter = CuckooFilter::new(capacity, fpr_bits).with_max_kicks(2_000);
    for value in 0..filter.slot_count() {
        if !filter.insert(&format!("cuckoo-load-{value}")) {
            break;
        }
    }
    filter.load_factor()
}

fn windowed_maximum_load(capacity: usize, fpr_bits: u8) -> f64 {
    let mut filter = WindowedCuckooFilter::new(capacity, fpr_bits).with_max_kicks(2_000);
    for value in 0..filter.slot_count() {
        if !filter.insert(&format!("windowed-load-{value}")) {
            break;
        }
    }
    filter.load_factor()
}

fn argument(name: &str, default: usize) -> usize {
    let arguments: Vec<String> = env::args().collect();
    arguments
        .windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| {
            pair[1]
                .parse()
                .unwrap_or_else(|_| panic!("invalid value for {name}"))
        })
        .unwrap_or(default)
}
