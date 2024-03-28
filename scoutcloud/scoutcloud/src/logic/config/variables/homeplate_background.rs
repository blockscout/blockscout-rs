use crate::logic::config::macros;

pub struct HomeplateBackground {}

macros::single_env_var!(
    HomeplateBackground,
    String,
    frontend,
    "NEXT_PUBLIC_HOMEPAGE_PLATE_BACKGROUND",
    None
);
