use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

pub fn generate_random_string(length: usize) -> String {
    let rng = thread_rng();
    let random_string: String = rng.sample_iter(&Alphanumeric).take(length).map(char::from).collect();

    random_string
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_string() {
        let random_string = generate_random_string(16);

        assert_eq!(random_string.len(), 16);
    }
}
