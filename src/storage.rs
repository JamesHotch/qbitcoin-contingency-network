use crate::{
    block::Block,
    chain::{ChainError, ChainState},
    crypto::SignatureProvider,
    hash::Hash32,
    params::ChainParams,
    pow::Sha256dPowV1,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Chain(#[from] ChainError),
    #[error("chainstate manifest belongs to another network")]
    WrongNetwork,
    #[error("chainstate manifest does not match validated blocks")]
    ManifestMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    generation: u64,
    chain_id: Hash32,
    tip_height: u64,
    tip_hash: Hash32,
}

pub struct ChainStore {
    root: PathBuf,
    generation: u64,
}

impl ChainStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let root = path.as_ref().to_path_buf();
        fs::create_dir_all(root.join("blocks"))?;
        let generation = read_manifests(&root)
            .into_iter()
            .map(|manifest| manifest.generation)
            .max()
            .unwrap_or(0);
        Ok(Self { root, generation })
    }

    pub fn load(
        &mut self,
        params: ChainParams,
        provider: &dyn SignatureProvider,
    ) -> Result<ChainState, StorageError> {
        let mut manifests = read_manifests(&self.root);
        manifests.sort_by_key(|manifest| std::cmp::Reverse(manifest.generation));
        for manifest in manifests {
            if manifest.chain_id != params.chain_id {
                continue;
            }
            match self.load_manifest(params.clone(), provider, &manifest) {
                Ok(chain) => {
                    self.generation = manifest.generation;
                    return Ok(chain);
                }
                Err(
                    StorageError::Io(_) | StorageError::Json(_) | StorageError::ManifestMismatch,
                ) => {
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(ChainState::new(params)?)
    }

    pub fn persist_active_chain(&mut self, chain: &ChainState) -> Result<(), StorageError> {
        for (height, block) in chain.active_blocks().iter().enumerate() {
            let path = self.block_path(height as u64);
            if !path.exists() {
                write_new_atomic(&path, &serde_json::to_vec(block)?)?;
            }
        }

        self.generation = self.generation.saturating_add(1);
        let manifest = Manifest {
            generation: self.generation,
            chain_id: chain.params().chain_id,
            tip_height: chain.height(),
            tip_hash: chain.tip_hash(),
        };
        let slot = self
            .root
            .join(format!("manifest-{}.json", self.generation % 2));
        write_replace_with_other_slot_preserved(&slot, &serde_json::to_vec_pretty(&manifest)?)?;
        Ok(())
    }

    fn load_manifest(
        &self,
        params: ChainParams,
        provider: &dyn SignatureProvider,
        manifest: &Manifest,
    ) -> Result<ChainState, StorageError> {
        let mut blocks = Vec::with_capacity(manifest.tip_height as usize + 1);
        for height in 0..=manifest.tip_height {
            let bytes = fs::read(self.block_path(height))?;
            blocks.push(serde_json::from_slice::<Block>(&bytes)?);
        }
        let chain = ChainState::from_active_blocks(params, &blocks, provider)?;
        if chain.tip_hash() != manifest.tip_hash
            || chain.height() != manifest.tip_height
            || blocks
                .last()
                .is_none_or(|block| block.hash(&Sha256dPowV1) != manifest.tip_hash)
        {
            return Err(StorageError::ManifestMismatch);
        }
        Ok(chain)
    }

    fn block_path(&self, height: u64) -> PathBuf {
        self.root.join("blocks").join(format!("{height:020}.json"))
    }
}

fn read_manifests(root: &Path) -> Vec<Manifest> {
    ["manifest-0.json", "manifest-1.json"]
        .into_iter()
        .filter_map(|name| fs::read(root.join(name)).ok())
        .filter_map(|bytes| serde_json::from_slice(&bytes).ok())
        .collect()
}

fn write_new_atomic(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let temporary = path.with_extension("tmp");
    let mut file = File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(temporary, path)
}

fn write_replace_with_other_slot_preserved(
    path: &Path,
    bytes: &[u8],
) -> Result<(), std::io::Error> {
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
    use crate::{
        crypto::{HybridKeyPair, RustCryptoProvider},
        params::Network,
        transaction::OutputPolicy,
    };

    #[test]
    fn chainstate_recovers_from_last_complete_manifest() {
        let directory = tempfile::tempdir().unwrap();
        let params = ChainParams::for_network(Network::Regtest);
        let mut chain = ChainState::new(params.clone()).unwrap();
        let mut store = ChainStore::open(directory.path()).unwrap();
        store.persist_active_chain(&chain).unwrap();

        let mut block = chain
            .block_template(
                OutputPolicy::Hybrid(HybridKeyPair::generate(None).public_keys()),
                Vec::new(),
                chain.tip_timestamp() + 1,
                b"persist",
                &RustCryptoProvider,
            )
            .unwrap();
        block.mine(&Sha256dPowV1, u32::MAX).unwrap();
        chain.submit_block(block, &RustCryptoProvider).unwrap();
        store.persist_active_chain(&chain).unwrap();

        fs::write(directory.path().join("manifest-0.tmp"), b"partial").unwrap();
        let recovered = ChainStore::open(directory.path())
            .unwrap()
            .load(params, &RustCryptoProvider)
            .unwrap();
        assert_eq!(recovered.height(), 1);
        assert_eq!(recovered.tip_hash(), chain.tip_hash());
    }
}
