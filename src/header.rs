use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    hash::{Hash, Hasher},
};

pub struct HeaderName(String);

impl HeaderName {
    pub fn to_string(self: Self) -> String {
        self.into()
    }
}

impl From<String> for HeaderName {
    fn from(string: String) -> Self {
        let string = string
            .split('-')
            .map(|word| -> String {
                let mut chars = word.chars();
                return match chars.next() {
                    Some(char) => char.to_uppercase().collect::<String>() + chars.as_str(),
                    None => "".to_string(),
                };
            })
            .intersperse("-".to_string())
            .collect::<String>();
        Self(string)
    }
}

impl<'a> From<&'a str> for HeaderName {
    #[inline]
    fn from(str: &'a str) -> Self {
        Self::from(str.to_string())
    }
}

impl From<HeaderName> for String {
    #[inline]
    fn from(header: HeaderName) -> Self {
        header.0.to_string()
    }
}

impl PartialEq for HeaderName {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    #[inline]
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

impl Eq for HeaderName {}

impl Hash for HeaderName {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Debug for HeaderName {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for HeaderName {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

pub type Headers = HashMap<HeaderName, String>;
