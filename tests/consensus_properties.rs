use proptest::prelude::*;
use qbitcoin::{
    amount::{Amount, MAX_MONEY_ATOMS},
    finality::ConsensusStage,
    hash::Hash32,
    params::{ChainParams, Network},
    transaction::{OutPoint, OutputPolicy, SpendWitness, Transaction, TxInput, TxOutput},
};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn canonical_transactions_round_trip_for_arbitrary_amounts(
        atoms in 0_u64..=MAX_MONEY_ATOMS,
        index in any::<u32>(),
        lock_time in any::<u32>(),
    ) {
        let params = ChainParams::for_network(Network::Regtest);
        let mut transaction = Transaction::new(
            params.chain_id,
            vec![TxInput {
                previous_output: OutPoint {
                    txid: Hash32([7; 32]),
                    index,
                },
                sequence: u32::MAX,
                witness: SpendWitness::Empty,
            }],
            vec![TxOutput {
                value: Amount::from_atoms(atoms).unwrap(),
                policy: OutputPolicy::Burn {
                    commitment: Hash32([8; 32]),
                },
            }],
        );
        transaction.lock_time = u64::from(lock_time);
        let bytes = transaction.encode(true);
        prop_assert_eq!(Transaction::decode(&bytes).unwrap(), transaction);
    }

    #[test]
    fn checked_amount_addition_never_wraps(
        left in 0_u64..=MAX_MONEY_ATOMS,
        right in 0_u64..=MAX_MONEY_ATOMS,
    ) {
        let left = Amount::from_atoms(left).unwrap();
        let right = Amount::from_atoms(right).unwrap();
        match left.checked_add(right) {
            Ok(sum) => prop_assert!(sum.atoms() <= MAX_MONEY_ATOMS),
            Err(_) => prop_assert!(left.atoms().saturating_add(right.atoms()) > MAX_MONEY_ATOMS),
        }
    }
}

#[test]
fn consensus_stage_successors_are_strictly_forward() {
    for stage in [
        ConsensusStage::BootstrapFinality,
        ConsensusStage::ReducedFinality,
        ConsensusStage::ObserverOnly,
    ] {
        assert!(stage.next().unwrap() > stage);
    }
    assert_eq!(ConsensusStage::Retired.next(), None);
}
