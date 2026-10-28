//! Criterion micro-bench for `collision::next_free_name`: free destination,
//! one collision, and a run of 100 taken `stem (n).ext` names.
//!
//! Run: `cargo bench -p valet-core --bench collision_next_free_name -- --quick`

use std::collections::HashSet;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use valet_core::collision::next_free_name;

/// `dest` plus `dest (2)` .. `dest (n)`: the next free name is `(n + 1)`.
fn taken_run(dest: &Path, n: u64) -> HashSet<PathBuf> {
    let mut taken = HashSet::new();
    if n == 0 {
        return taken;
    }
    taken.insert(dest.to_path_buf());
    let parent = dest.parent().expect("dest has a parent");
    let stem = dest.file_stem().and_then(|s| s.to_str()).expect("stem");
    let ext = dest.extension().and_then(|s| s.to_str()).expect("ext");
    for i in 2..=n {
        taken.insert(parent.join(format!("{stem} ({i}).{ext}")));
    }
    taken
}

fn bench_next_free_name(c: &mut Criterion) {
    let dest = PathBuf::from("Documente/Facturi/2026/factura_aprilie.pdf");
    let mut group = c.benchmark_group("next_free_name");
    group.sample_size(20);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));
    for taken_count in [0u64, 1, 10, 100] {
        let taken = taken_run(&dest, taken_count);
        let expected = if taken_count == 0 { 1 } else { taken_count + 1 };
        let got = next_free_name(&dest, |p| taken.contains(p));
        if expected > 1 {
            assert!(
                got.to_string_lossy()
                    .ends_with(&format!("({expected}).pdf")),
                "{got:?}"
            );
        } else {
            assert_eq!(got, dest);
        }
        group.bench_with_input(
            BenchmarkId::new("taken", taken_count),
            &taken,
            |b, taken| b.iter(|| next_free_name(black_box(&dest), |p| taken.contains(p))),
        );
    }
    group.finish();
}

criterion_group!(benches, bench_next_free_name);
criterion_main!(benches);
