use std::cmp::Ordering;
use std::fmt;

/// A version as kuleshov's tags use it: `x.y.z` with an optional pre-release (`0.2.0-alpha.1`),
/// ordered by `SemVer`'s precedence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// The pre-release's identifiers (`["alpha", "1"]`), empty for a release.
    pub prerelease: Vec<String>,
}

impl Version {
    /// A version from its text, or `None` when it is not `x.y.z` with an optional pre-release.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (core, prerelease) = match text.split_once('-') {
            Some((core, prerelease)) => (core, Some(prerelease)),
            None => (text, None),
        };
        let numbers: Vec<u64> = core.split('.').map(number).collect::<Option<_>>()?;
        let [major, minor, patch] = numbers[..] else {
            return None;
        };
        let prerelease = match prerelease {
            Some(text) => text
                .split('.')
                .map(|identifier| is_identifier(identifier).then(|| identifier.to_owned()))
                .collect::<Option<_>>()?,
            None => Vec::new(),
        };
        Some(Self {
            major,
            minor,
            patch,
            prerelease,
        })
    }

    /// The version after this one by default: the next patch (`0.1.0` → `0.1.1`), or a
    /// pre-release's release (`0.2.0-alpha.1` → `0.2.0`).
    #[must_use]
    pub fn next_patch(&self) -> Self {
        Self {
            patch: if self.prerelease.is_empty() {
                self.patch + 1
            } else {
                self.patch
            },
            prerelease: Vec::new(),
            ..self.clone()
        }
    }

    /// The tag this version is released under: `v0.1.0`.
    #[must_use]
    pub fn tag(&self) -> String {
        format!("v{self}")
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.prerelease.is_empty() {
            write!(formatter, "-{}", self.prerelease.join("."))?;
        }
        Ok(())
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(
                || match (self.prerelease.is_empty(), other.prerelease.is_empty()) {
                    // A pre-release comes before its release.
                    (true, true) => Ordering::Equal,
                    (true, false) => Ordering::Greater,
                    (false, true) => Ordering::Less,
                    (false, false) => compare_prereleases(&self.prerelease, &other.prerelease),
                },
            )
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A number without leading zeros.
fn number(text: &str) -> Option<u64> {
    let digits = !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    (digits && (text == "0" || !text.starts_with('0')))
        .then(|| text.parse().ok())
        .flatten()
}

/// A pre-release identifier: a number without leading zeros, or ASCII letters, digits and hyphens
/// with at least one that is not a digit.
fn is_identifier(text: &str) -> bool {
    let allowed = text
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    allowed
        && !text.is_empty()
        && (number(text).is_some() || !text.bytes().all(|byte| byte.is_ascii_digit()))
}

/// `SemVer`'s order of two pre-releases: identifier by identifier, numbers by value and before words,
/// and a shorter one first when all its identifiers are equal.
fn compare_prereleases(a: &[String], b: &[String]) -> Ordering {
    for (a, b) in a.iter().zip(b) {
        let order = match (number(a), number(b)) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => a.cmp(b),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    a.len().cmp(&b.len())
}
