import { Tabs, TabList, Tab, TabPanel, TabPanels } from '@chakra-ui/react';
import React from 'react';
import styles from '../styles/search.module.css'
import { ProxyResponse } from '../types/proxyResponse';
import { ResultTable } from './ResultTable';

interface Props {
    responses: ProxyResponse
}

export const ProxySearchResults = ({responses}: Props) => {
    let chains = Object.keys(responses)
    return (<div className={styles.results}>
       <Tabs variant='soft-rounded' size='md' colorScheme='messenger'>
        <TabList>
            <Tab>All</Tab>
            {chains.map((chain) => <Tab id={chain}>{responses[chain].instance.title}</Tab>)}
        </TabList>
        <TabPanels>
            <TabPanel>
                <ResultTable responses={Object.values(responses)}></ResultTable>
            </TabPanel>
            {chains.map((chain) => {
                return <TabPanel id={chain}>
                    <ResultTable responses={[responses[chain]]}/>
                </TabPanel>
            })}
        </TabPanels>
       </Tabs>
    </div>)
}