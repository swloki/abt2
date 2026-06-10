// Temporary utility to generate argon2 hash for test password
// Run: cargo build --example gen_hash && cargo run --example gen_hash
// Or just: rustc --edition 2024 -L target/debug/deps tests/permission/gen_hash.rs -o target/gen_hash && target/gen_hash

use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};

fn main() {
    let password = "test1234";
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("hash failed")
        .to_string();
    println!("{hash}");
}
