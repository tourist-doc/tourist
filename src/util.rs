/// Pulled from the `maplit` trait:
/// https://crates.io/crates/maplit
#[macro_export]
macro_rules! kv {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$(kv!(@single $rest)),*]));

    ($($key:expr => $value:expr,)+) => { kv!($($key => $value),+) };
    ($($key:expr => $value:expr),*) => {
        {
            let _cap = kv!(@count $($key),*);
            let mut _map = ::std::collections::HashMap::with_capacity(_cap);
            $(
                let _ = _map.insert($key, $value);
            )*
             _map
        }
    };
}
