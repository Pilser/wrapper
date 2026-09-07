//! NAT/masquerade rules for VM egress.

pub mod rules;
pub mod setup;

pub use rules::nat_rules;
pub use setup::setup_nat;
