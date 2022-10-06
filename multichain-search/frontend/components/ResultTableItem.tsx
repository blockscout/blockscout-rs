import { Link, Tr, Td } from '@chakra-ui/react';
import React from 'react';
import { Instance, InstanceSearchResponseItem } from '../types/proxyResponse';


interface Props {
    instance: Instance
    data: InstanceSearchResponseItem,
  }



  

export const ResultTableItem = ({instance, data}: Props) => {
    let url = "";
    let title = "";
    let hash = ""

    if (data.type  === 'token') {
        url = data.address_url
        title = data.name
        hash = data.address
    }
    else if (data.type === 'contract') {
        url = data.url
        title = data.name
        hash = data.address
    }
    else if (data.type === 'address') {
        url = data.url
        title = data.name || "Address"
        hash = data.address
    }
    else if (data.type === 'block') {
        url = data.url
        title = 'block#' + data.block_number
        hash = data.block_hash
    }
    else if (data.type === 'transaction') {
        url = data.url
        title = "transaction"
        hash = data.tx_hash
    }
    url = instance.url + url

    return (
    <Tr>
      <Td>
        <Link href={instance.url}>{instance.title}</Link>
      </Td>

      <Td>
        <Link href={url}>{title}</Link>
      </Td>

      <Td>
        <Link href={url}>{hash}</Link>
      </Td>

      <Td>
        {data.type}
      </Td>
      {/* <Td fontSize="sm">
        <Flex columnGap={ 2 } alignItems="center">
          <Link
            fontWeight={ 600 }
            href={ link('block_index', { id: String(data.height) }) }
          >
            { data.height }
          </Link>
        </Flex>
        <Text variant="secondary" mt={ 2 } fontWeight={ 400 }>{ dayjs(data.timestamp).locale('en-short').fromNow() }</Text>
      </Td>
      <Td fontSize="sm">{ data.size.toLocaleString('en') } bytes</Td>
      <Td fontSize="sm">
        <AddressLink alias={ data.miner?.name } hash={ data.miner.address } truncation="constant"/>
      </Td>
      <Td isNumeric fontSize="sm">{ data.transactionsNum }</Td>
      <Td fontSize="sm">
        <Box>{ data.gas_used.toLocaleString('en') }</Box>
        <Flex mt={ 2 }>
          <Utilization colorScheme="gray" value={ data.gas_used / data.gas_limit }/>
          <GasUsedToTargetRatio ml={ 2 } used={ data.gas_used } target={ data.gas_target }/>
        </Flex>
      </Td>
      <Td fontSize="sm">{ (data.reward.static + data.reward.tx_fee - data.burnt_fees).toLocaleString('en', { maximumFractionDigits: 5 }) }</Td>
      <Td fontSize="sm">
        <Flex alignItems="center" columnGap={ 1 }>
          <Icon as={ flameIcon } boxSize={ 5 } color={ useColorModeValue('gray.500', 'inherit') }/>
          { data.burnt_fees.toLocaleString('en', { maximumFractionDigits: 6 }) }
        </Flex>
        <Tooltip label="Burnt fees / Txn fees * 100%">
          <Box>
            <Utilization mt={ 2 } value={ data.burnt_fees / data.reward.tx_fee }/>
          </Box>
        </Tooltip>
      </Td> */}
    </Tr>
        );

}