use std::collections::HashMap;

/// A struct for tracking resources and their counts.
#[derive(Default)]
pub struct ResourceTracking {
    resources: HashMap<String, i32>,
}

impl ResourceTracking {
    /// Adds a resource to the list. If the resource already exists, its count is incremented.
    pub fn add(&mut self, resource: &str) {
        let entry = self.resources.entry(resource.to_string()).or_insert(0);
        *entry += 1;
    }

    /// Removes a resource from the list. If the count of the resource is 0, it is removed from the list.
    /// If the count of the resource is less than 0, a panic occurs.
    pub fn remove(&mut self, resource: &str) {
        if let Some(entry) = self.resources.get_mut(resource) {
            *entry -= 1;
            if *entry == 0 {
                self.resources.remove(resource);
            }
        } else {
            panic!("ResourceTracking: entry not found");
        }
    }

    /// Adds a sub-resource to the list.
    pub fn add2(&mut self, resource: &str, sub: &str) {
        self.add(&format!("{}/{}", resource, sub));
    }

    /// Removes a sub-resource from the list.
    pub fn remove2(&mut self, resource: &str, sub: &str) {
        self.remove(&format!("{}/{}", resource, sub));
    }

    /// Adds a sub-sub-resource to the list.
    pub fn add3(&mut self, resource: &str, sub: &str, sub2: &str) {
        self.add(&format!("{}/{}/{}", resource, sub, sub2));
    }

    /// Removes a sub-sub-resource from the list.
    pub fn remove3(&mut self, resource: &str, sub: &str, sub2: &str) {
        self.remove(&format!("{}/{}/{}", resource, sub, sub2));
    }

    /// Returns true if the resource list is empty.
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    /// Returns a string representation of the resource list.
    pub fn dump(&self) -> String {
        let mut res = String::new();
        for (k, v) in self.resources.iter() {
            res.push_str(&format!("{}: {}, ", k, v));
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let mut tracker = ResourceTracking::default();
        tracker.add("resource1");
        tracker.add("resource2");
        tracker.add("resource1");
        assert_eq!(tracker.resources.get("resource1"), Some(&2));
        assert_eq!(tracker.resources.get("resource2"), Some(&1));
    }

    #[test]
    fn test_remove() {
        let mut tracker = ResourceTracking::default();
        tracker.add("resource1");
        tracker.add("resource2");
        tracker.add("resource1");
        tracker.remove("resource1");
        assert_eq!(tracker.resources.get("resource1"), Some(&1));
        assert_eq!(tracker.resources.get("resource2"), Some(&1));
        tracker.remove("resource1");
        assert_eq!(tracker.resources.get("resource1"), None);
        assert_eq!(tracker.resources.get("resource2"), Some(&1));
    }

    #[test]
    #[should_panic(expected = "ResourceTracking: entry not found")]
    fn test_remove_panic() {
        let mut tracker = ResourceTracking::default();
        tracker.remove("resource1");
    }

    #[test]
    fn test_add2() {
        let mut tracker = ResourceTracking::default();
        tracker.add2("resource1", "sub1");
        tracker.add2("resource1", "sub2");
        tracker.add2("resource1", "sub1");
        assert_eq!(tracker.resources.get("resource1/sub1"), Some(&2));
        assert_eq!(tracker.resources.get("resource1/sub2"), Some(&1));
    }

    #[test]
    fn test_remove2() {
        let mut tracker = ResourceTracking::default();
        tracker.add2("resource1", "sub1");
        tracker.add2("resource1", "sub2");
        tracker.add2("resource1", "sub1");
        tracker.remove2("resource1", "sub1");
        assert_eq!(tracker.resources.get("resource1/sub1"), Some(&1));
        assert_eq!(tracker.resources.get("resource1/sub2"), Some(&1));
        tracker.remove2("resource1", "sub1");
        assert_eq!(tracker.resources.get("resource1/sub1"), None);
        assert_eq!(tracker.resources.get("resource1/sub2"), Some(&1));
    }

    #[test]
    fn test_add3() {
        let mut tracker = ResourceTracking::default();
        tracker.add3("resource1", "sub1", "subsub1");
        tracker.add3("resource1", "sub1", "subsub2");
        tracker.add3("resource1", "sub1", "subsub1");
        assert_eq!(tracker.resources.get("resource1/sub1/subsub1"), Some(&2));
        assert_eq!(tracker.resources.get("resource1/sub1/subsub2"), Some(&1));
    }

    #[test]
    fn test_remove3() {
        let mut tracker = ResourceTracking::default();
        tracker.add3("resource1", "sub1", "subsub1");
        tracker.add3("resource1", "sub1", "subsub2");
        tracker.add3("resource1", "sub1", "subsub1");
        tracker.remove3("resource1", "sub1", "subsub1");
        assert_eq!(tracker.resources.get("resource1/sub1/subsub1"), Some(&1));
        assert_eq!(tracker.resources.get("resource1/sub1/subsub2"), Some(&1));
        tracker.remove3("resource1", "sub1", "subsub1");
        assert_eq!(tracker.resources.get("resource1/sub1/subsub1"), None);
        assert_eq!(tracker.resources.get("resource1/sub1/subsub2"), Some(&1));
    }

    #[test]
    fn test_is_empty() {
        let mut tracker = ResourceTracking::default();
        assert_eq!(tracker.is_empty(), true);
        tracker.add("resource1");
        assert_eq!(tracker.is_empty(), false);
        tracker.remove("resource1");
        assert_eq!(tracker.is_empty(), true);
    }
}
