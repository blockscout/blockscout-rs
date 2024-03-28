use crate::logic::config::macros;

pub struct HomeplateTextColor {}

macros::single_env_var!(
    HomeplateTextColor,
    String,
    frontend,
    "NEXT_PUBLIC_HOMEPAGE_PLATE_TEXT_COLOR",
    None
);
