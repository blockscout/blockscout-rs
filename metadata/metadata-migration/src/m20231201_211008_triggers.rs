use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let set_created_at_and_created_by = r#"
            CREATE OR REPLACE FUNCTION set_created_at_and_created_by()
                RETURNS TRIGGER AS
            $$
            BEGIN
                NEW.created_at = now();
                NEW.created_by = current_user;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;
        "#;
        let not_update_created_at_and_created_by = r#"
            CREATE OR REPLACE FUNCTION not_update_created_at_and_created_by()
                RETURNS TRIGGER AS
            $$
            BEGIN
                NEW.created_at = OLD.created_at;
                NEW.created_by = OLD.created_by;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;
        "#;
        let set_modified_at_and_modified_by = r#"
            CREATE OR REPLACE FUNCTION set_modified_at_and_modified_by()
                RETURNS TRIGGER AS
            $$
            BEGIN
                NEW.modified_at = now();
                NEW.modified_by = current_user;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;
        "#;
        let create_triggers = r#"
            DO
            $$
                DECLARE
                    t_name text;
                BEGIN
                    FOR t_name IN (VALUES ('public_tags'),
                                        ('address_public_tags'),
                                        ('notes'),
                                        ('address_notes'), ('address_reputation'))
                        LOOP
                            EXECUTE format('CREATE TRIGGER trigger_set_created_by_and_created_at
                                    BEFORE INSERT ON %I
                                        FOR EACH ROW
                                    EXECUTE FUNCTION set_created_at_and_created_by()',
                                        t_name);

                            EXECUTE format('CREATE TRIGGER trigger_make_created_by_and_created_at_not_updatable
                                    BEFORE INSERT ON %I
                                        FOR EACH ROW
                                    EXECUTE FUNCTION not_update_created_at_and_created_by()',
                                        t_name);

                            EXECUTE format('CREATE TRIGGER trigger_set_modified_by_and_modified_at
                                    BEFORE INSERT ON %I
                                        FOR EACH ROW
                                    EXECUTE FUNCTION set_modified_at_and_modified_by()',
                                        t_name);
                        END LOOP;
                END;
            $$ LANGUAGE plpgsql;
        "#;
        crate::exec_stmts(
            manager,
            [
                set_created_at_and_created_by,
                not_update_created_at_and_created_by,
                set_modified_at_and_modified_by,
                create_triggers,
            ],
        )
        .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        let drop_triggers = r#"
            DO
            $$
                DECLARE
                    t_name text;
                BEGIN
                    FOR t_name IN (VALUES ('public_tags'),
                                        ('address_public_tags'),
                                        ('notes'),
                                        ('address_notes'), ('address_reputation'))
                        LOOP
                            EXECUTE format('DROP TRIGGER trigger_set_created_by_and_created_at ON %I', t_name);
                            EXECUTE format('DROP TRIGGER trigger_make_created_by_and_created_at_not_updatable ON %I', t_name);
                            EXECUTE format('DROP TRIGGER trigger_set_modified_by_and_modified_at ON %I', t_name);
                        END LOOP;
                END;
            $$ LANGUAGE plpgsql;
        "#;
        crate::exec_stmts(_manager, [drop_triggers]).await
    }
}
