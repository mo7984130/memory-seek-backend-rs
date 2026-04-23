use crate::utils::{Argon2idConfig, HashAlgorithm};

pub const HASHER: HashAlgorithm = HashAlgorithm::Argon2id(
    Argon2idConfig {
        m_cost: 16 * 1024,
        t_cost: 2,
        p_cost: 1
    }
);


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_hundred_hashes() {
        // DELETE FROM auth_user WHERE username LIKE 'testuser%';

        let password = "123456abc";
        for i in 0..100 {
            let hash = HASHER.hash(password).unwrap();
            println!("INSERT INTO auth_user (username, password, nickname, email, inviter) VALUES ('testuser{}', '{}', 'testuser{}', 'testuser{}@example.com', 1);", i + 1, hash, i + 1, i + 1);
        }
    }
}
