import type { GetServerSideProps, NextPage } from 'next'
import { Search } from '../components/Search'

import styles from '../styles/search.module.css'
import { useEffect, useState } from 'react'

import { Spinner } from '@chakra-ui/react'
import { ProxySearchResults } from '../components/ProxySearchResults'


interface Props {
  q: string,
  client_proxy_host: string,
}

const SearchResults: NextPage<Props> = ({q, client_proxy_host}) => {
  const [isLoading, setLoading] = useState(false);
  const [data, setData] = useState(null)

  useEffect(() => {
    if (client_proxy_host) {
      setLoading(true)
      let url = new URL('/api/v1/search', client_proxy_host).toString() + '?q=' + q;
      fetch(url)
        .then((res) => res.json())
        .then((data) => {
          setData(data)
        })
        .finally(() => setLoading(false))
    } else {
      console.log('failed to search: proxy host is unknown')
    }
  }, [q, client_proxy_host])

  return (
    <>
        <Search initialSearchValue={q}>
          <div className={styles.output}>
            <div className={styles.output_results}>
              <p>Search results:</p>
            </div>
            { isLoading ? <Spinner size='md' color="blue"/> : <ProxySearchResults responses={data || {}}></ProxySearchResults> }
          </div>
        </Search>
    </>
  )
}

export const getServerSideProps: GetServerSideProps = async ({ query }) => {
  if (!query.q) {
    return {
      notFound: true,
    };
  }

  return {
    props: {
      q: query.q, 
      client_proxy_host: process.env.CLIENT_PROXY_HOST,
    },
  };
}

export default SearchResults
