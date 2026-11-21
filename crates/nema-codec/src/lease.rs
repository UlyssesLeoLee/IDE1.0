//! IDE1.0 × IME1.0 所有权仲裁状态机 (Observer + Lease + Epoch)

use crate::EncodedChoice;
use serde::{Deserialize, Serialize};

/// 所有权角色：默认 `Observer`（只读 IME 最新版本，绝不按键后抢转换）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OwnershipRole {
    Observer,
    LeaseRequested {
        source_hash: u64,
        revision: u64,
        epoch: u64,
    },
    ImePausedSameEpoch {
        source_hash: u64,
        revision: u64,
        epoch: u64,
    },
    Committed {
        source_hash: u64,
        revision: u64,
        epoch: u64,
    },
}

/// 幂等转换缓存记录
#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedCommit {
    source_hash: u64,
    revision: u64,
    epoch: u64,
    choice: EncodedChoice,
}

/// IDE1.0 × IME1.0 租约仲裁器
#[derive(Debug, Clone)]
pub struct NemaLeaseArbitrator {
    role: OwnershipRole,
    current_epoch: u64,
    last_commit: Option<CachedCommit>,
}

impl Default for NemaLeaseArbitrator {
    fn default() -> Self {
        Self {
            role: OwnershipRole::Observer,
            current_epoch: 0,
            last_commit: None,
        }
    }
}

impl NemaLeaseArbitrator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn role(&self) -> &OwnershipRole {
        &self.role
    }

    pub fn current_epoch(&self) -> u64 {
        self.current_epoch
    }

    /// 计算源码 FNV-1a 64-bit 哈希
    pub fn hash_source(source: &str) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for b in source.as_bytes() {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    /// 1. `request_lease(source_hash, revision)`
    ///
    /// 若同一 `(source_hash, revision)` 已提交过，直接返回缓存的幂等结果。
    pub fn request_lease(
        &mut self,
        source_hash: u64,
        revision: u64,
    ) -> (u64, Option<EncodedChoice>) {
        if let Some(cached) = &self.last_commit {
            if cached.source_hash == source_hash && cached.revision == revision {
                return (cached.epoch, Some(cached.choice.clone()));
            }
        }
        self.current_epoch += 1;
        let epoch = self.current_epoch;
        self.role = OwnershipRole::LeaseRequested {
            source_hash,
            revision,
            epoch,
        };
        (epoch, None)
    }

    /// 2. `pause_ime_same_epoch(epoch)`：确认同一 epoch 下暂停 IME 自动转换；过时 epoch 丢弃
    pub fn pause_ime_same_epoch(&mut self, epoch: u64) -> bool {
        if epoch != self.current_epoch {
            return false;
        }
        if let OwnershipRole::LeaseRequested {
            source_hash,
            revision,
            epoch: ep,
        } = self.role
        {
            self.role = OwnershipRole::ImePausedSameEpoch {
                source_hash,
                revision,
                epoch: ep,
            };
            true
        } else {
            false
        }
    }

    /// 3. `atomic_commit(epoch, choice)`：仅当前活跃 epoch 允许原子提交，过时 epoch 的计算结果全部丢弃
    pub fn atomic_commit(&mut self, epoch: u64, choice: EncodedChoice) -> bool {
        if epoch != self.current_epoch {
            return false;
        }
        if let OwnershipRole::ImePausedSameEpoch {
            source_hash,
            revision,
            epoch: ep,
        } = self.role
        {
            self.last_commit = Some(CachedCommit {
                source_hash,
                revision,
                epoch: ep,
                choice,
            });
            self.role = OwnershipRole::Committed {
                source_hash,
                revision,
                epoch: ep,
            };
            true
        } else {
            false
        }
    }

    /// 4. `release_lease(epoch)`：释放租约并回到 `Observer`
    pub fn release_lease(&mut self, epoch: u64) -> bool {
        if epoch != self.current_epoch {
            return false;
        }
        self.role = OwnershipRole::Observer;
        true
    }
}
