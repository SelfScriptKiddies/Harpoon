#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Direction {
    ClientToServer,
    ServerToClient,
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterAction {
    Pass,
    Drop,
    TapOnly,
    /// Drop the entire connection (TCP: close, UDP: remove session).
    DropConnection,
}

#[derive(Debug, Clone)]
pub enum FilterKind {
    Substr(String),
    BinarySubstr(Vec<u8>),
    #[cfg(feature = "regex-filter")]
    Regex(String),
}

#[derive(Debug, Clone)]
pub struct Filter {
    pub kind: FilterKind,
    pub direction: Direction,
    pub action_on_match: FilterAction,
}
