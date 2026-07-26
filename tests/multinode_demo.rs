#[test]
fn full_progressive_finality_demo_passes() {
    let report = qbitcoin::demo::run_demo().unwrap();
    assert!(report.passed);
    assert_eq!(report.steps.len(), 17);
    assert!(
        report
            .steps
            .iter()
            .all(|step| { step.implementation == "real deterministic protocol execution" })
    );
}
