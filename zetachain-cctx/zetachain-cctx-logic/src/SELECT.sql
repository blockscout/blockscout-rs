SELECT
    all_descendants.* , tree.depth, tree.descendant_id
FROM
    cross_chain_transaction all_descendants
    JOIN cctx_closure tree on tree.cctx_id = all_descendants.id
    AND tree.ancestor_id = ancestor_id
    FROM cctx_closure ancestor
    WHERE ancestor.id in (
        SELECT
            cl.ancestor_id
        FROM
            cctx_closure ancestor
            join cross_chain_transaction cctx
        WHERE
            cctx.index = ?
            AND ancestor.cctx_id = cctx.id
    )
ORDER BY tree.depth DESC