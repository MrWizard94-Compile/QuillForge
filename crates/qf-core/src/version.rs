//! Package and schema versioning.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Semantic version for package format (independent of app semver).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl Version {
    pub const V1_0_0: Self = Self {
        major: 1,
        minor: 0,
        patch: 0,
    };

    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_order() {
        assert!(Version::new(1, 0, 0) < Version::new(1, 1, 0));
        assert!(Version::new(1, 1, 0) < Version::new(2, 0, 0));
    }
}
