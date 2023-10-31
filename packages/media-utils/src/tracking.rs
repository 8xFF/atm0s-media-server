use std::collections::HashMap;

#[derive(Default)]
pub struct ResourceTracking {
    resources: HashMap<String, i32>,
}

impl ResourceTracking {
    /// Adding resource to list, first type is count as 1, if already exits => count++
    pub fn add(&mut self, resource: &str) {
        let entry = self.resources.entry(resource.to_string()).or_insert(0);
        *entry += 1;
    }

    /// Remove resource from list, if count == 0 => remove from list, if count < 0 => panic
    pub fn remove(&mut self, resource: &str) {
        if let Some(entry) = self.resources.get_mut(resource) {
            *entry -= 1;
            if *entry == 0 {
                self.resources.remove(resource);
            } else if *entry < 0 {
                panic!("ResourceTracking: entry < 0");
            }
        }
    }

    pub fn add2(&mut self, resource: &str, sub: &str) {
        self.add(&format!("{}/{}", resource, sub));
    }

    pub fn remove2(&mut self, resource: &str, sub: &str) {
        self.remove(&format!("{}/{}", resource, sub));
    }

    pub fn add3(&mut self, resource: &str, sub: &str, sub2: &str) {
        self.add(&format!("{}/{}/{}", resource, sub, sub2));
    }

    pub fn remove3(&mut self, resource: &str, sub: &str, sub2: &str) {
        self.remove(&format!("{}/{}/{}", resource, sub, sub2));
    }

    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    pub fn dump(&self) -> String {
        let mut res = String::new();
        for (k, v) in self.resources.iter() {
            res.push_str(&format!("{}: {}, ", k, v));
        }
        res
    }
}
