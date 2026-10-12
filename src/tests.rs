use crate::{http, model::*, process, win};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
#[test]
fn checksum_validation_and_category_paths() {
    assert_eq!(
        crate::integrity::normalize(&"A".repeat(64)).unwrap(),
        "a".repeat(64)
    );
    assert!(crate::integrity::normalize("abcd").is_err());
    assert!(crate::integrity::normalize(&"g".repeat(64)).is_err());
    let base = std::path::Path::new("C:\\Downloads");
    assert_eq!(
        crate::integrity::destination(base, "movie.MP4", true),
        base.join("Videos/movie.MP4")
    );
    assert_eq!(
        crate::integrity::destination(base, "paper.pdf", true),
        base.join("Documents/paper.pdf")
    );
    assert_eq!(
        crate::integrity::destination(base, "future.xyz", true),
        base.join("Other/future.xyz")
    );
    assert_eq!(
        crate::integrity::destination(base, "paper.pdf", false),
        base.join("paper.pdf")
    );
    assert_eq!(
        crate::integrity::destination(&base.join("Videos"), "movie.mp4", true),
        base.join("Videos/movie.mp4")
    );
}
#[test]
fn filename_reservations_follow_windows_case_rules() {
    let engine = Engine::new();
    let first = engine
        .reserve(&engine.data.join("Case-Reservation.bin"))
        .unwrap();
    let second = engine
        .reserve(&engine.data.join("case-reservation.bin"))
        .unwrap();
    assert_ne!(path_key(&first), path_key(&second));
    assert!(second.to_string_lossy().contains("(1)"));
}
#[test]
fn duration_is_readable() {
    assert_eq!(win::eta(65), "1m 5s");
    assert_eq!(win::eta(3660), "1h 1m");
    assert_eq!(win::eta(90000), "1d 1h");
}
#[test]
fn invalid_urls_are_rejected() {
    for u in [
        "file:///C:/Windows/win.ini",
        "javascript:alert(1)",
        "https://example.com/\r\nX: y",
        "http://",
    ] {
        assert!(valid_url(u).is_err(), "{u}")
    }
}
#[test]
fn ranges_are_bounded() {
    assert_eq!(http::range("bytes 10-19/20"), Some((10, 19, 20)));
    for s in [
        "bytes 20-19/30",
        "bytes 0-20/20",
        "bytes 0-0/0",
        "bytes 0-5/*",
    ] {
        assert_eq!(http::range(s), None)
    }
}
#[test]
fn quoting_handles_trailing_slashes() {
    assert_eq!(process::quote("C:\\folder\\"), "\"C:\\folder\\\\\"");
    assert_eq!(process::quote("a\"b"), "\"a\\\"b\"");
}
#[test]
fn rate_change_discards_old_waits() {
    let engine = Engine::new();
    let j = Job::new(Details::default());
    engine.limit_kib.store(1, Ordering::Relaxed);
    let e = engine.clone();
    let job = j.clone();
    let worker = std::thread::spawn(move || e.pace(&job, 256 * 1024));
    std::thread::sleep(Duration::from_millis(30));
    engine.limit_kib.store(512, Ordering::Relaxed);
    worker.join().unwrap();
    let now = Instant::now();
    engine.pace(&j, 1024);
    assert!(now.elapsed() < Duration::from_millis(200));
}
