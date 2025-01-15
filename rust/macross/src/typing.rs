/// Define a new wrapping type similar to Haskell's `newtype`.
///
/// # Example.
///
/// ```rust
/// use macross::newtype;
///
/// newtype! {
///     #[derive(Debug)]
///     pub Address = String;
/// }
///
/// let addr: Address = "Easy St.".to_owned().into();
///
/// assert_eq!("Easy St.", addr.as_ref());
/// ```
#[macro_export]
macro_rules! newtype {
    {
        $(#[$($meta:meta)*])*
        $vis:vis $name:ident = $ty:ty;
    } => {
        $(#[$($meta)*])*
        $vis struct $name($vis $ty);

        impl AsRef<$ty> for $name {
            fn as_ref(&self) -> &$ty {
                &self.0
            }
        }

        impl From<$ty> for $name {
            fn from(x: $ty) -> Self {
                Self(x)
            }
        }

        impl Into<$ty> for $name {
            fn into(self) -> $ty {
                self.0
            }
        }
    };
}

#[macro_export]
macro_rules! impl_from {
    ($dst:ty, { $($f:expr => $src:ty,)* }) => {
        $(
            impl From<$src> for $dst {
                fn from(x: $src) -> Self {
                    $f(x)
                }
            }
        )*
    };
}
