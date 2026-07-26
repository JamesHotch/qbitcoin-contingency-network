use crate::{
    address::{AddressError, decode_address, encode_address, policy_commitment},
    amount::{Amount, AmountError},
    crypto::{CryptoError, DomainTag, HybridKeyPair, HybridPublicKeys, HybridSecretMaterial},
    params::{ChainParams, Network},
    transaction::{OutPoint, OutputPolicy, SpendWitness, Transaction, TxInput, TxOutput},
};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Write,
    path::Path,
};
use thiserror::Error;
use zeroize::Zeroize;

const WALLET_FORMAT_VERSION: u16 = 1;
const KDF_MEMORY_KIB: u32 = 65_536;
const KDF_ITERATIONS: u32 = 3;
const KDF_PARALLELISM: u32 = 1;

#[derive(Debug, Error)]
pub enum WalletError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Address(#[from] AddressError),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    #[error(transparent)]
    Amount(#[from] AmountError),
    #[error("wallet password is incorrect or the wallet is corrupt")]
    Decryption,
    #[error("wallet encryption failed")]
    Encryption,
    #[error("wallet format or KDF parameters are unsupported")]
    UnsupportedFormat,
    #[error("wallet belongs to another network")]
    WrongNetwork,
    #[error("wallet has insufficient confirmed funds")]
    InsufficientFunds,
    #[error("coin-control selection references an unavailable output")]
    CoinControl,
    #[error("wallet key index is invalid")]
    MissingKey,
}

#[derive(Serialize, Deserialize)]
struct WalletKeyRecord {
    label: String,
    public_keys: HybridPublicKeys,
    secret: HybridSecretMaterial,
}

#[derive(Serialize, Deserialize)]
pub struct Wallet {
    format_version: u16,
    network: Network,
    keys: Vec<WalletKeyRecord>,
    address_book: BTreeMap<String, String>,
    history: Vec<WalletHistoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletHistoryEntry {
    pub txid: crate::hash::Hash32,
    pub direction: String,
    pub amount: Amount,
    pub block_height: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletUtxo {
    pub outpoint: OutPoint,
    pub output: TxOutput,
    pub created_height: u64,
    pub confirmations: u64,
    pub coinbase: bool,
    pub key_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletSummary {
    pub network: Network,
    pub addresses: Vec<String>,
    pub address_book_entries: usize,
    pub history_entries: usize,
    pub recovery_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedWallet {
    format_version: u16,
    network: Network,
    kdf_memory_kib: u32,
    kdf_iterations: u32,
    kdf_parallelism: u32,
    salt_base64: String,
    nonce_base64: String,
    ciphertext_base64: String,
}

impl Wallet {
    #[must_use]
    pub fn create(params: &ChainParams, recovery_relative_timelock: Option<u32>) -> Self {
        let key_pair = HybridKeyPair::generate(recovery_relative_timelock);
        Self {
            format_version: WALLET_FORMAT_VERSION,
            network: params.network,
            keys: vec![WalletKeyRecord {
                label: "primary".to_owned(),
                public_keys: key_pair.public_keys(),
                secret: key_pair.export_secret_material(),
            }],
            address_book: BTreeMap::new(),
            history: Vec::new(),
        }
    }

    pub fn summary(&self, params: &ChainParams) -> Result<WalletSummary, WalletError> {
        self.ensure_network(params)?;
        Ok(WalletSummary {
            network: self.network,
            addresses: self
                .keys
                .iter()
                .map(|key| encode_address(params, &key.public_keys))
                .collect::<Result<_, _>>()?,
            address_book_entries: self.address_book.len(),
            history_entries: self.history.len(),
            recovery_enabled: self
                .keys
                .iter()
                .any(|key| key.public_keys.recovery.is_some()),
        })
    }

    pub fn primary_address(&self, params: &ChainParams) -> Result<String, WalletError> {
        self.ensure_network(params)?;
        let key = self.keys.first().ok_or(WalletError::MissingKey)?;
        Ok(encode_address(params, &key.public_keys)?)
    }

    pub fn primary_public_keys(
        &self,
        params: &ChainParams,
    ) -> Result<HybridPublicKeys, WalletError> {
        self.ensure_network(params)?;
        Ok(self
            .keys
            .first()
            .ok_or(WalletError::MissingKey)?
            .public_keys
            .clone())
    }

    pub fn add_address_book_entry(&mut self, label: String, address: String) {
        self.address_book.insert(label, address);
    }

    #[must_use]
    pub fn address_book(&self) -> &BTreeMap<String, String> {
        &self.address_book
    }

    #[must_use]
    pub fn history(&self) -> &[WalletHistoryEntry] {
        &self.history
    }

    pub fn record_history(&mut self, entry: WalletHistoryEntry) {
        self.history.push(entry);
    }

    pub fn balance(&self, utxos: &[WalletUtxo]) -> Result<Amount, WalletError> {
        utxos.iter().try_fold(Amount::ZERO, |sum, utxo| {
            if utxo.key_index >= self.keys.len() {
                return Err(WalletError::MissingKey);
            }
            sum.checked_add(utxo.output.value).map_err(Into::into)
        })
    }

    pub fn create_payment(
        &self,
        params: &ChainParams,
        available: &[WalletUtxo],
        recipient_address: &str,
        amount: Amount,
        fee: Amount,
        coin_control: Option<&BTreeSet<OutPoint>>,
    ) -> Result<Transaction, WalletError> {
        self.ensure_network(params)?;
        let recipient_commitment = decode_address(params, recipient_address)?;
        let target = amount.checked_add(fee)?;
        let mut candidates: Vec<&WalletUtxo> = available
            .iter()
            .filter(|utxo| coin_control.is_none_or(|selected| selected.contains(&utxo.outpoint)))
            .collect();
        candidates.sort_by_key(|utxo| (utxo.output.value, utxo.outpoint));
        if let Some(selected) = coin_control
            && candidates.len() != selected.len()
        {
            return Err(WalletError::CoinControl);
        }

        let mut selected = Vec::new();
        let mut input_total = Amount::ZERO;
        for candidate in candidates {
            if candidate.key_index >= self.keys.len() {
                return Err(WalletError::MissingKey);
            }
            if candidate.coinbase && candidate.confirmations < params.coinbase_maturity {
                continue;
            }
            input_total = input_total.checked_add(candidate.output.value)?;
            selected.push(candidate);
            if input_total >= target {
                break;
            }
        }
        if input_total < target {
            return Err(WalletError::InsufficientFunds);
        }

        let mut outputs = vec![TxOutput {
            value: amount,
            policy: OutputPolicy::HybridCommitment {
                commitment: recipient_commitment,
            },
        }];
        let change = input_total.checked_sub(target)?;
        if change != Amount::ZERO {
            let primary = self.keys.first().ok_or(WalletError::MissingKey)?;
            outputs.push(TxOutput {
                value: change,
                policy: OutputPolicy::HybridCommitment {
                    commitment: policy_commitment(&primary.public_keys),
                },
            });
        }
        let inputs = selected
            .iter()
            .map(|utxo| TxInput {
                previous_output: utxo.outpoint,
                sequence: u32::MAX,
                witness: SpendWitness::Empty,
            })
            .collect();
        let mut transaction = Transaction::new(params.chain_id, inputs, outputs);

        for (index, utxo) in selected.iter().enumerate() {
            let record = self
                .keys
                .get(utxo.key_index)
                .ok_or(WalletError::MissingKey)?;
            let key_pair = HybridKeyPair::from_secret_material(&record.secret)?;
            let message = transaction.signing_message(index, &utxo.output);
            let signature = key_pair.sign_hybrid(
                &params.chain_id,
                DomainTag::TransactionSignature,
                &message,
            )?;
            transaction.inputs[index].witness = match &utxo.output.policy {
                OutputPolicy::Hybrid(_) => SpendWitness::Hybrid(signature),
                OutputPolicy::HybridCommitment { commitment }
                    if *commitment == policy_commitment(&record.public_keys) =>
                {
                    SpendWitness::CommittedHybrid {
                        public_keys: record.public_keys.clone(),
                        signature,
                    }
                }
                _ => return Err(WalletError::MissingKey),
            };
        }
        Ok(transaction)
    }

    #[must_use]
    pub fn estimate_hybrid_fee(
        input_count: u64,
        output_count: u64,
        atoms_per_weight: u64,
    ) -> Amount {
        // Conservative estimate includes revealed PQ public keys and signatures.
        let base_weight = 500_u64.saturating_add(output_count.saturating_mul(180));
        let witness_weight = input_count.saturating_mul(11_000);
        Amount::from_atoms(
            base_weight
                .saturating_add(witness_weight)
                .saturating_mul(atoms_per_weight)
                .min(crate::amount::MAX_MONEY_ATOMS),
        )
        .expect("fee estimate is capped")
    }

    pub fn save_encrypted(
        &self,
        path: impl AsRef<Path>,
        password: &[u8],
    ) -> Result<(), WalletError> {
        let salt: [u8; 16] = rand::random();
        let nonce: [u8; 24] = rand::random();
        let mut key = derive_key(
            password,
            &salt,
            KDF_MEMORY_KIB,
            KDF_ITERATIONS,
            KDF_PARALLELISM,
        )?;
        let cipher = XChaCha20Poly1305::new((&key).into());
        let mut plaintext = serde_json::to_vec(self)?;
        let aad = wallet_aad(self.network);
        let nonce_value = XNonce::from(nonce);
        let ciphertext = cipher
            .encrypt(
                &nonce_value,
                Payload {
                    msg: &plaintext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| WalletError::Encryption)?;
        plaintext.zeroize();
        key.zeroize();

        let envelope = EncryptedWallet {
            format_version: WALLET_FORMAT_VERSION,
            network: self.network,
            kdf_memory_kib: KDF_MEMORY_KIB,
            kdf_iterations: KDF_ITERATIONS,
            kdf_parallelism: KDF_PARALLELISM,
            salt_base64: BASE64.encode(salt),
            nonce_base64: BASE64.encode(nonce),
            ciphertext_base64: BASE64.encode(ciphertext),
        };
        write_wallet_atomic(path.as_ref(), &serde_json::to_vec_pretty(&envelope)?)?;
        Ok(())
    }

    pub fn load_encrypted(path: impl AsRef<Path>, password: &[u8]) -> Result<Self, WalletError> {
        let envelope: EncryptedWallet = serde_json::from_slice(&fs::read(path)?)?;
        if envelope.format_version != WALLET_FORMAT_VERSION
            || envelope.kdf_memory_kib > 262_144
            || envelope.kdf_iterations > 10
            || envelope.kdf_parallelism > 8
        {
            return Err(WalletError::UnsupportedFormat);
        }
        let salt = BASE64
            .decode(envelope.salt_base64)
            .map_err(|_| WalletError::UnsupportedFormat)?;
        let nonce = BASE64
            .decode(envelope.nonce_base64)
            .map_err(|_| WalletError::UnsupportedFormat)?;
        let ciphertext = BASE64
            .decode(envelope.ciphertext_base64)
            .map_err(|_| WalletError::UnsupportedFormat)?;
        if salt.len() != 16 || nonce.len() != 24 {
            return Err(WalletError::UnsupportedFormat);
        }
        let mut key = derive_key(
            password,
            &salt,
            envelope.kdf_memory_kib,
            envelope.kdf_iterations,
            envelope.kdf_parallelism,
        )?;
        let cipher = XChaCha20Poly1305::new((&key).into());
        let aad = wallet_aad(envelope.network);
        let nonce_value =
            XNonce::try_from(nonce.as_slice()).map_err(|_| WalletError::UnsupportedFormat)?;
        let mut plaintext = cipher
            .decrypt(
                &nonce_value,
                Payload {
                    msg: &ciphertext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| WalletError::Decryption)?;
        key.zeroize();
        let wallet: Self =
            serde_json::from_slice(&plaintext).map_err(|_| WalletError::Decryption)?;
        plaintext.zeroize();
        if wallet.format_version != WALLET_FORMAT_VERSION || wallet.network != envelope.network {
            return Err(WalletError::UnsupportedFormat);
        }
        Ok(wallet)
    }

    pub fn backup(
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<(), WalletError> {
        fs::copy(source, destination)?;
        Ok(())
    }

    fn ensure_network(&self, params: &ChainParams) -> Result<(), WalletError> {
        if self.network == params.network {
            Ok(())
        } else {
            Err(WalletError::WrongNetwork)
        }
    }
}

fn derive_key(
    password: &[u8],
    salt: &[u8],
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
) -> Result<[u8; 32], WalletError> {
    let params = Params::new(memory_kib, iterations, parallelism, Some(32))
        .map_err(|_| WalletError::UnsupportedFormat)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0_u8; 32];
    argon
        .hash_password_into(password, salt, &mut key)
        .map_err(|_| WalletError::Encryption)?;
    Ok(key)
}

fn wallet_aad(network: Network) -> String {
    format!("QBTC/WALLET-FILE/v1:{}", network.name())
}

fn write_wallet_atomic(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let temporary = path.with_extension("tmp");
    let mut file = File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_wallet_round_trip_and_wrong_password_rejection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wallet.qbtc");
        let params = ChainParams::for_network(Network::Regtest);
        let wallet = Wallet::create(&params, None);
        wallet.save_encrypted(&path, b"correct horse").unwrap();

        let restored = Wallet::load_encrypted(&path, b"correct horse").unwrap();
        assert_eq!(
            restored.primary_address(&params).unwrap(),
            wallet.primary_address(&params).unwrap()
        );
        assert!(matches!(
            Wallet::load_encrypted(&path, b"wrong battery"),
            Err(WalletError::Decryption)
        ));
        let bytes = fs::read(path).unwrap();
        assert!(!bytes.windows(7).any(|window| window == b"schnorr"));
        assert!(!bytes.windows(6).any(|window| window == b"ml_dsa"));
    }

    #[test]
    fn fee_estimate_accounts_for_large_hybrid_witnesses() {
        let one = Wallet::estimate_hybrid_fee(1, 2, 1);
        let two = Wallet::estimate_hybrid_fee(2, 2, 1);
        assert!(one.atoms() > 10_000);
        assert!(two > one);
    }
}
