import { Link, Tr, Td, Icon } from '@chakra-ui/react';
import React from 'react';
import { Instance, InstanceSearchResponseItem } from '../types/proxyResponse';
import { Network } from './Network';
const path = require('path');

import styles from '../styles/search.module.css'


interface Props {
  instance: Instance
  data: InstanceSearchResponseItem,
}


export const ResultTableItem = ({instance, data}: Props) => {
    let url_title = "";
    let url_hash = ""
    let title = "";
    let hash = ""

    if (data.type  === 'token') {
      url_title = data.token_url
      url_hash = data.address_url
      title = data.name
      hash = data.address
    }
    else if (data.type === 'contract') {
      url_title = url_hash = data.url
      title = data.name
      hash = data.address
    }
    else if (data.type === 'address') {
      url_title = url_hash = data.url
      title = data.name || ""
      hash = data.address
    }
    else if (data.type === 'block') {
      url_title = url_hash = data.url
      title = 'block#' + data.block_number
      hash = data.block_hash
    }
    else if (data.type === 'transaction') {
      url_title = url_hash = data.url
      title = ""
      hash = data.tx_hash
    }
    url_title = new URL(url_title, instance.url).toString();
    url_hash = new URL(url_hash, instance.url).toString();

    return (
    <Tr>
      <Td>
        <Network instance={instance} addUrl/>
      </Td>

      <Td>
        <Link href={url_title}>{title}</Link>
      </Td>

      <Td>
        <Link href={url_hash}>{hash}</Link>
      </Td>

      <Td>
        <div className={styles.result_type}>{data.type}</div>
      </Td>
    </Tr>
        );

}