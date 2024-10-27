use once_cell::sync::Lazy;
use spin::Mutex;
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};

static REGISTRY: Lazy<Mutex<HashMap<&'static str, AtomicUsize>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Tracks the number of live instances for a specific type
#[derive(Debug)]
pub struct Count<T> {
    _phantom: PhantomData<T>,
}

impl<T> Count<T> {
    fn type_name() -> &'static str {
        std::any::type_name::<T>()
    }

    fn increase() {
        let mut registry = REGISTRY.lock();
        let counter = registry.entry(Self::type_name()).or_insert_with(|| AtomicUsize::new(0));
        counter.fetch_add(1, Ordering::SeqCst);
    }

    fn decrease() {
        let mut registry = REGISTRY.lock();
        let counter = registry.entry(Self::type_name()).or_insert_with(|| AtomicUsize::new(0));
        counter.fetch_sub(1, Ordering::SeqCst);
    }
}

impl<T> Default for Count<T> {
    fn default() -> Self {
        Self::increase();
        Self { _phantom: Default::default() }
    }
}

impl<T> Drop for Count<T> {
    fn drop(&mut self) {
        Self::decrease();
    }
}

impl<T> Clone for Count<T> {
    fn clone(&self) -> Self {
        Self::increase();
        Self { _phantom: Default::default() }
    }
}

/// Returns a map of all type names to their current counts
pub fn get_all_counts() -> BTreeMap<&'static str, usize> {
    let registry = REGISTRY.lock();
    registry.iter().map(|(name, count)| (*name, count.load(Ordering::SeqCst))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count() {
        let c1 = Count::<u32>::default();
        let c2 = Count::<u32>::default();
        let c3 = Count::<u32>::default();
        let _c4 = Count::<u32>::default();
        assert_eq!(get_all_counts().get("u32"), Some(&4));

        drop(c1);
        drop(c2);
        drop(c3);
        assert_eq!(get_all_counts().get("u32"), Some(&1));
    }
}
