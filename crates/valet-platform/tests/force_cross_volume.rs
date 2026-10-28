//! `VALET_FORCE_CROSS_VOLUME=1` makes `same_volume` answer false. Kept in its
//! own test binary (one test, one process) so setting the variable cannot race
//! with other tests.

#[test]
fn force_cross_volume_env_returns_false() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    assert!(valet_platform::same_volume(&a, &b).unwrap());

    // SAFETY: this binary has a single test and no other threads read the
    // environment while it is changed.
    unsafe { std::env::set_var("VALET_FORCE_CROSS_VOLUME", "1") };
    assert!(!valet_platform::same_volume(&a, &b).unwrap());
    // Paths are not even looked at.
    assert!(!valet_platform::same_volume(&a.join("missing"), &b).unwrap());

    unsafe { std::env::set_var("VALET_FORCE_CROSS_VOLUME", "0") };
    assert!(valet_platform::same_volume(&a, &b).unwrap());
    unsafe { std::env::remove_var("VALET_FORCE_CROSS_VOLUME") };
}
