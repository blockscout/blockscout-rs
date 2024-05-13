use sqlx::FromRow;

#[derive(Debug, FromRow)]
pub struct Protocol {
    pub id: i32,
    pub slug: String,
    pub tld: String,
    pub title: String,
    pub description: String,
    pub icon_url: String,
}

#[derive(Debug, FromRow)]
pub struct Network {
    pub network_id: String,
    pub title: String,
}

#[derive(Debug, FromRow)]
pub struct NetworkProtocol {
    pub network_id: String,
    pub protocol_id: i32,
}
