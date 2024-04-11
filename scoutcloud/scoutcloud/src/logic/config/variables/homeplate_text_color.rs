use crate::logic::config::macros;

macros::simple_env_var!(
    HomeplateTextColor,
    String,
    FrontendEnv,
    "NEXT_PUBLIC_HOMEPAGE_PLATE_TEXT_COLOR"
);
