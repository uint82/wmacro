//! end-to-end smoke test against the live ScreenCast portal.

use super::*;
use crate::image_utils::outputs::query_outputs;

#[test]
#[ignore = "requires a Wayland session and a granted ScreenCast session"]
fn capture_smoke() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let outputs = query_outputs().expect("query_outputs failed");
    log::info!("outputs: {outputs:?}");

    let capturer = Capturer::new().expect("Capturer::new failed");
    log::info!("matched output: {:?}", capturer.output());

    let t0 = std::time::Instant::now();
    let all = capturer.capture_all().expect("capture_all failed");
    log::info!("capture_all: {}x{} (took {:.2}ms)", all.width(), all.height(), t0.elapsed().as_secs_f64() * 1000.0);
    assert!(all.width() > 0 && all.height() > 0);

    let t1 = std::time::Instant::now();
    let region = capturer
        .capture_region(0, 0, 64, 48)
        .expect("capture_region failed");
    log::info!("capture_region(0,0,64,48): {}x{} (took {:.2}ms)", region.width(), region.height(), t1.elapsed().as_secs_f64() * 1000.0);
    assert!(region.width() > 0 && region.height() > 0);

    let t2 = std::time::Instant::now();
    let region = capturer
        .capture_region(960, 540, 200, 120)
        .expect("capture_region failed");
    log::info!("capture_region(960,540,200,120): {}x{} (took {:.2}ms)", region.width(), region.height(), t2.elapsed().as_secs_f64() * 1000.0);
    assert_eq!(region.width(), 200);
    assert_eq!(region.height(), 120);

    std::thread::sleep(std::time::Duration::from_secs(4));

    let t3 = std::time::Instant::now();
    let again = capturer.capture_all().expect("second capture_all failed");
    log::info!("capture_all after 4s: {}x{} (took {:.2}ms)", again.width(), again.height(), t3.elapsed().as_secs_f64() * 1000.0);
    assert!(again.width() > 0 && again.height() > 0);

    let t4 = std::time::Instant::now();
    let region2 = capturer
        .capture_region(0, 0, 64, 48)
        .expect("second capture_region failed");
    log::info!("capture_region after 4s: {}x{} (took {:.2}ms)", region2.width(), region2.height(), t4.elapsed().as_secs_f64() * 1000.0);
    assert!(region2.width() > 0 && region2.height() > 0);

    let mut hot_times = Vec::new();
    for _ in 0..5 {
        let t = std::time::Instant::now();
        let hot = capturer
            .capture_region(0, 0, 64, 48)
            .expect("hot capture_region failed");
        hot_times.push(t.elapsed().as_secs_f64() * 1000.0);
        assert!(hot.width() > 0 && hot.height() > 0);
    }
    log::info!(
        "hot loop region captures: {}",
        hot_times.iter().map(|t| format!("{t:.2}ms")).collect::<Vec<_>>().join(", ")
    );
}
