SELECT
        -- CrossChainTx fields
        cctx.id as cctx_id,--0
        cctx.creator,--1
        cctx.index,--2
        cctx.zeta_fees,--3
        cctx.retries_number,--4
        cctx.processing_status::text,--5
        cctx.relayed_message,--6
        cctx.last_status_update_timestamp,--7
        cctx.protocol_contract_version::text,--8
        cctx.root_id,--9
        cctx.parent_id,--10
        cctx.depth,--11
        cctx.updated_by,--12
        -- CctxStatus fields
        cs.id as status_id,--13
        cs.cross_chain_tx_id as status_cross_chain_tx_id,--14
        cs.status::text,--15
        cs.status_message::text,--16
        cs.error_message,--17
        cs.last_update_timestamp,--18
        cs.is_abort_refunded,--19
        cs.created_timestamp,--20
        cs.error_message_revert,--21
        cs.error_message_abort,--22
        -- InboundParams fields
        ip.id as inbound_id,--23
        ip.cross_chain_tx_id as inbound_cross_chain_tx_id,--24
        ip.sender,--25
        ip.sender_chain_id,--26
        ip.tx_origin,--27
        ip.coin_type::text,--28
        ip.asset,--29
        ip.amount,--30
        ip.observed_hash,--31
        ip.observed_external_height,--32
        ip.ballot_index,--33
        ip.finalized_zeta_height,--34
        ip.tx_finalization_status::text,--35
        ip.is_cross_chain_call,--36
        ip.status::text as inbound_status,--37
        ip.confirmation_mode::text as inbound_confirmation_mode,--38
        -- RevertOptions fields
        ro.id as revert_id,--39
        ro.cross_chain_tx_id as revert_cross_chain_tx_id,--40
        ro.revert_address,--41
        ro.call_on_revert,--42
        ro.abort_address,--43
        ro.revert_message,--44
        ro.revert_gas_limit,--45
        t.symbol as token_symbol,--46
        t.zrc20_contract_address as zrc20_contract_address,--47
        t.icon_url as icon_url,--48
        t.decimals as decimals,--49
        t.name as token_name--50

        FROM cross_chain_tx cctx
        LEFT JOIN cctx_status cs ON cctx.id = cs.cross_chain_tx_id
        LEFT JOIN inbound_params ip ON cctx.id = ip.cross_chain_tx_id
        LEFT JOIN revert_options ro ON cctx.id = ro.cross_chain_tx_id
        JOIN token t ON t.id = cctx.token_id
        WHERE cctx.index = $1