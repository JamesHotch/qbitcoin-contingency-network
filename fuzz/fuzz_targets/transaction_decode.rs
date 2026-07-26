#![no_main]

use libfuzzer_sys::fuzz_target;
use qbitcoin::transaction::Transaction;

fuzz_target!(|data: &[u8]| {
    if let Ok(transaction) = Transaction::decode(data) {
        assert_eq!(transaction.encode(true), data);
    }
});
