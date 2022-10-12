import type { GetServerSideProps, NextPage } from 'next'
import { Search } from '../components/Search'

import styles from '../styles/search.module.css'
import { useEffect, useState } from 'react'

import { Spinner } from '@chakra-ui/react'
import { ProxySearchResults } from '../components/ProxySearchResults'
import config from '../config'



interface Props {
  q: string
}

const SearchResults: NextPage<Props> = ({q}) => {
  const [isLoading, setLoading] = useState(false);
  const [data, setData] = useState(null)

  useEffect(() => {
    if (config.PROXY_HOST) {
      setLoading(true)
      let url = new URL('/api/v1/search', config.PROXY_HOST).toString() + '?q=' + q;
      fetch(url)
        .then((res) => res.json())
        .then((data) => {
          setData(data)
          setLoading(false)
        })
    } else {
      console.log('failed to search: proxy host is unknown')
    }
  }, [q])

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
    props: {q: query.q},
  };
}

export default SearchResults
