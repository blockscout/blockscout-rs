use crate::types::*;
use futures::try_join;
use sea_orm::{ConnectionTrait, DatabaseConnection, FromQueryResult, Statement};

pub async fn get_metadata(
    db: &DatabaseConnection,
    request: GetMetadataRequest,
) -> anyhow::Result<GetMetadataResponse> {
    let chain_id = request.chain_id;
    
    // TODO: db uses 0x-prefixed addresses, should be changed later
    let address = format!("{:?}", request.address);
    let address = address.as_bytes();

    let notes_fut = async {
        match request.notes {
            Some(NotesRequest { limit }) => {
                NoteData::find_by_statement(Statement::from_sql_and_values(
                    db.get_database_backend(),
                    r#"
                    SELECT
                        n.text,
                        n.severity,
                        n.ordinal,
                        n.meta
                    FROM address_notes as an
                            INNER JOIN notes as n ON n.id = an.note_id
                    WHERE an.address = $1 AND (an.chain_id IS NULL OR an.chain_id=$2)
                    ORDER BY n.ordinal DESC
                    LIMIT $3;
                    "#,
                    [address.into(), chain_id.into(), limit.into()],
                ))
                .all(db)
                .await
                .map(Some)
            }
            None => Ok(None),
        }
    };

    let tags_fut = async {
        match request.tags {
            Some(TagsRequest { limit }) => {
                TagData::find_by_statement(Statement::from_sql_and_values(
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
                    WHERE apt.address = $1 AND (apt.chain_id IS NULL OR apt.chain_id=$2)
                    ORDER BY pt.ordinal DESC
                    LIMIT $3;
                    "#,
                    [address.into(), chain_id.into(), limit.into()],
                ))
                .all(db)
                .await
                .map(Some)
            }
            None => Ok(None),
        }
    };

    let reputation_fut = async {
        match request.reputation {
            Some(ReputationRequest {}) => {
                ReputationData::find_by_statement(Statement::from_sql_and_values(
                    db.get_database_backend(),
                    r#"
                    SELECT reputation AS score
                    FROM address_reputation
                    WHERE address = $1
                    "#,
                    [address.into()],
                ))
                .one(db)
                .await
            }
            None => Ok(None),
        }
    };

    let metadata = try_join!(notes_fut, tags_fut, reputation_fut)?;

    Ok(GetMetadataResponse {
        notes: metadata.0,
        tags: metadata.1,
        reputation: metadata.2,
    })
}
