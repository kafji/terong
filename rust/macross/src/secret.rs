use core::fmt;

/// Secret container.
///
/// A simple container that will redact its content from being printed.
///
/// ```rust
/// let token = macross::secret::Secret("secret_token");
/// println!("Secret token is: `{token}`.");
/// ```
#[derive(Clone, PartialEq, Default)]
pub struct Secret<T>(pub T);

impl<T> Copy for Secret<T> where T: Copy {}

impl<T> From<T> for Secret<T> {
    fn from(s: T) -> Self {
        Self(s)
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Secret").field(&"█████").finish()
    }
}

impl<T> fmt::Display for Secret<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "█████")
    }
}

impl<T> Secret<T> {
    /// Convert `&Secret<T>` to `Secret<&T>`.
    pub fn as_ref(&self) -> Secret<&T> {
        Secret(&self.0)
    }

    /// Map the contained value.
    pub fn map<U, F>(self, f: F) -> Secret<U>
    where
        F: FnOnce(T) -> U,
    {
        let v = f(self.0);
        Secret(v)
    }

    /// Unwrap `Secret` returning the contained value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

#[cfg(feature = "serde")]
impl<'de, T> serde::Deserialize<'de> for Secret<T>
where
    T: serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = T::deserialize(deserializer)?;
        let s = Secret(v);
        Ok(s)
    }
}

#[cfg(feature = "serde")]
impl<T> serde::Serialize for Secret<T>
where
    T: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let v = T::serialize(&self.0, serializer)?;
        Ok(v)
    }
}
