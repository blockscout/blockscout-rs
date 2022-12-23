use sea_orm::{Related, RelationDef, RelationTrait};

// https://www.sea-ql.org/SeaORM/docs/next/relation/many-to-many/
impl Related<super::files::Entity> for super::sources::Entity {
    fn to() -> RelationDef {
        super::source_files::Relation::Files.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::source_files::Relation::Sources.def().rev())
    }
}
