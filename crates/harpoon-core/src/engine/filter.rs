use crate::error::HarpoonError;
use crate::types::filter::{Direction, Filter, FilterAction, FilterKind};

pub struct CompiledFilter {
    pub filter: Filter,
    #[cfg(feature = "regex-filter")]
    compiled_regex: Option<regex::bytes::Regex>,
}

impl CompiledFilter {
    pub fn new(filter: Filter) -> Result<Self, HarpoonError> {
        #[cfg(feature = "regex-filter")]
        let compiled_regex = match &filter.kind {
            FilterKind::Regex(pattern) => {
                let re = regex::bytes::Regex::new(pattern)
                    .map_err(|e| HarpoonError::Filter(format!("invalid regex: {e}")))?;
                Some(re)
            }
            _ => None,
        };

        Ok(Self {
            filter,
            #[cfg(feature = "regex-filter")]
            compiled_regex,
        })
    }

    pub fn matches(&self, data: &[u8]) -> bool {
        match &self.filter.kind {
            FilterKind::Substr(s) => {
                let pattern = s.as_bytes();
                data.windows(pattern.len()).any(|w| w == pattern)
            }
            FilterKind::BinarySubstr(pattern) => {
                data.windows(pattern.len()).any(|w| w == pattern.as_slice())
            }
            #[cfg(feature = "regex-filter")]
            FilterKind::Regex(_) => self
                .compiled_regex
                .as_ref()
                .map(|re| re.is_match(data))
                .unwrap_or(false),
        }
    }

    pub fn applies_to_direction(&self, direction: &Direction) -> bool {
        match &self.filter.direction {
            Direction::Both => true,
            d => d == direction,
        }
    }
}

pub fn apply_filters(
    filters: &[CompiledFilter],
    data: &[u8],
    direction: &Direction,
) -> (FilterAction, Option<usize>) {
    for (i, f) in filters.iter().enumerate() {
        if !f.applies_to_direction(direction) {
            continue;
        }
        if f.matches(data) {
            return (f.filter.action_on_match.clone(), Some(i));
        }
    }
    (FilterAction::Pass, None)
}
