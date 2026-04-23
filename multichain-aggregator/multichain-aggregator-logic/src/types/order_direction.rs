/// Sort order for list endpoints (e.g. "asc" or "desc").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum OrderDirection {
    Asc,
    #[default]
    Desc,
}
