use crate::logic::config::macros;

macros::simple_env_var!(
    HomeplateBackground,
    String,
    FrontendEnv,
    "NEXT_PUBLIC_HOMEPAGE_PLATE_BACKGROUND"
);
