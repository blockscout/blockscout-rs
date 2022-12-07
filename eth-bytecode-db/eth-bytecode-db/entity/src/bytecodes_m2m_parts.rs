use sea_orm::{Related, RelationDef, RelationTrait};

// https://www.sea-ql.org/SeaORM/docs/next/relation/many-to-many/
impl Related<super::parts::Entity> for super::bytecodes::Entity {
    fn to() -> RelationDef {
        super::bytecode_parts::Relation::Parts.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::bytecode_parts::Relation::Bytecodes.def().rev())
    }
}
