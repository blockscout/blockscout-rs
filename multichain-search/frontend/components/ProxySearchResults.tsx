import { SearchIcon } from '@chakra-ui/icons';
import { Tabs, TabList, Tab, TabPanel, TabPanels, Flex } from '@chakra-ui/react';
import React from 'react';
import styles from '../styles/search.module.css'
import { ProxyResponse } from '../types/proxyResponse';
import { Network } from './Network';
import { ResultTable } from './ResultTable';

interface Props {
    responses: ProxyResponse
}

const background = "#edf2f7"

export const ProxySearchResults = ({responses}: Props) => {
    if (responses) {
        let chains = Object.keys(responses).sort()
        return (<Flex className={styles.results} fontSize="14">
        <Tabs isFitted variant='soft-rounded' size="sm" colorScheme='purple'>
            <TabList flexWrap="wrap" gap="5">
                <Tab background={background}> <Network instance={{
                    url: "",
                    title: "All",
                    id: "all"
                }} /> </Tab>
                {chains.map((chain) => <Tab key={chain} background={background}> <Network instance={responses[chain].instance}/> </Tab>)}
            </TabList>
            <TabPanels width="100%">
                <TabPanel>
                    <ResultTable responses={Object.values(responses)}></ResultTable>
                </TabPanel>
                {chains.map((chain) => {
                    return <TabPanel key={chain}>
                        <ResultTable responses={[responses[chain]]}/>
                    </TabPanel>
                })}
            </TabPanels>
        </Tabs>
        </Flex>)
    } else {
        return <>No results found</>
    }
}