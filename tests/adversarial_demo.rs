#[test]
fn adversarial_consensus_demo_passes() {
    let report = qbitcoin::demo::run_attack_demo().unwrap();
    assert!(report.passed);
    assert_eq!(report.steps.len(), 13);
    assert_eq!(
        report
            .steps
            .iter()
            .filter(|step| step.implementation == "bounded simulation")
            .count(),
        1
    );
}
