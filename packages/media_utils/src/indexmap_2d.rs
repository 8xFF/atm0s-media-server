use std::hash::Hash;

use indexmap::IndexMap;

pub struct IndexMap2d<T1, T2> {
    data: IndexMap<T1, T2>,
    reverse: IndexMap<T2, T1>,
}

impl<T1: Hash + Eq + Clone, T2: Hash + Eq + Clone> Default for IndexMap2d<T1, T2> {
    fn default() -> Self {
        Self {
            data: Default::default(),
            reverse: Default::default(),
        }
    }
}

impl<T1: Hash + Eq + Clone, T2: Hash + Eq + Clone> IndexMap2d<T1, T2> {
    pub fn insert(&mut self, key: T1, value: T2) {
        self.data.insert(key.clone(), value.clone());
        self.reverse.insert(value, key);
    }

    pub fn keys1(&self) -> Vec<T1> {
        self.data.keys().cloned().collect::<Vec<_>>()
    }

    pub fn pairs(&self) -> Vec<(T1, T2)> {
        self.data.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>()
    }

    pub fn keys2(&self) -> Vec<T2> {
        self.reverse.keys().cloned().collect::<Vec<_>>()
    }

    pub fn get1(&self, key: &T1) -> Option<&T2> {
        self.data.get(key)
    }

    pub fn get2(&self, key: &T2) -> Option<&T1> {
        self.reverse.get(key)
    }

    pub fn remove1(&mut self, key: &T1) -> Option<T2> {
        let value = self.data.swap_remove(key)?;
        self.reverse.swap_remove(&value);
        Some(value)
    }

    pub fn remove2(&mut self, key: &T2) -> Option<T1> {
        let value = self.reverse.swap_remove(key)?;
        self.data.swap_remove(&value);
        Some(value)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
