use crate::types::*;
use sea_orm::{ConnectionTrait, DatabaseConnection, FromQueryResult, Statement};

pub async fn get_metadata(
    db: &DatabaseConnection,
    request: GetMetadataRequest,
) -> anyhow::Result<GetMetadataResponse> {
    let mut response: GetMetadataResponse = Default::default();

    let chain_id = request.chain_id;
    let address = request.address.as_bytes();

    if let Some(NotesRequest { limit }) = request.notes {
        let notes = NoteData::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
            SELECT
                n.text,
                n.severity,
                n.ordinal,
                n.meta
            FROM address_notes as an
                    INNER JOIN notes as n ON n.id = an.note_id
            WHERE an.address = $1 AND an.chain_id = $2
            ORDER BY n.ordinal DESC
            LIMIT $3;
            "#,
            [address.into(), chain_id.into(), limit.into()],
        ))
        .all(db)
        .await?;

        response.notes = Some(notes);
    };

    if let Some(TagsRequest { limit }) = request.tags {
        let tags = TagData::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
            SELECT
                pt.slug,
                pt.name,
                CAST(pt.tag_type AS text),
                pt.ordinal,
                apt.overrided_meta || pt.meta AS meta
            FROM address_public_tags as apt
                    INNER JOIN public_tags as pt ON pt.id = apt.public_tag_id
            WHERE apt.address = $1 AND apt.chain_id = $2
            ORDER BY pt.ordinal DESC
            LIMIT $3;
            "#,
            [address.into(), chain_id.into(), limit.into()],
        ))
        .all(db)
        .await?;
        response.tags = Some(tags);
    };

    if let Some(ReputationRequest {}) = request.reputation {
        // TODO: probably need to filter by created_by
        let reputation = ReputationData::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
            SELECT reputation AS score
            FROM address_reputation
            WHERE address = $1
            "#,
            [address.into()],
        ))
        .one(db)
        .await?;
        response.reputation = reputation;
    };
    Ok(response)
}
