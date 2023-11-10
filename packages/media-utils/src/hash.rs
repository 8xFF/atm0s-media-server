use std::hash::{Hash, Hasher};

pub fn hash_str(input: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_str() {
        let input = "hello world";
        let expected_output = 8170069951894177743;
        let output = hash_str(input);
        assert_eq!(output, expected_output);
    }
}
