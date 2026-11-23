use std::sync::Arc;

/// Why an exact direction-run pattern cannot be registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternError {
    /// At least one direction run is required.
    Empty,
    /// More than 4096 direction runs were supplied.
    TooLong,
    /// The zero-based second member of the first consecutive equal pair.
    RepeatedDirection {
        /// Index of the unreachable run.
        index: usize,
    },
}
impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("A gesture pattern requires at least one direction run"),
            Self::TooLong => f.write_str("A gesture pattern supports at most 4096 direction runs"),
            Self::RepeatedDirection { index } => {
                write!(f, "Consecutive equal direction at index {index}")
            }
        }
    }
}
impl std::error::Error for PatternError {}

/// Validate without mutation. Length errors precede direction-run errors.
pub fn validate_pattern(pattern: &[Direction]) -> Result<(), PatternError> {
    if pattern.is_empty() {
        return Err(PatternError::Empty);
    }
    if !crate::validate_direction_limit(pattern.len()) {
        return Err(PatternError::TooLong);
    }
    if let Some(index) = pattern.windows(2).position(|pair| pair[0] == pair[1]) {
        return Err(PatternError::RepeatedDirection { index: index + 1 });
    }
    Ok(())
}

/// Cloning captures an O(1) snapshot; later registration copies shared trie storage.
#[derive(Clone)]
pub struct Matcher {
    pub(crate) storage: Arc<Trie>,
}
impl Default for Matcher {
    fn default() -> Self {
        Self {
            storage: Arc::new(Trie::new()),
        }
    }
}
impl Matcher {
    /// Replaces an exact reachable pattern of 1..=4096 direction runs. Consecutive
    /// equal directions cannot be emitted by recognition and are rejected without
    /// mutation. Zero and u64::MAX are valid IDs. Set a session budget at least as
    /// large as the patterns you intend to recognize.
    pub fn register(&mut self, pattern: &[Direction], identifier: u64) -> Result<(), PatternError> {
        validate_pattern(pattern)?;
        Arc::make_mut(&mut self.storage).register(pattern.iter().copied(), identifier);
        Ok(())
    }
    /// Remove one exact pattern and return its identifier. Missing or invalid
    /// patterns leave this matcher unchanged. Clones and active sessions retain
    /// their captured registrations. Vacated trie storage is reused on insertion.
    pub fn remove(&mut self, pattern: &[Direction]) -> Option<u64> {
        if !crate::validate_direction_limit(pattern.len()) || self.match_pattern(pattern).is_none()
        {
            return None;
        }
        Arc::make_mut(&mut self.storage).remove(pattern)
    }
    /// Remove every registration and release this matcher's trie storage.
    /// Existing snapshots remain valid and independently mutable.
    pub fn clear(&mut self) {
        self.storage = Arc::new(Trie::new());
    }
    /// Look up an exact pattern. Empty input and unmatched prefixes return none.
    pub fn match_pattern(&self, pattern: &[Direction]) -> Option<u64> {
        if pattern.is_empty() {
            return None;
        }
        self.storage.lookup(pattern.iter().copied())
    }
    /// Whether no nonempty pattern has been registered.
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }
}

use crate::Direction;
#[derive(Clone, Default)]
pub(crate) struct Node {
    pub(crate) children: [Option<usize>; 4],
    pub(crate) terminal: Option<u64>,
}

#[derive(Clone)]
pub(crate) struct Trie {
    pub(crate) nodes: Vec<Node>,
    free: Vec<usize>,
}

impl Trie {
    pub(crate) fn new() -> Self {
        Self {
            nodes: vec![Node::default()],
            free: vec![],
        }
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.nodes[0].children.iter().all(Option::is_none) && self.nodes[0].terminal.is_none()
    }
    pub(crate) fn register(&mut self, directions: impl IntoIterator<Item = Direction>, value: u64) {
        let mut cursor = 0;
        for direction in directions {
            let index = direction as usize - 1;
            cursor = if let Some(child) = self.nodes[cursor].children[index] {
                child
            } else {
                let child = self.free.pop().unwrap_or_else(|| {
                    self.nodes.push(Node::default());
                    self.nodes.len() - 1
                });
                self.nodes[cursor].children[index] = Some(child);
                child
            };
        }
        self.nodes[cursor].terminal = Some(value);
    }
    fn remove(&mut self, directions: &[Direction]) -> Option<u64> {
        let mut cursor = 0;
        let mut path = Vec::with_capacity(directions.len());
        for &direction in directions {
            let slot = direction as usize - 1;
            let child = self.nodes[cursor].children[slot]?;
            path.push((cursor, slot, child));
            cursor = child;
        }
        let value = self.nodes[cursor].terminal.take()?;
        for (parent, slot, child) in path.into_iter().rev() {
            let node = &self.nodes[child];
            if node.terminal.is_some() || node.children.iter().any(Option::is_some) {
                break;
            }
            self.nodes[parent].children[slot] = None;
            self.free.push(child);
        }
        Some(value)
    }
    pub(crate) fn advance(&self, cursor: Option<usize>, direction: Direction) -> Option<usize> {
        self.nodes
            .get(cursor?)?
            .children
            .get(direction as usize - 1)
            .copied()
            .flatten()
    }
    pub(crate) fn lookup(&self, directions: impl IntoIterator<Item = Direction>) -> Option<u64> {
        let mut cursor = Some(0);
        for direction in directions {
            cursor = self.advance(cursor, direction);
        }
        self.nodes.get(cursor?)?.terminal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removal_preserves_prefixes_descendants_and_snapshots() {
        let mut matcher = Matcher::default();
        matcher.register(&[Direction::Up], 0).unwrap();
        matcher
            .register(&[Direction::Up, Direction::Right], u64::MAX)
            .unwrap();
        matcher.register(&[Direction::Down], 42).unwrap();
        let snapshot = matcher.clone();
        assert_eq!(matcher.remove(&[Direction::Up]), Some(0));
        assert_eq!(matcher.match_pattern(&[Direction::Up]), None);
        assert_eq!(
            matcher.match_pattern(&[Direction::Up, Direction::Right]),
            Some(u64::MAX)
        );
        assert_eq!(matcher.remove(&[Direction::Up]), None);
        assert_eq!(matcher.remove(&[]), None);
        assert_eq!(snapshot.match_pattern(&[Direction::Up]), Some(0));
        assert_eq!(
            matcher.remove(&[Direction::Up, Direction::Right]),
            Some(u64::MAX)
        );
        assert_eq!(matcher.match_pattern(&[Direction::Down]), Some(42));
        matcher.clear();
        assert!(matcher.is_empty());
        assert_eq!(snapshot.match_pattern(&[Direction::Down]), Some(42));
        matcher.register(&[Direction::Right], 7).unwrap();
        assert_eq!(matcher.match_pattern(&[Direction::Right]), Some(7));
    }

    #[test]
    fn registration_churn_reuses_vacated_storage_and_deep_removal_is_iterative() {
        let mut matcher = Matcher::default();
        for index in 0..1000 {
            let direction = [
                Direction::Up,
                Direction::Down,
                Direction::Left,
                Direction::Right,
            ][index % 4];
            matcher.register(&[direction], index as u64).unwrap();
            assert_eq!(matcher.remove(&[direction]), Some(index as u64));
        }
        assert!(matcher.is_empty());
        assert_eq!(matcher.storage.nodes.len(), 2);
        let deep: Vec<_> = (0..4096)
            .map(|i| {
                if i % 2 == 0 {
                    Direction::Right
                } else {
                    Direction::Down
                }
            })
            .collect();
        matcher.register(&deep, 9).unwrap();
        assert_eq!(matcher.remove(&deep), Some(9));
        assert!(matcher.is_empty());
        matcher.clear();
        assert_eq!(matcher.storage.nodes.len(), 1);
    }
}
