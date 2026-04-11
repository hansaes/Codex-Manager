use rand::RngCore;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

// Env overrides:
// - CODEXMANAGER_PROMPT_CACHE_TTL_SECS (default: 0)
// - CODEXMANAGER_PROMPT_CACHE_CLEANUP_INTERVAL_SECS (default: 60)
// - CODEXMANAGER_PROMPT_CACHE_CAPACITY (default: 0; 0 disables capacity limit)
const PROMPT_CACHE_TTL_SECS_ENV: &str = "CODEXMANAGER_PROMPT_CACHE_TTL_SECS";
const PROMPT_CACHE_CLEANUP_INTERVAL_SECS_ENV: &str =
    "CODEXMANAGER_PROMPT_CACHE_CLEANUP_INTERVAL_SECS";
const PROMPT_CACHE_CAPACITY_ENV: &str = "CODEXMANAGER_PROMPT_CACHE_CAPACITY";

const DEFAULT_PROMPT_CACHE_TTL_SECS: u64 = 0;
const DEFAULT_PROMPT_CACHE_CLEANUP_INTERVAL_SECS: u64 = 60;
const DEFAULT_PROMPT_CACHE_CAPACITY: usize = 0;

static PROMPT_CACHE: OnceLock<Mutex<PromptCache>> = OnceLock::new();

#[derive(Clone)]
struct PromptCacheEntry {
    id: String,
    last_seen: Instant,
    lru_tick: u64,
}

/// 函数 `resolve_prompt_cache_key`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - super: 参数 super
///
/// # 返回
/// 返回函数执行结果
pub(super) fn resolve_prompt_cache_key(
    source: &serde_json::Map<String, Value>,
    model: Option<&Value>,
) -> Option<String> {
    let model = model
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())?;
    let user_id = source
        .get("metadata")
        .and_then(Value::as_object)
        .and_then(|meta| meta.get("user_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("unknown");

    let cache_key = format!("{model}:{user_id}");
    Some(get_or_create_prompt_cache_id(&cache_key))
}

/// 函数 `get_or_create_prompt_cache_id`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - key: 参数 key
///
/// # 返回
/// 返回函数执行结果
fn get_or_create_prompt_cache_id(key: &str) -> String {
    let now = Instant::now();
    let cache = PROMPT_CACHE.get_or_init(|| Mutex::new(PromptCache::new(now)));
    let mut guard = crate::lock_utils::lock_recover(cache, "prompt_cache");
    guard.get_or_create(key, now)
}

/// 函数 `with_prompt_cache_mut`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - f: 参数 f
///
/// # 返回
/// 返回函数执行结果
fn with_prompt_cache_mut<R>(f: impl FnOnce(&mut PromptCache) -> R) -> Option<R> {
    let cache = PROMPT_CACHE.get()?;
    let mut guard = crate::lock_utils::lock_recover(cache, "prompt_cache");
    Some(f(&mut guard))
}

#[derive(Clone, Copy)]
struct PromptCacheConfig {
    ttl: Duration,
    cleanup_interval: Duration,
    capacity: usize,
}

impl PromptCacheConfig {
    /// 函数 `load_from_env`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// 无
    ///
    /// # 返回
    /// 返回函数执行结果
    fn load_from_env() -> Self {
        let ttl_secs = env_u64_or(PROMPT_CACHE_TTL_SECS_ENV, DEFAULT_PROMPT_CACHE_TTL_SECS);
        let cleanup_secs = env_u64_or(
            PROMPT_CACHE_CLEANUP_INTERVAL_SECS_ENV,
            DEFAULT_PROMPT_CACHE_CLEANUP_INTERVAL_SECS,
        );
        let capacity = env_usize_or(PROMPT_CACHE_CAPACITY_ENV, DEFAULT_PROMPT_CACHE_CAPACITY);
        Self {
            ttl: Duration::from_secs(ttl_secs),
            cleanup_interval: Duration::from_secs(cleanup_secs),
            capacity,
        }
    }
}

struct PromptCache {
    by_key: HashMap<String, PromptCacheEntry>,
    // LRU ordering by monotonic tick: smallest tick = least recently seen.
    lru_by_tick: BTreeMap<u64, String>,
    tick: u64,
    last_cleanup: Instant,
    config: PromptCacheConfig,
}

impl PromptCache {
    /// 函数 `new`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - now: 参数 now
    ///
    /// # 返回
    /// 返回函数执行结果
    fn new(now: Instant) -> Self {
        Self {
            by_key: HashMap::new(),
            lru_by_tick: BTreeMap::new(),
            tick: 0,
            last_cleanup: now,
            config: PromptCacheConfig::load_from_env(),
        }
    }

    /// 函数 `get_or_create`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    /// - key: 参数 key
    /// - now: 参数 now
    ///
    /// # 返回
    /// 返回函数执行结果
    fn get_or_create(&mut self, key: &str, now: Instant) -> String {
        self.maybe_cleanup(now);

        // Fast path: key hit.
        let mut expired_tick: Option<u64> = None;
        let mut touch: Option<(String, u64, u64)> = None;
        if let Some(entry) = self.by_key.get_mut(key) {
            if is_entry_expired(entry.last_seen, now, self.config.ttl) {
                // If the accessed entry is expired, drop it immediately (no full scan).
                expired_tick = Some(entry.lru_tick);
            } else {
                let old_tick = entry.lru_tick;
                self.tick = self.tick.wrapping_add(1);
                let new_tick = self.tick;
                entry.last_seen = now;
                entry.lru_tick = new_tick;
                touch = Some((entry.id.clone(), old_tick, new_tick));
            }
        }

        if let Some(stale_tick) = expired_tick {
            self.by_key.remove(key);
            self.lru_by_tick.remove(&stale_tick);
        }

        if let Some((id, old_tick, new_tick)) = touch {
            self.lru_by_tick.remove(&old_tick);
            self.lru_by_tick.insert(new_tick, key.to_string());
            return id;
        }

        // Miss: create new entry.
        let id = random_uuid_v4();
        self.tick = self.tick.wrapping_add(1);
        let tick = self.tick;
        self.by_key.insert(
            key.to_string(),
            PromptCacheEntry {
                id: id.clone(),
                last_seen: now,
                lru_tick: tick,
            },
        );
        self.lru_by_tick.insert(tick, key.to_string());
        self.enforce_capacity();
        id
    }

    /// 函数 `maybe_cleanup`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    /// - now: 参数 now
    ///
    /// # 返回
    /// 无
    fn maybe_cleanup(&mut self, now: Instant) {
        let interval = self.config.cleanup_interval;
        if interval.is_zero()
            || now
                .checked_duration_since(self.last_cleanup)
                .is_some_and(|elapsed| elapsed >= interval)
        {
            self.cleanup(now);
        }
    }

    /// 函数 `cleanup`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    /// - now: 参数 now
    ///
    /// # 返回
    /// 无
    fn cleanup(&mut self, now: Instant) {
        self.last_cleanup = now;

        let ttl = self.config.ttl;
        if !ttl.is_zero() {
            self.by_key
                .retain(|_, entry| !is_entry_expired(entry.last_seen, now, ttl));
        }

        // Rebuild the LRU index to avoid drift (e.g. if entries were pruned).
        self.lru_by_tick.clear();
        for (key, entry) in self.by_key.iter() {
            self.lru_by_tick.insert(entry.lru_tick, key.clone());
        }

        self.enforce_capacity();
    }

    /// 函数 `enforce_capacity`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    ///
    /// # 返回
    /// 无
    fn enforce_capacity(&mut self) {
        let cap = self.config.capacity;
        if cap == 0 {
            return;
        }
        while self.by_key.len() > cap {
            let Some((&oldest_tick, oldest_key)) = self.lru_by_tick.iter().next() else {
                break;
            };
            let oldest_key = oldest_key.clone();
            self.lru_by_tick.remove(&oldest_tick);
            self.by_key.remove(oldest_key.as_str());
        }
    }
}

/// 函数 `clear_runtime_state`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - super: 参数 super
///
/// # 返回
/// 无
pub(super) fn clear_runtime_state() {
    let _ = with_prompt_cache_mut(|cache| {
        let now = Instant::now();
        let config = cache.config;
        *cache = PromptCache::new(now);
        cache.config = config;
    });
}

/// 函数 `reload_from_env`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - super: 参数 super
///
/// # 返回
/// 无
pub(super) fn reload_from_env() {
    let _ = with_prompt_cache_mut(|cache| {
        cache.config = PromptCacheConfig::load_from_env();
        cache.cleanup(Instant::now());
    });
}

/// 函数 `reload_runtime_state`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - crate: 参数 crate
///
/// # 返回
/// 无
pub(crate) fn reload_runtime_state() {
    clear_runtime_state();
    reload_from_env();
}

/// 函数 `is_entry_expired`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - last_seen: 参数 last_seen
/// - now: 参数 now
/// - ttl: 参数 ttl
///
/// # 返回
/// 返回函数执行结果
fn is_entry_expired(last_seen: Instant, now: Instant, ttl: Duration) -> bool {
    if ttl.is_zero() {
        return false;
    }
    now.checked_duration_since(last_seen)
        .is_some_and(|age| age > ttl)
}

/// 函数 `env_u64_or`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - name: 参数 name
/// - default: 参数 default
///
/// # 返回
/// 返回函数执行结果
fn env_u64_or(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

/// 函数 `env_usize_or`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - name: 参数 name
/// - default: 参数 default
///
/// # 返回
/// 返回函数执行结果
fn env_usize_or(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

/// 函数 `random_uuid_v4`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// 无
///
/// # 返回
/// 返回函数执行结果
fn random_uuid_v4() -> String {
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

#[cfg(test)]
#[path = "tests/prompt_cache_tests.rs"]
mod tests;
