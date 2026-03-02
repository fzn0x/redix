use dashmap::DashMap;
use std::sync::Arc;

pub struct KvStore {
    data: Arc<DashMap<String, String>>,
}

impl KvStore {
    pub fn new() -> Self {
        Self {
            data: Arc::new(DashMap::new()),
        }
    }

    pub fn set(&self, key: String, value: String) {
        self.data.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.data.get(key).map(|v| v.clone())
    }

    pub fn delete(&self, key: &str) {
        self.data.remove(key);
    }

    pub fn keys(&self) -> Vec<String> {
        self.data.iter().map(|item| item.key().clone()).collect()
    }

    pub fn clear(&self) {
        self.data.clear();
    }

    pub fn get_state(&self) -> DashMap<String, String> {
        let new_map = DashMap::new();
        for item in self.data.iter() {
            new_map.insert(item.key().clone(), item.value().clone());
        }
        new_map
    }

    pub fn restore_state(&self, state: DashMap<String, String>) {
        self.data.clear();
        for item in state {
            self.data.insert(item.0, item.1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kv_set_get() {
        let kv = KvStore::new();
        kv.set("foo".into(), "bar".into());
        assert_eq!(kv.get("foo"), Some("bar".into()));
    }

    #[test]
    fn test_kv_delete() {
        let kv = KvStore::new();
        kv.set("foo".into(), "bar".into());
        kv.delete("foo");
        assert_eq!(kv.get("foo"), None);
    }
}
