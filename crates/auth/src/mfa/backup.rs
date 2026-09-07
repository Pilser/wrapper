//! Backup codes for MFA recovery.

pub fn generate_backup_codes(n: usize) -> Vec<String> {
    (0..n).map(|_| uuid::Uuid::new_v4().simple().to_string()).collect()
}
