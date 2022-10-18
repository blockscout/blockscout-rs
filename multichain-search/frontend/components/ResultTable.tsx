import { TableContainer, Table, Thead, Tr, Th, Tbody, Flex } from '@chakra-ui/react';
import React from 'react';
import { InstanceResponse, InstanceSearchResponse, InstanceSearchResponseItem } from '../types/proxyResponse';
import { ResultTableItem } from './ResultTableItem';

interface Props {
    responses: InstanceResponse[]
}


export const ResultTable = ({responses}: Props) => {
    let corrent_responses = responses
        .filter((r) => r.status == 200)
        .map((r) => {
            let response = JSON.parse(r.content) as InstanceSearchResponse

            return {
                instance: r.instance, 
                items: response.items
            }
        })
        .filter(({items}) => items.length > 0)
    
    let empty = corrent_responses.length == 0;
    let table_content = <Tbody>{corrent_responses.map(({instance, items}, index) => {
            return items.map((item) => {
                return <ResultTableItem key={ index } instance = { instance } data={item} />
            })
        }).flat()}
        </Tbody>
    return (
        <TableContainer width="100%" mt={ 8 }>
            <Table variant="simple" minWidth="1000px" size="md" fontWeight={ 500 }>
            <Thead>
                <Tr>
                <Th width="15%">Network</Th>
                <Th width="40%">Result</Th>
                <Th width="30%">Hash</Th>
                <Th width="15%">Category</Th>
                </Tr>
            </Thead>
                {empty ? undefined : table_content}
            </Table>

            {empty ? <Flex alignItems="center" flexDirection="column" padding="5">no content</Flex> : undefined}
        </TableContainer>
        );

}