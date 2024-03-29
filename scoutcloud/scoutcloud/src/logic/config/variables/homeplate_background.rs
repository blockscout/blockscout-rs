use crate::logic::config::macros;

macros::simple_env_var!(
    HomeplateBackground,
    String,
    frontend,
    "NEXT_PUBLIC_HOMEPAGE_PLATE_BACKGROUND",
    None
);
