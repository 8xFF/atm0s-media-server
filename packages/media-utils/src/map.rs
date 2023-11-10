use std::hash::Hash;

/// A hash map that allows for multiple keys to reference the same value.
///
/// This implementation uses two internal hash maps to allow for efficient lookups
/// by either of the two keys. The keys must implement the `Hash` and `Eq` traits,
/// and must also be cloneable.
#[derive(Default)]
pub struct HashMapMultiKey<K1, K2, V> {
    map: std::collections::HashMap<K1, (V, K2)>,
    key2map: std::collections::HashMap<K2, K1>,
}

impl<K1: Hash + Eq + Clone, K2: Hash + Eq + Clone, V> HashMapMultiKey<K1, K2, V> {
    /// Inserts a new key-value pair into the map.
    ///
    /// If the first key already exists in the map, its corresponding value and
    /// second key will be updated to the new values.
    pub fn insert(&mut self, k1: K1, k2: K2, v: V) {
        self.map.insert(k1.clone(), (v, k2.clone()));
        self.key2map.insert(k2, k1);
    }

    /// Returns a reference to the value and second key corresponding to the given first key.
    pub fn get_by_k1(&self, k1: &K1) -> Option<(&V, &K2)> {
        self.map.get(k1).map(|(v, k2)| (v, k2))
    }

    /// Returns a reference to the value and first key corresponding to the given second key.
    pub fn get_by_k2(&self, k2: &K2) -> Option<(&V, &K1)> {
        let k1 = self.key2map.get(k2)?;
        self.map.get(k1).map(|(v, _)| (v, k1))
    }

    /// Returns a mutable reference to the value and second key corresponding to the given first key.
    pub fn get_mut_by_k1(&mut self, k1: &K1) -> Option<(&mut V, &K2)> {
        self.map.get_mut(k1).map(|(v, k2)| (v, &*k2))
    }

    /// Returns a mutable reference to the value and first key corresponding to the given second key.
    pub fn get_mut_by_k2(&mut self, k2: &K2) -> Option<(&mut V, &K1)> {
        let k1 = self.key2map.get(k2)?;
        self.map.get_mut(k1).map(|(v, _)| (v, k1))
    }

    /// Removes the key-value pair corresponding to the given first key from the map.
    ///
    /// Returns the value and second key that were associated with the removed first key,
    /// or `None` if the first key was not found in the map.
    pub fn remove_by_k1(&mut self, k1: &K1) -> Option<(V, K2)> {
        if let Some((v, k2)) = self.map.remove(k1) {
            self.key2map.remove(&k2);
            Some((v, k2))
        } else {
            None
        }
    }

    /// Removes the key-value pair corresponding to the given second key from the map.
    ///
    /// Returns the value and first key that were associated with the removed second key,
    /// or `None` if the second key was not found in the map.
    pub fn remove_by_k2(&mut self, k2: &K2) -> Option<(V, K1)> {
        if let Some(k1) = self.key2map.remove(k2) {
            self.map.remove(&k1).map(|(v, _)| (v, k1))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get_by_k1() {
        let mut map = HashMapMultiKey::default();
        map.insert("key1", "key2", "value");
        assert_eq!(map.get_by_k1(&"key1"), Some((&"value", &"key2")));
    }

    #[test]
    fn test_insert_and_get_by_k2() {
        let mut map = HashMapMultiKey::default();
        map.insert("key1", "key2", "value");
        assert_eq!(map.get_by_k2(&"key2"), Some((&"value", &"key1")));
    }

    #[test]
    fn test_insert_and_get_mut_by_k1() {
        let mut map = HashMapMultiKey::default();
        map.insert("key1", "key2", "value");
        if let Some((v, k2)) = map.get_mut_by_k1(&"key1") {
            *v = "new_value";
            assert_eq!(*v, "new_value");
            assert_eq!(*k2, "key2");
        } else {
            panic!("Expected to find key1 in the map");
        }
    }

    #[test]
    fn test_insert_and_get_mut_by_k2() {
        let mut map = HashMapMultiKey::default();
        map.insert("key1", "key2", "value");
        if let Some((v, k1)) = map.get_mut_by_k2(&"key2") {
            *v = "new_value";
            assert_eq!(*v, "new_value");
            assert_eq!(*k1, "key1");
        } else {
            panic!("Expected to find key2 in the map");
        }
    }

    #[test]
    fn test_remove_by_k1() {
        let mut map = HashMapMultiKey::default();
        map.insert("key1", "key2", "value");
        assert_eq!(map.remove_by_k1(&"key1"), Some(("value", "key2")));
        assert_eq!(map.get_by_k1(&"key1"), None);
        assert_eq!(map.get_by_k2(&"key2"), None);
    }

    #[test]
    fn test_remove_by_k2() {
        let mut map = HashMapMultiKey::default();
        map.insert("key1", "key2", "value");
        assert_eq!(map.remove_by_k2(&"key2"), Some(("value", "key1")));
        assert_eq!(map.get_by_k1(&"key1"), None);
        assert_eq!(map.get_by_k2(&"key2"), None);
    }
}
