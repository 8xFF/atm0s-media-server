use std::hash::Hash;

#[derive(Default)]
pub struct HashMapMultiKey<K1, K2, V> {
    map: std::collections::HashMap<K1, (V, K2)>,
    key2map: std::collections::HashMap<K2, K1>,
}

impl<K1: Hash + Eq + Clone, K2: Hash + Eq + Clone, V> HashMapMultiKey<K1, K2, V> {
    pub fn insert(&mut self, k1: K1, k2: K2, v: V) {
        self.map.insert(k1.clone(), (v, k2.clone()));
        self.key2map.insert(k2, k1);
    }

    pub fn get_by_k1(&self, k1: &K1) -> Option<(&V, &K2)> {
        self.map.get(k1).map(|(v, k2)| (v, k2))
    }

    pub fn get_by_k2(&self, k2: &K2) -> Option<(&V, &K1)> {
        let k1 = self.key2map.get(k2)?;
        self.map.get(k1).map(|(v, _)| (v, k1))
    }

    pub fn get_mut_by_k1(&mut self, k1: &K1) -> Option<(&mut V, &K2)> {
        self.map.get_mut(k1).map(|(v, k2)| (v, &*k2))
    }

    pub fn get_mut_by_k2(&mut self, k2: &K2) -> Option<(&mut V, &K1)> {
        let k1 = self.key2map.get(k2)?;
        self.map.get_mut(k1).map(|(v, _)| (v, k1))
    }

    pub fn remove_by_k1(&mut self, k1: &K1) -> Option<(V, K2)> {
        if let Some((v, k2)) = self.map.remove(k1) {
            self.key2map.remove(&k2);
            Some((v, k2))
        } else {
            None
        }
    }

    pub fn remove_by_k2(&mut self, k2: &K2) -> Option<(V, K1)> {
        if let Some(k1) = self.key2map.remove(k2) {
            self.map.remove(&k1).map(|(v, _)| (v, k1))
        } else {
            None
        }
    }
}
