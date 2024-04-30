use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_statements(manager, [
            r#"
            -- Trigger function to update user balance after insert, update, or delete on balance_changes
            CREATE OR REPLACE FUNCTION update_user_balance()
            RETURNS TRIGGER AS $$
            BEGIN
                IF TG_OP = 'INSERT' THEN
                    -- Update user balance when a new balance change is inserted
                    UPDATE users
                    SET balance = balance + NEW.amount
                    WHERE id = NEW.user_id;
                ELSIF TG_OP = 'UPDATE' THEN
                    -- Update user balance when a balance change is updated
                    UPDATE users
                    SET balance = balance + (NEW.amount - OLD.amount)
                    WHERE id = NEW.user_id;
                ELSIF TG_OP = 'DELETE' THEN
                    -- Update user balance when a balance change is deleted
                    UPDATE users
                    SET balance = balance - OLD.amount
                    WHERE id = OLD.user_id;
                END IF;
                
                RETURN NULL;
            END;
            $$ LANGUAGE plpgsql;"#,
            r#"
            -- Create trigger for balance_changes
            CREATE TRIGGER balance_changes_trigger
            AFTER INSERT OR UPDATE OR DELETE
            ON balance_changes
            FOR EACH ROW
            EXECUTE FUNCTION update_user_balance();
            "#,
            r#"
            -- Trigger function to update user balance after insert, update, or delete on balance_expenses
            CREATE OR REPLACE FUNCTION update_for_balance_expenses()
            RETURNS TRIGGER AS $$
            BEGIN
                IF TG_OP = 'INSERT' THEN
                    -- Update user balance when a new balance expense is inserted
                    UPDATE users
                    SET balance = balance - NEW.expense_amount
                    WHERE id = NEW.user_id;

                    UPDATE deployments
                    SET total_cost = total_cost + NEW.expense_amount
                    WHERE id = NEW.deployment_id;
                ELSIF TG_OP = 'UPDATE' THEN
                    UPDATE users
                    SET balance = balance - (NEW.expense_amount - OLD.expense_amount)
                    WHERE id = NEW.user_id;

                    UPDATE deployments
                    SET total_cost = total_cost + (NEW.expense_amount - OLD.expense_amount)
                    WHERE id = NEW.deployment_id;
                ELSIF TG_OP = 'DELETE' THEN
                    -- Update user balance when a balance expense is deleted
                    UPDATE users
                    SET balance = balance + OLD.expense_amount
                    WHERE id = OLD.user_id;

                    UPDATE deployments
                    SET total_cost = total_cost - OLD.expense_amount
                    WHERE id = OLD.deployment_id;
                END IF;
            
                RETURN NULL;
            END;
            $$ LANGUAGE plpgsql;"#,
            r#"
            -- Create trigger for balance_expenses
            CREATE TRIGGER balance_expenses_trigger
            AFTER INSERT OR UPDATE OR DELETE
            ON balance_expenses
            FOR EACH ROW
            EXECUTE FUNCTION update_for_balance_expenses();"#,
        ].as_ref()).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            r#"
            DROP TRIGGER IF EXISTS balance_changes_trigger ON balance_changes;
            DROP TRIGGER IF EXISTS balance_expenses_trigger ON balance_expenses;

            DROP FUNCTION IF EXISTS update_for_balance_expenses() CASCADE;
            DROP FUNCTION IF EXISTS update_user_balance() CASCADE;"#,
        )
        .await?;
        Ok(())
    }
}
