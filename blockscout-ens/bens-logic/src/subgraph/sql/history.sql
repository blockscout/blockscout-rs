SELECT transaction_id, block_number, array_agg(table_name) as actions
FROM (
    SELECT distinct on (transaction_id, table_name) *
    FROM (
        {% for table_name in domain_event_tables -%}
        SELECT '{{table_name}}' as table_name, block_number, transaction_id
        FROM {{schema}}.{{table_name}}
        WHERE domain = $1
            UNION ALL
        {% endfor -%}

        {% for table_name in resolver_event_tables -%}
        SELECT '{{table_name}}' as table_name, t.block_number, t.transaction_id 
        FROM {{schema}}.{{table_name}} t
        JOIN {{schema}}.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
            UNION ALL
        {% endfor -%}

        {% for table_name in registration_event_tables -%}
        SELECT '{{table_name}}' as table_name, t.block_number, t.transaction_id 
        FROM {{schema}}.{{table_name}} t
        JOIN {{schema}}.registration r
        ON t.registration = r.id
        WHERE r.domain = $1
        {% if loop.last -%}{% continue -%}{% endif -%}
            UNION ALL
        {% endfor -%}
    ) all_events
    ORDER BY transaction_id
) unique_events
GROUP BY transaction_id, block_number
ORDER BY {{sort}} {{order}}

